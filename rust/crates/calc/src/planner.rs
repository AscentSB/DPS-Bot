//! The planner (`planUpgrades`, `greedyPlan`, `scoreUpgrades`, `armorBundles`, `pieceChanges`, `stepFacts`,
//! `replacedSoon` of src/calc/upgrades.js).
//!
//! The greedy plan is the JS one: every state's candidates are generated single-threaded ([`CandidateGen`]), scored in
//! parallel on rayon (results are collected in list order, so the plan equals a serial run), and the best
//! gain-per-million step is applied. Scored states are cached by content (`rules.states`) across re-plan rounds, a
//! per-plan lossless [`EvalMemo`] shares evaluations between candidates, and the plan is checked for cancellation
//! once per step.

use crate::candidates::CandidateGen;
use crate::edit::Edit;
use crate::eval::{weighted_gain, EvalResult};
use crate::ids::*;
use crate::loadout::Loadout;
use crate::memo::{EvalMemo, LoadoutKey};
use crate::pick::Evaluator;
use crate::run::PlanError;
use crate::{Ctx, Model};
use dps_core::config::constants::MAX_UPGRADES_LISTED;
use dps_core::config::pets::GOLDEN_DRAGON_BANK_CAP;
use dps_core::config::upgrades::EXTRA_UPGRADES;
use dps_core::data::accessory_family;
use dps_core::drops::{highest_floor, is_floor_drop};
use dps_core::jsfmt::{js_string, to_fixed, to_locale_string};
use dps_core::model::{PriceTables, Weights};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Most coins spent on an armor or equipment piece the plan later replaces (buy + upgrades, kept below this).
pub const REPLACED_PIECE_BUDGET: f64 = 25_000_000.0;
/// Armor bundles: the best this many single buys per armor slot are mixed into multi-piece steps.
const ARMOR_BUNDLE_SHORTLIST: usize = 3;
const REPLAN_ROUNDS: usize = 10;
/// A step overwritten within this many coins that adds at most `REPLACED_SOON_MIN_GAIN` percent is skipped.
pub const REPLACED_SOON_COINS: f64 = 10_000_000.0;
pub const REPLACED_SOON_MIN_GAIN: f64 = 5.0;
/// Floor-scaled dungeon armor from below floor 10 stays at or under the piece budget.
const MAX_FLOOR: usize = 10;
const BANK_STEP: f64 = 1_000_000.0;

/// One step of the plan, as `planUpgrades` returns it.
#[derive(Clone, Debug)]
pub struct PlanStep {
    pub label: String,
    pub cost: f64,
    pub source: String,
    pub unverified: Option<bool>,
    pub gain: f64,
    pub per_million: f64,
    pub wither_gain: f64,
    pub dragon_gain: f64,
}

/// What `planUpgrades` returns (without the internal bookkeeping).
pub struct PlanOutput {
    pub steps: Vec<PlanStep>,
    pub start_damage: EvalResult,
    pub end_damage: EvalResult,
    /// The loadout after the last step.
    pub loadout: Loadout,
}

// ---------------------------------------------------------------------------------------------------------------
// pieceChanges
// ---------------------------------------------------------------------------------------------------------------

const SLOT_NAMES: [&str; 9] =
    ["weapon", "HELMET", "CHESTPLATE", "LEGGINGS", "BOOTS", "NECKLACE", "CLOAK", "BELT", "GLOVES"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ChangeKind {
    Same,
    Upgrade,
    Replace,
}

/// A piece a step touches, while scoring (ids only).
#[derive(Clone, Copy, Debug)]
struct RawChange {
    slot: u8,
    item: ItemId,
    prev: Option<ItemId>,
    kind: ChangeKind,
}

/// A piece a step touches, with the JS string keys.
#[derive(Clone, Debug)]
struct Change {
    key: String,
    prev_key: Option<String>,
    kind: ChangeKind,
    floor_drop: bool,
}

impl Change {
    fn piece_key(&self) -> &str {
        if self.kind == ChangeKind::Upgrade {
            self.prev_key.as_deref().unwrap_or(&self.key)
        } else {
            &self.key
        }
    }
}

/// JS `pieceChanges(prev, next)`: weapon, armor and equipment pieces a step touches.
fn piece_changes(model: &Model, prev: &Loadout, next: &Loadout) -> Vec<RawChange> {
    let mut changes: Vec<RawChange> = Vec::new();
    let both = [false, true];
    let after_views = [next.gear_view(false), next.gear_view(true)];
    let before_views = [prev.gear_view(false), prev.gear_view(true)];
    // Fast path: nothing differs in either view.
    if before_views[0] == after_views[0] && before_views[1] == after_views[1] {
        return changes;
    }
    let mut seen: smallvec_like::Seen = smallvec_like::Seen::default();
    let worn_after = |it: ItemId| after_views.iter().any(|g| g.items().any(|x| x == it));
    for (vi, _) in both.iter().enumerate() {
        let before = &before_views[vi];
        let after = &after_views[vi];
        for slot in 0..9usize {
            let (a, b) = match slot {
                0 => (before.weapon, after.weapon),
                1..=4 => (before.armor.get(slot - 1), after.armor.get(slot - 1)),
                _ => (before.equipment.get(slot - 5), after.equipment.get(slot - 5)),
            };
            let Some(b) = b else { continue };
            if a == Some(b) {
                continue;
            }
            let bid = model.item(b).id.as_str();
            if !seen.insert(slot as u8, bid) {
                continue;
            }
            let kind = match a {
                None => ChangeKind::Same,
                Some(a) => {
                    let aid = model.item(a).id.as_str();
                    if aid == bid || worn_after(a) {
                        ChangeKind::Same
                    } else if crate::helpers::upgrades_into(aid, bid) {
                        ChangeKind::Upgrade
                    } else {
                        ChangeKind::Replace
                    }
                }
            };
            changes.push(RawChange { slot: slot as u8, item: b, prev: a, kind });
        }
    }
    changes
}

mod smallvec_like {
    /// A tiny set of `(slot, item id)` pairs.
    #[derive(Default)]
    pub struct Seen<'a>(Vec<(u8, &'a str)>);
    impl<'a> Seen<'a> {
        /// True when the pair was new.
        pub fn insert(&mut self, slot: u8, id: &'a str) -> bool {
            if self.0.iter().any(|&(s, i)| s == slot && i == id) {
                return false;
            }
            self.0.push((slot, id));
            true
        }
    }
}

fn convert_changes(model: &Model, ctx: &Ctx, raw: &[RawChange]) -> Vec<Change> {
    raw.iter()
        .map(|c| {
            let id = model.item(c.item).id.as_str();
            Change {
                key: format!("{}:{}", SLOT_NAMES[c.slot as usize], id),
                prev_key: c.prev.map(|p| format!("{}:{}", SLOT_NAMES[c.slot as usize], model.item(p).id)),
                kind: c.kind,
                floor_drop: is_floor_drop(id, &ctx.resource),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------
// stepFacts, overwrites, replacedSoon
// ---------------------------------------------------------------------------------------------------------------

/// JS `stepFacts(prev, next)`: what a step sets, as keys a later step can overwrite.
fn step_facts(model: &Model, prev: &Loadout, next: &Loadout) -> FxHashSet<String> {
    let mut facts: FxHashSet<String> = FxHashSet::default();
    let mut item = |slot: &str, a: Option<ItemId>, b: Option<ItemId>| {
        let Some(b) = b else { return };
        if a == Some(b) {
            return;
        }
        let bi = model.item(b);
        let Some(a) = a else {
            facts.insert(format!("{slot}:piece"));
            return;
        };
        let ai = model.item(a);
        if ai.id != bi.id && !crate::helpers::upgrades_into(&ai.id, &bi.id) {
            facts.insert(format!("{slot}:piece"));
            return;
        }
        if ai.reforge != bi.reforge {
            facts.insert(format!("{slot}:reforge"));
        }
        if ai.enrichment != bi.enrichment {
            facts.insert(format!("{slot}:enrichment"));
        }
        let mut keys: Vec<&String> = ai.enchants.keys().collect();
        for k in bi.enchants.keys() {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        for k in keys {
            if ai.enchants.get(k).copied().unwrap_or(0) != bi.enchants.get(k).copied().unwrap_or(0) {
                facts.insert(format!("{slot}:enchant:{k}"));
            }
        }
        let mut gkeys: Vec<&String> = ai.gems.slots.keys().collect();
        for k in bi.gems.slots.keys() {
            if !gkeys.contains(&k) {
                gkeys.push(k);
            }
        }
        for k in gkeys {
            if ai.gems.slots.get(k) != bi.gems.slots.get(k) {
                facts.insert(format!("{slot}:gem:{k}"));
            }
        }
    };
    item("weapon", prev.weapon, next.weapon);
    item("rag", prev.rag, next.rag);
    for i in 0..4 {
        item(ARMOR_NAMES[i], prev.armor.get(i), next.armor.get(i));
    }
    for i in 0..4 {
        item(EQUIPMENT_NAMES[i], prev.equipment.get(i), next.equipment.get(i));
    }
    item("dragon:weapon", prev.dragon_item(GearSlot::Weapon), next.dragon_item(GearSlot::Weapon));
    for i in 0..4 {
        item(&format!("dragon:{}", ARMOR_NAMES[i]), prev.dragon_item(GearSlot::Armor(i as u8)), next.dragon_item(GearSlot::Armor(i as u8)));
    }
    for i in 0..4 {
        item(
            &format!("dragon:{}", EQUIPMENT_NAMES[i]),
            prev.dragon_item(GearSlot::Equipment(i as u8)),
            next.dragon_item(GearSlot::Equipment(i as u8)),
        );
    }
    // Accessories: a new one takes over its family; a new enrichment replaces the old.
    let nb = model.bag(next.accessories);
    for (&kind, &b) in nb.kinds.iter().zip(nb.items.iter()) {
        let bi = model.item(b);
        let family = || accessory_family(&bi.id).unwrap_or("null").to_string();
        match model.bag_get(prev.accessories, kind) {
            None => {
                facts.insert(format!("acc:{}:piece", family()));
            }
            Some(a) => {
                if model.item(a).enrichment != bi.enrichment {
                    facts.insert(format!("acc:{}:enrichment", family()));
                }
            }
        }
    }
    for t in 0..2 {
        let (a, b) = (prev.active_pets[t], next.active_pets[t]);
        let Some(b) = b else { continue };
        if a == Some(b) {
            continue;
        }
        let target = if t == 0 { "wither" } else { "dragon" };
        let bp = model.pet(b);
        match a {
            Some(a) if model.pet(a).kind == bp.kind => {
                let ap = model.pet(a);
                if ap.held_item != bp.held_item || ap.tier != bp.tier {
                    facts.insert(format!("pet:{}:item", bp.kind));
                }
            }
            _ => {
                facts.insert(format!("activePet:{target}"));
            }
        }
    }
    for key in next.extras.keys() {
        if prev.extras.has(key) {
            continue;
        }
        if let Some(group) = EXTRA_UPGRADES.iter().find(|e| e.key == key).and_then(|e| e.group) {
            facts.insert(format!("extra:{group}"));
        }
    }
    facts
}

/// JS `factSlot(fact)`: the gear slot a fact belongs to (`HELMET`, `acc:<family>`, `dragon:<slot>`), or `None`.
fn fact_slot(fact: &str) -> Option<String> {
    let mut parts = fact.split(':');
    let first = parts.next()?;
    if first == "acc" || first == "dragon" {
        return Some(format!("{first}:{}", parts.next().unwrap_or("undefined")));
    }
    if first == "weapon" || first == "rag" || ARMOR_NAMES.contains(&first) || EQUIPMENT_NAMES.contains(&first) {
        return Some(first.to_string());
    }
    None
}

/// JS `overwrites(later, earlier)`.
fn overwrites(later: &FxHashSet<String>, earlier: &FxHashSet<String>) -> bool {
    for f in earlier {
        if later.contains(f) {
            return true;
        }
        if let Some(slot) = fact_slot(f) {
            if later.contains(&format!("{slot}:piece")) {
                return true;
            }
        }
    }
    false
}

/// JS `replacedSoon(steps)`: labels of steps overwritten within `REPLACED_SOON_COINS` that add little.
fn replaced_soon(steps: &[GStep]) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..steps.len() {
        if steps[i].facts.is_empty() || steps[i].gain > REPLACED_SOON_MIN_GAIN {
            continue;
        }
        let mut coins = 0.0;
        for j in i + 1..steps.len() {
            coins += steps[j].cost;
            if coins > REPLACED_SOON_COINS {
                break;
            }
            if overwrites(&steps[j].facts, &steps[i].facts) {
                out.push(steps[i].label.clone());
                break;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------------------------------------------

/// The result of scoring one candidate (JS `{ index, changes, gain, shortlistOnly? }`).
struct Scored {
    index: usize,
    changes: Vec<RawChange>,
    gain: f64,
    shortlist_only: bool,
}

/// Score one candidate (or bundle): the JS body of `scoreUpgrades`' loop. `ev` evaluates without the memo (a candidate
/// loadout is rarely seen twice and the memo costs more than it saves); `evm` has the per-plan memo and is used where
/// evaluations repeat a lot: the pet re-pick of a pet buy.
fn score_edits(ev: &Evaluator<'_>, evm: &Evaluator<'_>, l: &Loadout, current: &EvalResult, index: usize, edits: &[&Edit], repick: bool, armor: bool) -> Option<Scored> {
    let pick = |e: &Edit| if e.needs_evaluator() { evm } else { ev };
    let mut next = edits[0].apply_scoring(pick(edits[0]), l);
    for e in &edits[1..] {
        next = e.apply_scoring(pick(e), &next);
    }
    let changes = piece_changes(ev.model, l, &next);
    let mut gain = weighted_gain(current, &ev.evaluate(&next), &ev.weights);
    if repick && gain > 1e-9 {
        next = ev.select_power(&ev.select_pet(&next));
        gain = weighted_gain(current, &ev.evaluate(&next), &ev.weights);
    }
    if gain > 1e-9 {
        Some(Scored { index, changes, gain, shortlist_only: false })
    } else if armor {
        Some(Scored { index, changes, gain, shortlist_only: true })
    } else {
        None
    }
}

/// An armor bundle (JS `armorBundles` entry): several armor buys as one step.
struct Bundle {
    label: String,
    cost: f64,
    source: String,
    unverified: bool,
    picks: Vec<usize>,
}

/// JS `armorBundles(singles)`; `singles` are `(candidate index, gain)` of the main-set armor buys, in list order.
fn armor_bundles(ups: &[crate::candidates::Candidate], singles: &[(usize, f64)]) -> Vec<Bundle> {
    // slot -> armorId -> (index, gain), both in first-insertion order.
    let mut by_slot: Vec<indexmap::IndexMap<&str, (usize, f64)>> = (0..4).map(|_| indexmap::IndexMap::new()).collect();
    for &(index, gain) in singles {
        let up = &ups[index];
        let Some(slot_name) = up.armor_slot else { continue };
        let Some(slot) = ARMOR_NAMES.iter().position(|&n| n == slot_name) else { continue };
        let id = up.armor_id.as_deref().unwrap_or("undefined");
        match by_slot[slot].get_mut(id) {
            Some(prev) => {
                if gain > prev.1 {
                    *prev = (index, gain);
                }
            }
            None => {
                by_slot[slot].insert(id, (index, gain));
            }
        }
    }
    let lists: Vec<Vec<Option<usize>>> = by_slot
        .iter()
        .map(|m| {
            let mut v: Vec<(usize, f64)> = m.values().copied().collect();
            v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            std::iter::once(None).chain(v.into_iter().take(ARMOR_BUNDLE_SHORTLIST).map(|x| Some(x.0))).collect()
        })
        .collect();
    let mut bundles = Vec::new();
    let mut picked: Vec<usize> = Vec::new();
    fn walk(i: usize, lists: &[Vec<Option<usize>>], picked: &mut Vec<usize>, ups: &[crate::candidates::Candidate], out: &mut Vec<Bundle>) {
        if i == lists.len() {
            if picked.len() < 2 {
                return;
            }
            let label = format!("Armor bundle: {}", picked.iter().map(|&p| &*ups[p].label).collect::<Vec<_>>().join("; "));
            let mut cost = 0.0;
            for &p in picked.iter() {
                cost += ups[p].cost;
            }
            let mut sources: Vec<&str> = Vec::new();
            for &p in picked.iter() {
                for s in ups[p].source.split('+') {
                    if !sources.contains(&s) {
                        sources.push(s);
                    }
                }
            }
            out.push(Bundle {
                label,
                cost,
                source: sources.join("+"),
                unverified: picked.iter().any(|&p| ups[p].unverified == Some(true)),
                picks: picked.clone(),
            });
            return;
        }
        for up in &lists[i] {
            match up {
                Some(u) => {
                    picked.push(*u);
                    walk(i + 1, lists, picked, ups, out);
                    picked.pop();
                }
                None => walk(i + 1, lists, picked, ups, out),
            }
        }
    }
    walk(0, &lists, &mut picked, ups, &mut bundles);
    bundles
}

// ---------------------------------------------------------------------------------------------------------------
// The greedy plan
// ---------------------------------------------------------------------------------------------------------------

/// A scored candidate kept in a state (JS `light(...)` result).
struct Cand {
    label: String,
    cost: f64,
    source: String,
    unverified: Option<bool>,
    bank: bool,
    stacks_on: Option<String>,
    edits: Vec<Edit>,
    changes: Vec<Change>,
    gain: f64,
    per_million: f64,
}

struct State {
    loadout: Loadout,
    current: EvalResult,
    candidates: Option<Vec<Arc<Cand>>>,
}

/// Everything that makes two loadouts the same planner state (JS `stateFor` key).
#[derive(Hash, PartialEq, Eq)]
struct StateKey {
    base: LoadoutKey,
    owned: (u64, u64, usize),
    powers: Vec<u8>,
    blades: Vec<ItemId>,
}

fn state_key(model: &Model, l: &Loadout) -> StateKey {
    let (mut sum, mut xor) = (0u64, 0u64);
    for id in l.owned.iter() {
        let mut h = FxHasher::default();
        id.hash(&mut h);
        let v = h.finish().wrapping_mul(0x9E37_79B9_7F4A_7C15);
        sum = sum.wrapping_add(v);
        xor ^= v.rotate_left(17);
    }
    let mut powers = l.unlocked_powers.to_vec();
    powers.sort_unstable();
    StateKey { base: LoadoutKey::new(model, l), owned: (sum, xor, l.owned.len()), powers, blades: l.blades.to_vec() }
}

/// JS `path`: the state plus the spending bookkeeping.
#[derive(Clone)]
struct PathInfo {
    state: usize,
    spend: HashMap<String, f64>,
    parent: HashMap<String, String>,
    replaced: FxHashSet<String>,
}

struct GStep {
    label: String,
    cost: f64,
    source: String,
    unverified: Option<bool>,
    gain: f64,
    per_million: f64,
    facts: FxHashSet<String>,
    choice: Arc<Cand>,
    path: Arc<PathInfo>,
}

struct Greedy {
    steps: Vec<GStep>,
    start_state: usize,
    end_state: usize,
    overspent: Vec<String>,
}

fn allowed(cand: &Cand, spend: &HashMap<String, f64>, doomed: &FxHashSet<String>, banned: &FxHashSet<String>, low_floor: bool) -> bool {
    if banned.contains(&cand.label) {
        return false;
    }
    !cand.changes.iter().any(|c| {
        let is_doomed = doomed.contains(c.piece_key()) || (low_floor && c.floor_drop);
        is_doomed && spend.get(c.piece_key()).copied().unwrap_or(0.0) + cand.cost >= REPLACED_PIECE_BUDGET
    })
}

pub(crate) struct Planner<'a> {
    pub model: &'a mut Model,
    pub ctx: &'a Ctx,
    pub weights: Weights,
    pub prices: &'a PriceTables,
    pub gen: CandidateGen,
    pub memo: EvalMemo,
    store: Vec<State>,
    index: FxHashMap<StateKey, usize>,
    doomed: FxHashSet<String>,
    banned: FxHashSet<String>,
    cancel: &'a AtomicBool,
    low_floor: bool,
}

impl<'a> Planner<'a> {
    fn ev(&self) -> Evaluator<'_> {
        Evaluator::new(self.model, self.ctx, self.weights)
    }

    fn state_for(&mut self, loadout: &Loadout) -> usize {
        let key = state_key(self.model, loadout);
        if let Some(&i) = self.index.get(&key) {
            return i;
        }
        let current = self.ev().evaluate(loadout);
        self.store.push(State { loadout: loadout.clone(), current, candidates: None });
        let i = self.store.len() - 1;
        self.index.insert(key, i);
        i
    }

    /// JS `candidatesFor(state)`: every upgrade from this state that adds damage, scored in parallel.
    fn candidates_for(&mut self, idx: usize) -> Vec<Arc<Cand>> {
        let loadout = self.store[idx].loadout.clone();
        let current = self.store[idx].current;
        let ups = self.gen.available_upgrades(self.model, self.ctx, self.weights, self.prices, &loadout);
        let banned = &self.banned;
        let ev = Evaluator::new(self.model, self.ctx, self.weights);
        let evm = ev.with_memo(&self.memo);
        let scored: Vec<Scored> = ups
            .par_iter()
            .enumerate()
            .with_min_len(16)
            .filter_map(|(i, up)| {
                if !(up.cost > 0.0) || banned.contains(&*up.label) {
                    return None;
                }
                score_edits(&ev, &evm, &loadout, &current, i, &[&up.edit], up.repick == Some(true), up.armor_slot.is_some())
            })
            .collect();
        let singles: Vec<(usize, f64)> = scored.iter().filter(|x| ups[x.index].armor_slot.is_some()).map(|x| (x.index, x.gain)).collect();
        let bundles = armor_bundles(&ups, &singles);
        let bundle_scored: Vec<Scored> = bundles
            .par_iter()
            .enumerate()
            .filter_map(|(i, b)| {
                if !(b.cost > 0.0) || banned.contains(&b.label) {
                    return None;
                }
                let edits: Vec<&Edit> = b.picks.iter().map(|&p| &ups[p].edit).collect();
                score_edits(&ev, &evm, &loadout, &current, i, &edits, true, false)
            })
            .collect();
        let mut out: Vec<Arc<Cand>> = Vec::with_capacity(scored.len() + bundle_scored.len());
        for s in scored.iter().filter(|x| !x.shortlist_only) {
            let up = &ups[s.index];
            out.push(Arc::new(Cand {
                label: up.label.to_string(),
                cost: up.cost,
                source: up.source.to_string(),
                unverified: up.unverified,
                bank: up.bank,
                stacks_on: up.stacks_on.clone(),
                edits: vec![up.edit.clone()],
                changes: convert_changes(self.model, self.ctx, &s.changes),
                gain: s.gain,
                per_million: s.gain / (up.cost / 1_000_000.0),
            }));
        }
        for s in bundle_scored.iter().filter(|x| !x.shortlist_only) {
            let b = &bundles[s.index];
            out.push(Arc::new(Cand {
                label: b.label.clone(),
                cost: b.cost,
                source: b.source.clone(),
                unverified: Some(b.unverified),
                bank: false,
                stacks_on: None,
                edits: b.picks.iter().map(|&p| ups[p].edit.clone()).collect(),
                changes: convert_changes(self.model, self.ctx, &s.changes),
                gain: s.gain,
                per_million: s.gain / (b.cost / 1_000_000.0),
            }));
        }
        out
    }

    /// JS `greedyPlan`.
    fn greedy(&mut self, start: &Loadout, limit: usize, prev: Option<Greedy>) -> Result<Greedy, PlanError> {
        let (mut steps, mut path, start_state): (Vec<GStep>, Arc<PathInfo>, usize);
        match prev {
            Some(p) => {
                let keep = p
                    .steps
                    .iter()
                    .position(|st| !allowed(&st.choice, &st.path.spend, &self.doomed, &self.banned, self.low_floor));
                let Some(keep) = keep else { return Ok(p) };
                let mut s = p.steps;
                s.truncate(keep + 1);
                let last = s.pop().expect("kept step");
                path = last.path;
                steps = s;
                start_state = p.start_state;
            }
            None => {
                let st = self.state_for(start);
                steps = Vec::new();
                path = Arc::new(PathInfo { state: st, spend: HashMap::new(), parent: HashMap::new(), replaced: FxHashSet::default() });
                start_state = st;
            }
        }

        while steps.len() < limit {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(PlanError::Cancelled);
            }
            let sidx = path.state;
            if self.store[sidx].candidates.is_none() {
                let c = self.candidates_for(sidx);
                self.store[sidx].candidates = Some(c);
            }
            let state_loadout = self.store[sidx].loadout.clone();
            let mut best: Option<Arc<Cand>> = None;
            for cand in self.store[sidx].candidates.as_ref().unwrap() {
                if allowed(cand, &path.spend, &self.doomed, &self.banned, self.low_floor)
                    && best.as_ref().map_or(true, |b| cand.per_million > b.per_million)
                {
                    best = Some(cand.clone());
                }
            }
            let Some(best) = best else { break };
            let mut loadout = {
                let ev = self.ev();
                let mut l = best.edits[0].apply(&ev, &state_loadout);
                for e in &best.edits[1..] {
                    l = e.apply(&ev, &l);
                }
                ev.select_power(&ev.select_pet(&l))
            };
            let mut label = best.label.clone();
            let mut cost = best.cost;
            let mut gain = best.gain;
            let mut per_million = best.per_million;
            // Deposits right after a deposit join the step before, so the plan shows one bank step.
            let joined = if best.bank && steps.last().is_some_and(|s| s.choice.bank) { steps.pop() } else { None };
            let base_idx = match &joined {
                Some(j) => j.path.state,
                None => sidx,
            };
            if let Some(j) = &joined {
                path = j.path.clone();
            }
            if best.bank {
                // Keep depositing 1M at a time while the next million still beats every other option, as one step.
                let mut next: Option<f64> = None;
                for cand in self.store[sidx].candidates.as_ref().unwrap() {
                    if !cand.bank
                        && allowed(cand, &path.spend, &self.doomed, &self.banned, self.low_floor)
                        && next.map_or(true, |n| cand.per_million > n)
                    {
                        next = Some(cand.per_million);
                    }
                }
                let mut now = self.ev().evaluate(&loadout);
                while loadout.bank_value(self.model) < GOLDEN_DRAGON_BANK_CAP {
                    let target = dps_core::jsmath::min(GOLDEN_DRAGON_BANK_CAP, loadout.bank_value(self.model) + BANK_STEP);
                    let bank_id = self.model.bank_id(target);
                    let more = crate::edit::deposit_bank(&loadout, bank_id);
                    let after = self.ev().evaluate(&more);
                    let g = weighted_gain(&now, &after, &self.weights);
                    if !(g > 1e-9) || next.is_some_and(|n| g < n) {
                        break;
                    }
                    loadout = more;
                    now = after;
                }
                let base_bank = self.store[base_idx].loadout.bank_value(self.model);
                let amount = loadout.bank_value(self.model) - base_bank;
                gain = weighted_gain(&self.store[base_idx].current, &now, &self.weights);
                let amt = if amount >= 1e9 {
                    let v: f64 = to_fixed(amount / 1e9, 3).parse().unwrap_or(0.0);
                    format!("{}B", js_string(v))
                } else {
                    format!("{}M", to_locale_string(amount / 1e6))
                };
                label = format!("Put {amt} coins in the bank (Golden Dragon Legendary Treasure)");
                cost = amount;
                per_million = gain / (amount / 1e6);
            }
            let mut facts = step_facts(self.model, &self.store[base_idx].loadout, &loadout);
            // A combined enchant level builds on the one before (nothing is wasted), so it does not overwrite it.
            if let Some(s) = &best.stacks_on {
                facts.remove(s);
            }
            steps.push(GStep {
                label,
                cost,
                source: best.source.clone(),
                unverified: best.unverified,
                gain,
                per_million,
                facts,
                choice: best.clone(),
                path: path.clone(),
            });
            let mut spend = path.spend.clone();
            let mut parent = path.parent.clone();
            let mut replaced = path.replaced.clone();
            for c in &best.changes {
                let cur = spend.get(c.piece_key()).copied().unwrap_or(0.0);
                spend.insert(c.key.clone(), cur + best.cost);
                if c.kind == ChangeKind::Upgrade {
                    if let Some(pk) = &c.prev_key {
                        parent.insert(c.key.clone(), pk.clone());
                    }
                }
                if c.kind == ChangeKind::Replace {
                    if let Some(pk) = &c.prev_key {
                        replaced.insert(pk.clone());
                    }
                }
            }
            let st = self.state_for(&loadout);
            path = Arc::new(PathInfo { state: st, spend, parent, replaced });
        }
        // Pieces this plan replaces after spending the budget or more on them, with the ids they had before being
        // upgraded (so the budget also covers spending before the upgrade).
        let mut overspent = Vec::new();
        for key in &path.replaced {
            if path.spend.get(key).copied().unwrap_or(0.0) < REPLACED_PIECE_BUDGET {
                continue;
            }
            let mut k = Some(key);
            while let Some(kk) = k {
                overspent.push(kk.clone());
                k = path.parent.get(kk);
            }
        }
        Ok(Greedy { steps, start_state, end_state: path.state, overspent })
    }
}

/// JS `planUpgrades(startLoadout, ctx, prices, weights, limit, scorer, isCancelled)`.
///
/// `model` must be the model the start loadout was built with, and `ctx` the context after `build_loadout` (its
/// leftover tuning split is set). Candidate scoring runs on the rayon pool.
pub fn plan_upgrades(
    model: &mut Model,
    ctx: &Ctx,
    prices: &PriceTables,
    weights: Weights,
    limit: Option<usize>,
    start: &Loadout,
    cancel: &AtomicBool,
) -> Result<PlanOutput, PlanError> {
    let limit = limit.unwrap_or(MAX_UPGRADES_LISTED);
    let memo = EvalMemo::new(ctx);
    let low_floor = highest_floor(ctx.cata_level) < MAX_FLOOR;
    let mut p = Planner {
        model,
        ctx,
        weights,
        prices,
        gen: CandidateGen::new(),
        memo,
        store: Vec::new(),
        index: FxHashMap::default(),
        doomed: FxHashSet::default(),
        banned: FxHashSet::default(),
        cancel,
        low_floor,
    };
    let mut plan = p.greedy(start, limit, None)?;
    // Re-plan until no piece is overspent before being replaced (25M budget) and no small step is overwritten
    // within 10M coins. Each round resumes from the first step the new rules reject.
    for _ in 0..REPLAN_ROUNDS {
        let fresh_pieces: Vec<String> = plan.overspent.iter().filter(|k| !p.doomed.contains(*k)).cloned().collect();
        let fresh_steps: Vec<String> = replaced_soon(&plan.steps).into_iter().filter(|l| !p.banned.contains(l)).collect();
        if fresh_pieces.is_empty() && fresh_steps.is_empty() {
            break;
        }
        p.doomed.extend(fresh_pieces);
        p.banned.extend(fresh_steps);
        plan = p.greedy(start, limit, Some(plan))?;
    }
    // % change vs withers and dragons for each step (the weighted gain combines them).
    let pct = |a: f64, b: f64| if a > 0.0 { (b / a - 1.0) * 100.0 } else if b > 0.0 { 100.0 } else { 0.0 };
    let full = |st: usize| crate::eval::evaluate(p.model, p.ctx, &p.store[st].loadout, &p.weights);
    let end_current = full(plan.end_state);
    let mut steps = Vec::with_capacity(plan.steps.len());
    for (i, s) in plan.steps.iter().enumerate() {
        let before = full(s.path.state);
        let after = match plan.steps.get(i + 1) {
            Some(n) => full(n.path.state),
            None => end_current,
        };
        steps.push(PlanStep {
            label: s.label.clone(),
            cost: s.cost,
            source: s.source.clone(),
            unverified: s.unverified,
            gain: s.gain,
            per_million: s.per_million,
            wither_gain: pct(before.damage[0], after.damage[0]),
            dragon_gain: pct(before.damage[1], after.damage[1]),
        });
    }
    Ok(PlanOutput {
        steps,
        start_damage: full(plan.start_state),
        end_damage: end_current,
        loadout: p.store[plan.end_state].loadout.clone(),
    })
}
