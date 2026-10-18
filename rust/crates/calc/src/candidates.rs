//! `availableUpgrades` (src/calc/upgrades.js): every upgrade available from a loadout, in the JS order, as data.
//!
//! [`CandidateGen`] holds what the JS keeps on the context or in module caches for one plan: the `bestReforge` cache,
//! interned fresh items, the cached `hasUsefulStats` answers and the per-type pet purchase data. Create one per plan
//! (prices and the context must not change under it). Candidate lists are built single-threaded with `&mut Model`;
//! each [`Candidate`] carries an [`Edit`] that can then be applied on any thread.

use crate::arena::Model;
use crate::ctx::{Ctx, UpgradeMode};
use crate::edit::{BuyEdit, Edit, ReplacePost};
use crate::helpers::{can_recomb, is_dungeon, item_rarity_of, new_item};
use crate::ids::*;
use crate::loadout::{Levels, Loadout};
use crate::parts::*;
use crate::pick::Evaluator;
use dps_core::clean_name;
use dps_core::config::accessories::{
    enrichment, enrichment_kinds, ENRICHABLE_RARITIES, ENRICHMENT_SWAPPER_ID, ENRICHMENTS, POWERS, POWER_STONES,
};
use dps_core::config::constants::*;
use dps_core::config::enchants::{enchant, ENCHANTS};
use dps_core::config::essence::{ESSENCE_MIN_CATA, ESSENCE_PERKS};
use dps_core::config::gemstones::{gem_stat, gemstone, slot_gems, GEM_QUALITIES, UPGRADE_QUALITIES};
use dps_core::config::item_upgrades::item_upgrades;
use dps_core::config::pets::{
    kat_upgrade, GOLDEN_DRAGON_BANK_CAP, KAT_FLOWER_ID, KAT_LEVEL_DISCOUNT, PET_ITEMS, TIER_BOOST_ID, BUY_RARITIES,
    EXCLUDED_PETS,
};
use dps_core::config::profile::{
    BOOK_DAMAGE_OPTION, BOOK_DAMAGE_UNLOCK_LEVEL, BOOK_OF_PROGRESSION_ID, BOOK_SWAP_COINS_PER_LEVEL,
};
use dps_core::config::shards::{shard_hunting_req, shards_cumulative, SHARDS, SHARD_MAX_LEVEL};
use dps_core::config::upgrades::EXTRA_UPGRADES;
use dps_core::data::{accessory_chain_position, accessory_family, all_reforges, fragged, get_reforge, Reforge};
use dps_core::drops::{buy_price_id, drop_base_rarity};
use dps_core::jsfmt;
use dps_core::kinds::Mode;
use std::sync::Arc;
use dps_core::model::{Gem, ItemInfo, ItemState, Pet, PriceTables, Resource, Weights};
use dps_core::pets::{all_pet_types, effective_tier, max_pet_level, pet_stats, pet_tiers};
use dps_core::requirements::{meets_requirements, Player};
use dps_core::stat::Stat;
use rustc_hash::{FxHashMap, FxHashSet};

/// Items nobody can buy (admin and unobtainable items like the Raygun).
const UNBUYABLE_TIERS: [&str; 2] = ["UNOBTAINABLE", "ADMIN"];
/// Free items (no coins): offered as a "Claim" step priced at 1 coin. JS `FREE_ITEMS`.
pub const FREE_ITEMS: [(&str, &str); 1] = [("GOLD_NECRON_HEAD", "free with 50 completions")];
/// The Mage clear set never suggests these pets.
const RCM_SKIPPED_PETS: [&str; 3] = ["CROW", "SHEEP", "PIGMAN"];
/// Bank deposits for Golden Dragon, 1M at a time.
pub const BANK_STEP: f64 = 1_000_000.0;
pub const BANK_LABEL: &str = "Put 1M coins in the bank (Golden Dragon Legendary Treasure)";
/// Learning a power at Maxwell uses up this many of its power stone.
const POWER_STONES_TO_LEARN: f64 = 9.0;
/// Mage weapon buys bundled with the current weapon's cheaper enchants: books up to this price each.
const COPY_BOOK_MAX: f64 = 10_000_000.0;
const DPS_STATS: [Stat; 9] = [
    Stat::Damage,
    Stat::Strength,
    Stat::CritDamage,
    Stat::CritChance,
    Stat::Intelligence,
    Stat::Speed,
    Stat::AttackSpeed,
    Stat::AbilityDamage,
    Stat::Ferocity,
];
/// Stats a pet item has to raise to be worth offering.
const PET_ITEM_STATS: [Stat; 6] =
    [Stat::Strength, Stat::CritDamage, Stat::CritChance, Stat::Intelligence, Stat::AttackSpeed, Stat::AbilityDamage];
const SCROLL_NAMES: [(&str, &str); 3] =
    [("IMPLOSION_SCROLL", "Implosion"), ("SHADOW_WARP_SCROLL", "Shadow Warp"), ("WITHER_SHIELD_SCROLL", "Wither Shield")];

/// One upgrade the planner can take (JS `{ label, cost, source, apply, unverified?, group?, stacksOn?, repick?,
/// armorSlot?, armorId?, bank? }`).
#[derive(Clone, Debug)]
pub struct Candidate {
    pub label: Arc<str>,
    pub cost: f64,
    pub source: Arc<str>,
    pub unverified: Option<bool>,
    pub group: Option<Arc<str>>,
    pub stacks_on: Option<String>,
    pub repick: Option<bool>,
    /// The armor category (`HELMET`, ...) of a main-set armor buy.
    pub armor_slot: Option<&'static str>,
    pub armor_id: Option<Arc<str>>,
    pub bank: bool,
    pub edit: Edit,
}

#[derive(Default)]
struct Extra {
    unverified: Option<bool>,
    group: Option<String>,
    stacks_on: Option<String>,
    repick: Option<bool>,
    armor_slot: Option<&'static str>,
    armor_id: Option<String>,
    bank: bool,
}

fn group(s: String) -> Extra {
    Extra { group: Some(s), ..Default::default() }
}

fn free_item(id: &str) -> Option<&'static str> {
    FREE_ITEMS.iter().find(|(k, _)| *k == id).map(|&(_, v)| v)
}

/// JS `FREE_UPGRADES[id]`: `(from id, parts)` for an id a free item upgrades into.
fn free_upgrade(id: &str) -> Option<(&'static str, &'static [dps_core::config::pets::Part])> {
    for (from, _) in FREE_ITEMS {
        if let Some(ups) = item_upgrades(from) {
            for up in ups {
                if up.id == id {
                    return Some((from, up.parts));
                }
            }
        }
    }
    None
}

/// `(bag, old, next) -> bag after replaceItem's bag step`: the same replacements come back state after state.
pub type BagCache = FxHashMap<(BagId, ItemId, ItemId), Option<BagId>>;

fn bag_replace_cached(model: &mut Model, cache: &mut BagCache, bag: BagId, old: ItemId, next: ItemId) -> Option<BagId> {
    *cache.entry((bag, old, next)).or_insert_with(|| model.bag_replace(bag, old, next))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CopyKind {
    None,
    Cheap,
    All,
}

#[derive(Clone, Copy)]
struct BuyExtra {
    recomb: bool,
    full: bool,
    enrichment: Option<&'static str>,
    gems: bool,
    copy: CopyKind,
}

impl BuyExtra {
    fn new() -> BuyExtra {
        BuyExtra { recomb: false, full: false, enrichment: None, gems: false, copy: CopyKind::None }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BuySet {
    Main,
    Dragon,
    Wither,
}

#[derive(Clone, Copy)]
struct ItemMeta {
    recombable: bool,
    best: Option<&'static Reforge>,
    /// `ENRICHABLE_RARITIES.includes(itemRarity({ ...fresh, recomb }))` for recomb false / true.
    enrichable: [bool; 2],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BuyKey {
    ri: u32,
    set: u8,
    reforge: u16,
    recomb: bool,
    enrichment: u8,
    gems: bool,
    copy: u8,
    src: u32,
    keep: u32,
    cloak: bool,
}

struct BuyTemplate {
    label: Arc<str>,
    cost: f64,
    source: Arc<str>,
    item: ItemId,
    group: Option<Arc<str>>,
    repick: bool,
    armor_slot: Option<&'static str>,
    armor_id: Option<Arc<str>>,
}

struct PetVariant {
    id: PetId,
    label: String,
    parts: Vec<Part>,
}

struct PetBuyType {
    kind: &'static str,
    variants: Vec<PetVariant>,
}

/// Per-plan state of candidate generation (see the module docs).
pub struct CandidateGen {
    /// `ctx.reforgeCache`: best reforge per `category:rarity:dungeon`, as an index into `all_reforges()`.
    reforge_cache: FxHashMap<String, Option<usize>>,
    /// `newItem(id, resource)` interned, by resource index.
    fresh: Vec<Option<ItemId>>,
    /// `hasUsefulStats` by resource index: 0 unknown, 1 no, 2 yes.
    useful: Vec<u8>,
    pet_buys: Option<Vec<PetBuyType>>,
    pet_gain_cache: FxHashMap<Pet, [f64; 6]>,
    bag_cache: BagCache,
    bag_with_cache: FxHashMap<(BagId, ItemId), BagId>,
    /// Buy candidates by what they depend on (see `BuyKey`); `None` = not priced / not possible.
    buy_cache: FxHashMap<BuyKey, Option<BuyTemplate>>,
    /// `canRecomb`, the best reforge and enrichable rarities of a fresh item, by resource index.
    meta: Vec<Option<ItemMeta>>,
    /// Candidates of an accessory, by item (they depend on nothing else but the accessory bag).
    acc_cache: FxHashMap<ItemId, Vec<Candidate>>,
    prices_version: Option<u64>,
    /// Wall time spent per section of `available_upgrades` over all calls (for profiling).
    pub section_nanos: [u64; 14],
}

impl Default for CandidateGen {
    fn default() -> Self {
        CandidateGen::new()
    }
}

impl CandidateGen {
    pub fn new() -> CandidateGen {
        CandidateGen { reforge_cache: FxHashMap::default(), fresh: Vec::new(), useful: Vec::new(), pet_buys: None, pet_gain_cache: FxHashMap::default(), bag_cache: FxHashMap::default(), bag_with_cache: FxHashMap::default(), buy_cache: FxHashMap::default(), meta: Vec::new(), acc_cache: FxHashMap::default(), prices_version: None, section_nanos: [0; 14] }
    }

    /// The `ctx.reforgeCache` entry for a key (for tests): the best reforge's key.
    pub fn cached_reforge(&self, key: &str) -> Option<Option<&'static str>> {
        self.reforge_cache.get(key).map(|o| o.map(|i| all_reforges()[i].key))
    }

    /// JS `availableUpgrades(loadout, ctx, prices)`. `weights` is `ctx.weights` (what `planUpgrades` sets): it scores
    /// the reforge tests and re-picks the pet of `buyPet` edits.
    pub fn available_upgrades(
        &mut self,
        model: &mut Model,
        ctx: &Ctx,
        weights: Weights,
        prices: &PriceTables,
        l: &Loadout,
    ) -> Vec<Candidate> {
        debug_assert!(model.accepts(ctx));
        let resource: std::sync::Arc<Resource> = ctx.resource.clone();
        let player = Player { cata_level: ctx.cata_level, skills: ctx.skills.clone(), slayers: ctx.slayers.clone() };
        if self.fresh.len() != resource.len() {
            self.fresh = vec![None; resource.len()];
            self.useful = vec![0; resource.len()];
            self.meta = vec![None; resource.len()];
        }
        // Everything cached by price (templates) is only valid for the prices it was built with.
        if self.prices_version != Some(prices.version) {
            self.prices_version = Some(prices.version);
            self.buy_cache.clear();
            self.acc_cache.clear();
        }
        let mut b = B { model, ctx, weights, prices, resource: &resource, player: &player, g: self, ups: Vec::with_capacity(4096), l };
        b.run();
        b.ups
    }
}

struct B<'a> {
    model: &'a mut Model,
    ctx: &'a Ctx,
    weights: Weights,
    prices: &'a PriceTables,
    resource: &'a Resource,
    player: &'a Player,
    g: &'a mut CandidateGen,
    ups: Vec<Candidate>,
    l: &'a Loadout,
}

fn has_dps_stats(stats: Option<&[(Stat, f64)]>) -> bool {
    stats.is_some_and(|s| s.iter().any(|&(k, v)| DPS_STATS.contains(&k) && v != 0.0 && !v.is_nan()))
}

fn family_rank(rarity: &str, id: &str) -> i64 {
    rarity_index(stat_rarity(rarity)).map(|i| i as i64).unwrap_or(-1) * 100 + accessory_chain_position(id) as i64
}

/// Whether an item with this reforge (a key of `all_reforges()`) would be the same reforge object.
fn same_reforge(a: &Option<String>, r: &Reforge) -> bool {
    a.as_deref().and_then(get_reforge).is_some_and(|x| x.index == r.index)
}

struct GemUp {
    gem: &'static str,
    quality: &'static str,
    unlock: bool,
    parts: Vec<Part>,
    slot_key: String,
    unlocked_before: bool,
}

/// JS `gemUpgrades(item, ctx)`.
fn gem_upgrades(st: &ItemState, info: Option<&ItemInfo>, upgrade_max: bool) -> Vec<GemUp> {
    let Some(slots) = info.and_then(|i| i.gemstone_slots.as_ref()) else { return vec![] };
    if slots.is_empty() {
        return vec![];
    }
    let rarity = stat_rarity(item_rarity_of(st)).to_string();
    let mut counts: Vec<(String, usize)> = Vec::new();
    let mut out = Vec::new();
    for slot in slots {
        let ty = slot.slot_type.as_str();
        let index = match counts.iter_mut().find(|(t, _)| t == ty) {
            Some((_, n)) => {
                *n += 1;
                *n
            }
            None => {
                counts.push((ty.to_string(), 0));
                0
            }
        };
        let key = format!("{ty}_{index}");
        let current = st.gems.slots.get(&key);
        let unlocked = slot.costs.is_none() || st.gems.unlocked.iter().any(|u| *u == key);
        let accepts: Vec<&'static str> = match slot_gems(ty) {
            Some(list) => list.iter().copied().filter(|g| gemstone(g).is_some()).collect(),
            None => gemstone(ty).map(|g| g.gem).into_iter().collect(),
        };
        for gem in accepts {
            let qualities: &[&'static str] = if upgrade_max { &UPGRADE_QUALITIES[UPGRADE_QUALITIES.len() - 1..] } else { &UPGRADE_QUALITIES };
            for &quality in qualities {
                if let Some(c) = current {
                    let qi = GEM_QUALITIES.iter().position(|&q| q == c.quality).map(|i| i as i32).unwrap_or(-1);
                    let ni = GEM_QUALITIES.iter().position(|&q| q == quality).map(|i| i as i32).unwrap_or(-1);
                    if c.gem == gem && qi >= ni {
                        continue;
                    }
                }
                if gem_stat(gem, quality, &rarity).is_none() {
                    continue;
                }
                let mut parts = vec![Part::owned(format!("{quality}_{gem}_GEM"))];
                if !unlocked {
                    push_slot_costs(&mut parts, slot.costs.as_deref().unwrap_or(&[]));
                }
                out.push(GemUp { gem, quality, unlock: !unlocked, parts, slot_key: key.clone(), unlocked_before: unlocked });
            }
        }
    }
    out
}

fn push_slot_costs(parts: &mut Vec<Part>, costs: &[dps_core::model::GemCost]) {
    for c in costs {
        if c.kind == "COINS" {
            parts.push(Part::Coins(c.coins.unwrap_or(f64::NAN)));
        } else if c.kind == "ITEM" {
            parts.push(Part::owned_qty(c.item_id.clone().unwrap_or_else(|| "undefined".to_string()), c.amount.unwrap_or(1.0)));
        }
    }
}

/// Gems a class puts in a slot, best first.
fn gem_picks(cls: dps_core::Class) -> &'static [&'static str] {
    match cls {
        dps_core::Class::Arch | dps_core::Class::Bers => &["JASPER", "ONYX"],
        dps_core::Class::Mage => &["SAPPHIRE"],
    }
}

struct Filled {
    gems: dps_core::model::Gems,
    parts: Vec<Part>,
    label: String,
}

/// JS `fillGems(item, ctx, prices)`.
fn fill_gems(id: &str, cls: dps_core::Class, resource: &Resource, prices: &PriceTables) -> Option<Filled> {
    let slots = resource.get(id).and_then(|i| i.gemstone_slots.as_ref())?;
    if slots.is_empty() {
        return None;
    }
    let mut counts: Vec<(String, usize)> = Vec::new();
    let mut gems = dps_core::model::Gems::default();
    let mut parts = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for slot in slots {
        let ty = slot.slot_type.as_str();
        let index = match counts.iter_mut().find(|(t, _)| t == ty) {
            Some((_, n)) => {
                *n += 1;
                *n
            }
            None => {
                counts.push((ty.to_string(), 0));
                0
            }
        };
        let key = format!("{ty}_{index}");
        let gem = gem_picks(cls).iter().copied().find(|&g| {
            let ok = match slot_gems(ty) {
                Some(list) => list.contains(&g),
                None => ty == g,
            };
            ok && gemstone(g).is_some()
        });
        let quality = gem.and_then(|g| ["PERFECT", "FLAWLESS"].into_iter().find(|q| prices.get(&format!("{q}_{g}_GEM")).is_some()));
        let (Some(gem), Some(quality)) = (gem, quality) else { continue };
        gems.slots.insert(key.clone(), Gem { gem: gem.to_string(), quality: quality.to_string() });
        parts.push(Part::owned(format!("{quality}_{gem}_GEM")));
        if let Some(costs) = &slot.costs {
            gems.unlocked.push(key);
            push_slot_costs(&mut parts, costs);
        }
        let name = format!("{} {}", title_lower(quality), title_lower(gem));
        if !names.contains(&name) {
            names.push(name);
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(Filled { gems, parts, label: format!("{} gems", names.join(", ")) })
}

impl<'a> B<'a> {
    fn cls(&self) -> dps_core::Class {
        self.ctx.cls
    }

    /// JS `push(label, parts, apply, extra)`: the edit is only built when the parts are priced.
    fn push(&mut self, label: String, parts: &[Part], extra: Extra, edit: impl FnOnce(&mut Model, &mut BagCache) -> Edit) {
        if let Some((cost, source)) = price_parts(parts, self.prices) {
            let edit = edit(self.model, &mut self.g.bag_cache);
            self.ups.push(Candidate {
                label: Arc::from(label),
                cost,
                source: Arc::from(source),
                unverified: extra.unverified,
                group: extra.group.map(Arc::from),
                stacks_on: extra.stacks_on,
                repick: extra.repick,
                armor_slot: extra.armor_slot,
                armor_id: extra.armor_id.map(Arc::from),
                bank: extra.bank,
                edit,
            });
        }
    }

    /// `pushItem`: a dual-wielded weapon buys every upgrade twice.
    fn push_item(&mut self, dual: bool, label: String, parts: Vec<Part>, extra: Extra, edit: impl FnOnce(&mut Model, &mut BagCache) -> Edit) {
        if dual {
            let parts: Vec<Part> = parts.iter().map(|p| p.doubled()).collect();
            self.push(format!("{label} (both copies)"), &parts, extra, edit);
        } else {
            self.push(label, &parts, extra, edit);
        }
    }

    /// Edit for `replaceItem(l, old, next)`: the bag is worked out for the loadout the list is built from.
    fn replace(l: &Loadout, model: &mut Model, bc: &mut BagCache, old: ItemId, copy: bool, next: ItemId, post: ReplacePost) -> Edit {
        let bag = bag_replace_cached(model, bc, l.accessories, old, next).map(|nb| (l.accessories, nb));
        Edit::Replace { old, next, copy, bag, post }
    }

    fn fresh_item(&mut self, ri: usize, id: &str) -> ItemId {
        if let Some(f) = self.g.fresh[ri] {
            return f;
        }
        let f = self.model.intern_item(new_item(id, self.resource));
        self.g.fresh[ri] = Some(f);
        f
    }

    fn has_useful_stats(&mut self, ri: usize, id: &str) -> bool {
        match self.g.useful[ri] {
            0 => {}
            1 => return false,
            _ => return true,
        }
        // baseItemStats(id, resource, cls) with the default cata level (Infinity) and floor mode (master).
        let b = dps_core_base_stats(id, self.resource, self.cls());
        let yes = [&b.global, &b.wither, &b.dragon].iter().any(|s| s.iter().any(|(_, v)| v != 0.0 && v != 1.0));
        self.g.useful[ri] = if yes { 2 } else { 1 };
        yes
    }

    fn boots_speed(&self, item: Option<ItemId>) -> f64 {
        match item {
            Some(i) => self.model.contribution(i, 0).global.get0(Stat::Speed),
            None => 0.0,
        }
    }

    fn boss_specific(&self, item: ItemId) -> bool {
        if self.model.flags(item).wi_weapon_id {
            return true;
        }
        let c = self.model.contribution(item, 0);
        c.wither.any_truthy() || c.dragon.any_truthy()
    }

    fn boots_allowed(&self, id: &str) -> bool {
        if !CLEAR_BOOTS_CLASSES.contains(&self.cls()) {
            return true;
        }
        !self.l.owned.contains(MAXOR_BOOTS_ID) || id == MAXOR_BOOTS_ID || id == NECRON_BOOTS_ID
    }

    fn run(&mut self) {
        let sections: [(&str, fn(&mut B<'a>)); 14] = [
            ("items", B::item_upgrades),
            ("spare", B::spare_blades),
            ("buys", B::buys),
            ("clearboots", B::clear_boots),
            ("shards", B::shards),
            ("enrichswap", B::enrichment_swap),
            ("bop", B::book_of_progression),
            ("bank", B::bank),
            ("powers", B::power_stones),
            ("scrolls", B::wither_scrolls),
            ("pets", B::pets),
            ("petitems", B::active_pet_items),
            ("essence", B::essence_perks),
            ("extras", B::extras),
        ];
        for (i, (_, f)) in sections.iter().enumerate() {
            let t = std::time::Instant::now();
            f(self);
            self.g.section_nanos[i] += t.elapsed().as_nanos() as u64;
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // Upgrades on owned items
    // -----------------------------------------------------------------------------------------------------

    fn item_upgrades(&mut self) {
        let l = self.l;
        let ctx = self.ctx;
        let resource = self.resource;
        let cls = ctx.cls;
        let max_mode = ctx.upgrade_mode == UpgradeMode::Max;
        for (item, copy) in l.upgradable_items(self.model) {
            // An accessory's candidates depend on nothing but the accessory bag: remember them.
            let cacheable = {
                let it = self.model.item(item);
                it.category.as_deref() == Some(ACCESSORY_CATEGORY) && fragged(&it.id).is_none() && item_upgrades(&it.id).is_none()
            };
            if cacheable {
                if let Some(templates) = self.g.acc_cache.get(&item) {
                    for t in templates {
                        let mut c = t.clone();
                        if let Edit::Replace { old, next, bag, .. } = &mut c.edit {
                            *bag = bag_replace_cached(self.model, &mut self.g.bag_cache, l.accessories, *old, *next).map(|nb| (l.accessories, nb));
                        }
                        self.ups.push(c);
                    }
                    continue;
                }
            }
            let start = self.ups.len();
            let st: ItemState = self.model.item(item).clone();
            let name = clean_name(&st.name);
            let slot = l.slot_name(self.model, item, copy);
            let cat = st.category.as_deref();
            let is_accessory = cat == Some(ACCESSORY_CATEGORY);
            let weapon_like = cat.is_some_and(is_weapon_category)
                && (Some(item) == l.weapon || Some(item) == l.rag || Some(item) == l.dragon_item(GearSlot::Weapon));
            let dual = st.dual_wield;
            let info = resource.get(&st.id);

            // Dual wield (LCM).
            if weapon_like
                && (copy || Some(item) != l.rag)
                && DUAL_WIELD_WEAPONS.contains(&st.id.as_str())
                && st.enchants.get("prosecute").is_some_and(|&v| v != 0)
                && !st.dual_wield
                && !ctx.is_rcm()
            {
                // A spare copy you own only needs the upgrades it is missing, listed in the label as one bundle.
                let spare = l.spare_blades.iter().copied().find(|&b| self.model.item(b).id == st.id);
                let (parts, label) = match spare {
                    Some(s) => {
                        let (_, parts, names) = convert_parts(&self.model.item(s).clone(), &st, self.prices, &[("prosecute", "execute")]);
                        let missing = if names.is_empty() { " (nothing missing)".to_string() } else { format!(" + {}", names.join(", ")) };
                        (parts, format!("Dual wield {name}: your spare one with Execute in place of Prosecute{missing}"))
                    }
                    None => {
                        let mut p = vec![Part::owned(st.id.clone())];
                        p.extend(copy_parts(&st, self.prices));
                        (p, format!("Dual wield {name}: a second copy with Execute in place of Prosecute"))
                    }
                };
                let parts = if parts.is_empty() { vec![Part::Coins(1.0)] } else { parts };
                self.push(label, &parts, group(format!("{slot}:dualwield")), |m, bc| {
                    let next = m.modify_item(item, |s| s.dual_wield = true);
                    let post = match spare {
                        Some(s) => ReplacePost::RemoveSpare(s),
                        None => ReplacePost::None,
                    };
                    Self::replace(l, m, bc, item, copy, next, post)
                });
            }

            // Master stars: next star only, dungeon gear with 5 regular stars.
            if !is_accessory
                && ctx.floor_mode != dps_core::FloorMode::Normal
                && self.model.flags(item).is_dungeon
                && st.stars >= MAX_STARS as i32
                && st.master_stars < MAX_MASTER_STARS as i32
            {
                let ms = st.master_stars;
                if max_mode {
                    let parts: Vec<Part> = MASTER_STAR_IDS.iter().skip(ms.max(0) as usize).take(MAX_MASTER_STARS as usize).map(|&i| Part::id(i)).collect();
                    let label = format!(
                        "{} on {name}",
                        if parts.len() > 1 {
                            format!("{}–{} Master Stars", ordinal(ms + 1), ordinal(MAX_MASTER_STARS as i32))
                        } else {
                            format!("{} Master Star", ordinal(MAX_MASTER_STARS as i32))
                        }
                    );
                    self.push_item(dual, label, parts, Extra::default(), |m, bc| {
                        let next = m.modify_item(item, |s| s.master_stars = MAX_MASTER_STARS as i32);
                        Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                    });
                } else {
                    let label = format!("{} Master Star on {name}", ordinal(ms + 1));
                    let part = MASTER_STAR_IDS.get(ms.max(0) as usize).map(|&i| Part::id(i)).unwrap_or(Part::owned("undefined".to_string()));
                    self.push_item(dual, label, vec![part], Extra::default(), |m, bc| {
                        let next = m.modify_item(item, |s| s.master_stars = ms + 1);
                        Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                    });
                }
            }

            // Enchants: any level above the current one.
            if !is_accessory {
                for ench in ENCHANTS {
                    let Some(c) = cat else { continue };
                    if !ench.categories.contains(&c) {
                        continue;
                    }
                    if ench.items.is_some_and(|items| !items.contains(&st.id.as_str())) {
                        continue;
                    }
                    if ench.classes.is_some_and(|cs| !cs.contains(&cls)) {
                        continue;
                    }
                    if ench.chimera && ctx.mode == Mode::Rcm {
                        continue;
                    }
                    // First Strike V on a Prosecute weapon stays: never swapped for Triple Strike (IV or less is).
                    let fs = st.enchants.get("first_strike").copied().unwrap_or(0);
                    if ench.key == "triple_strike" && fs >= 5 && st.enchants.get("prosecute").is_some_and(|&v| v != 0) {
                        continue;
                    }
                    let current = st.enchants.get(ench.key).copied().unwrap_or(0);
                    let max_level = ench.max_level();
                    let first = (current + 1).max(ench.min_level.unwrap_or(1));
                    let mut level = if max_mode { first.max(max_level) } else { first };
                    while level <= max_level {
                        let replaced: Vec<String> = ench
                            .conflicts
                            .iter()
                            .filter(|c| st.enchants.get(**c).is_some_and(|&v| v != 0))
                            .map(|c| match enchant(c).and_then(|e| e.name) {
                                Some(n) => n.to_string(),
                                None => title_case(c.strip_prefix("ultimate_").unwrap_or(c)),
                            })
                            .collect();
                        let base_name = match ench.name {
                            Some(n) => n.to_string(),
                            None => title_case(ench.key.strip_prefix("ultimate_").unwrap_or(ench.key)),
                        };
                        let mut label = format!("{base_name} {} on {name}", roman(level));
                        if !replaced.is_empty() {
                            label.push_str(&format!(" (replaces {})", replaced.join(", ")));
                        }
                        let way = enchant_parts(ench.key, level, current, self.prices);
                        level += 1;
                        let Some(way) = way else { continue };
                        let lv = level - 1;
                        let label = match &way.note {
                            Some(n) => format!("{label} ({n})"),
                            None => label,
                        };
                        let gname = format!("{slot}:enchant:{}", ench.key);
                        let extra = Extra { group: Some(gname.clone()), stacks_on: way.stacks.then(|| gname.clone()), ..Default::default() };
                        let key = ench.key;
                        self.push_item(dual, label, way.parts, extra, |m, bc| {
                            let next = m.modify_item(item, |s| {
                                s.enchants.insert(key.to_string(), lv);
                                for c in ench.conflicts {
                                    s.enchants.shift_remove(*c);
                                }
                            });
                            Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                        });
                    }
                }
            }

            // Hot potato books (all remaining), then fuming potato books, and The Art of War.
            if weapon_like {
                let hpb = st.hpb;
                let hot_max = HOT_POTATO.max as i32;
                let fuming_max = FUMING_POTATO.max as i32;
                if hpb < hot_max {
                    let n = hot_max - hpb;
                    let label = format!("{n} Hot Potato Book{} on {name}", if n > 1 { "s" } else { "" });
                    self.push_item(dual, label, vec![Part::qty(HOT_POTATO.id, n as f64)], Extra::default(), |m, bc| {
                        let next = m.modify_item(item, |s| s.hpb = hot_max);
                        Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                    });
                } else if hpb < hot_max + fuming_max {
                    let n = hot_max + fuming_max - hpb;
                    let label = format!("{n} Fuming Potato Book{} on {name}", if n > 1 { "s" } else { "" });
                    self.push_item(dual, label, vec![Part::qty(FUMING_POTATO.id, n as f64)], Extra::default(), |m, bc| {
                        let next = m.modify_item(item, |s| s.hpb = hot_max + fuming_max);
                        Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                    });
                }
                if !st.art_of_war {
                    self.push_item(dual, format!("The Art of War on {name}"), vec![Part::id(ART_OF_WAR_ID)], Extra::default(), |m, bc| {
                        let next = m.modify_item(item, |s| s.art_of_war = true);
                        Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                    });
                }
            }

            // Fragging.
            if let Some(frag) = fragged(&st.id) {
                if let Some(finfo) = resource.get(frag.id) {
                    let label = format!("Frag {name} ({})", parts_text(frag.parts));
                    let parts: Vec<Part> = frag.parts.iter().map(|p| Part::qty(p.id, p.qty)).collect();
                    let new_name = clean_name(finfo.name.as_deref().unwrap_or(&st.name));
                    self.push_item(dual, label, parts, Extra::default(), |m, bc| {
                        let next = m.modify_item(item, |s| {
                            s.id = frag.id.to_string();
                            s.name = new_name;
                            s.category = finfo.category.clone().or_else(|| s.category.take());
                            s.base_rarity = finfo.tier.clone().unwrap_or_else(|| std::mem::take(&mut s.base_rarity));
                        });
                        Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                    });
                }
            }

            // Golden -> Diamond heads and Wither armor -> class armor (keeps its upgrades).
            for up in item_upgrades(&st.id).unwrap_or(&[]) {
                let Some(uinfo) = resource.get(up.id) else { continue };
                if l.owned.contains(up.id) || ctx.excluded_item(up.id) || !meets_requirements(Some(uinfo), Some(self.player)) {
                    continue;
                }
                let label = format!("Upgrade {name} into {} ({})", clean_name(uinfo.name.as_deref().unwrap_or(up.id)), parts_text(up.parts));
                let parts: Vec<Part> = up.parts.iter().map(|p| Part::qty(p.id, p.qty)).collect();
                let new_name = clean_name(uinfo.name.as_deref().unwrap_or(&st.name));
                let base_rarity = drop_base_rarity(up.id, resource).to_string();
                self.push_item(dual, label, parts, Extra::default(), |m, bc| {
                    let next = m.modify_item(item, |s| {
                        s.id = up.id.to_string();
                        s.name = new_name;
                        s.category = uinfo.category.clone().or_else(|| s.category.take());
                        s.base_rarity = base_rarity;
                    });
                    Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                });
            }

            // Recombobulator.
            if can_recomb(&st, resource) {
                self.push_item(dual, format!("Recombobulate {name}"), vec![Part::id(RECOMBOBULATOR_ID)], Extra::default(), |m, bc| {
                    let next = m.modify_item(item, |s| s.recomb = true);
                    Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                });
            }

            // Reforges: stones, and blacksmith reforges.
            if !is_accessory {
                let rarity = stat_rarity(item_rarity_of(&st)).to_string();
                let mut rolls: Option<usize> = None;
                for reforge in all_reforges() {
                    if !reforge_allowed(reforge, &st.id, cat, ctx.mode) || (!has_dps_stats(reforge.stats_for(&rarity)) && reforge.cata_stat.is_none()) {
                        continue;
                    }
                    if same_reforge(&st.reforge, reforge) {
                        continue;
                    }
                    let label = format!("{} reforge on {name}{}", reforge.name, if reforge.stone.is_some() { "" } else { " (blacksmith)" });
                    let parts = reforge_cost_cached(reforge, &st.id, cat, &mut rolls);
                    let key = reforge.key;
                    self.push_item(dual, label, parts, group(format!("{slot}:reforge")), |m, bc| {
                        let next = m.modify_item(item, |s| s.reforge = Some(key.to_string()));
                        Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                    });
                }
            }

            // Gemstones.
            for up in gem_upgrades(&st, info, max_mode) {
                let label = format!(
                    "{} {} in {name}{}",
                    title_lower(up.quality),
                    title_lower(up.gem),
                    if up.unlock { " (unlock slot)" } else { "" }
                );
                let gname = format!("{slot}:gem:{}", up.slot_key);
                let (gem, quality, slot_key, unlocked) = (up.gem, up.quality, up.slot_key.clone(), up.unlocked_before);
                self.push_item(dual, label, up.parts, group(gname), |m, bc| {
                    let next = m.modify_item(item, |s| {
                        s.gems.slots.insert(slot_key.clone(), Gem { gem: gem.to_string(), quality: quality.to_string() });
                        if !unlocked {
                            s.gems.unlocked.push(slot_key.clone());
                        }
                    });
                    Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                });
            }

            // Enrichments on Legendary+ accessories.
            if is_accessory && ENRICHABLE_RARITIES.contains(&item_rarity_of(&st)) {
                for &kind in enrichment_kinds(cls, ctx.mode) {
                    if st.enrichment.as_deref() == Some(kind) {
                        continue;
                    }
                    let Some(en) = enrichment(kind) else { continue };
                    let label = format!("{} Enrichment on {name}", enrichment_name(kind));
                    self.push_item(dual, label, vec![Part::owned(format!("TALISMAN_ENRICHMENT_{}", en.key))], group(format!("{slot}:enrich")), |m, bc| {
                        let next = m.modify_item(item, |s| s.enrichment = Some(kind.to_string()));
                        Self::replace(l, m, bc, item, copy, next, ReplacePost::None)
                    });
                }
            }
            if cacheable {
                let list = self.ups[start..].to_vec();
                self.g.acc_cache.insert(item, list);
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // Spare blades
    // -----------------------------------------------------------------------------------------------------

    fn spare_blades(&mut self) {
        let l = self.l;
        let ctx = self.ctx;
        if ctx.is_rcm() {
            return;
        }
        let cls = ctx.cls;
        for &spare in l.spare_blades.iter() {
            let sst: ItemState = self.model.item(spare).clone();
            if !sst.category.as_deref().is_some_and(|c| class_weapon_categories(cls).contains(&c)) {
                continue;
            }
            let spare_name = clean_name(&sst.name);
            let mut targets: Vec<(bool, ItemId)> = Vec::new();
            if let Some(w) = l.weapon {
                if self.model.item(w).id != sst.id {
                    targets.push((false, w));
                }
            }
            if ctx.has_dragon_set() && l.dragon_item(GearSlot::Weapon).is_none() {
                if let Some(w) = l.weapon {
                    targets.push((true, w));
                }
            }
            for (dragon, src_id) in targets {
                let src: ItemState = self.model.item(src_id).clone();
                let (item_state, parts, _) = convert_parts(&sst, &src, self.prices, &[]);
                let label = format!(
                    "Use your spare {spare_name}{} with {}'s upgrades (missing ones only)",
                    if dragon { " as the dragon-set weapon" } else { " as the boss weapon" },
                    clean_name(&src.name)
                );
                let parts = if parts.is_empty() { vec![Part::Coins(1.0)] } else { parts };
                let extra = Extra { group: Some(format!("spare:{}", if dragon { "dragon" } else { "main" })), repick: Some(true), ..Default::default() };
                let weapon = l.weapon;
                self.push(label, &parts, extra, |m, _| {
                    let item = m.intern_item(item_state);
                    Edit::Spare { spare, item, dragon, weapon }
                });
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // New items from the auction house / bazaar
    // -----------------------------------------------------------------------------------------------------

    fn buys(&mut self) {
        let l = self.l;
        let ctx = self.ctx;
        let resource = self.resource;
        let cls = ctx.cls;
        let weapon_cats = class_weapon_categories(cls);
        let dragon_set = ctx.has_dragon_set();
        // Accessories in the bag by family, for `accessorySuperseded`.
        let mut by_family: FxHashMap<&'static str, Vec<(KindId, i64)>> = FxHashMap::default();
        for &k in self.model.bag(l.accessories).kinds.iter() {
            let name = self.model.kind_name(k);
            if let Some(fam) = accessory_family(name) {
                by_family.entry(fam).or_default().push((k, family_rank(drop_base_rarity(name, resource), name)));
            }
        }
        // Ids the worn items upgrade into are never bought fresh.
        let mut worn: Vec<ItemId> = Vec::new();
        worn.extend(l.weapon);
        worn.extend(l.rag);
        worn.extend(l.armor.values());
        worn.extend(l.equipment.values());
        worn.extend(l.dragon_items());
        let mut worn_into: FxHashSet<&'static str> = FxHashSet::default();
        for &w in &worn {
            let wid = self.model.item(w).id.as_str();
            if let Some(f) = fragged(wid) {
                worn_into.insert(f.id);
            }
            for u in item_upgrades(wid).unwrap_or(&[]) {
                worn_into.insert(u.id);
            }
        }
        for (ri, (id, info)) in resource.iter().enumerate() {
            let cat = info.category.as_deref();
            let is_rag = id == RAGNAROCK_ID;
            let is_weapon = cat.is_some_and(|c| weapon_cats.contains(&c));
            let armor_idx = cat.and_then(|c| ARMOR_SLOTS.iter().position(|&s| s == c));
            let equip_slot = cat.and_then(equipment_slot);
            let equip_idx = equip_slot.and_then(|e| EQUIPMENT_SLOTS.iter().position(|&s| s == e));
            let is_acc = cat == Some(ACCESSORY_CATEGORY);
            if !is_rag && !is_weapon && armor_idx.is_none() && equip_slot.is_none() && !is_acc {
                continue;
            }
            if l.owned.contains(id)
                || info.tier.as_deref().is_some_and(|t| UNBUYABLE_TIERS.contains(&t))
                || !meets_requirements(Some(info), Some(self.player))
            {
                continue;
            }
            if ctx.excluded_item(id) {
                continue;
            }
            // Never buy a fresh copy of what an item you wear upgrades into.
            if worn_into.contains(id.as_str()) {
                continue;
            }
            if is_acc {
                if let Some(fam) = accessory_family(id) {
                    let mine = family_rank(drop_base_rarity(id, resource), id);
                    let my_kind = self.model.find_kind(id);
                    if by_family.get(fam).is_some_and(|v| v.iter().any(|&(k, r)| Some(k) != my_kind && r >= mine)) {
                        continue;
                    }
                }
            }
            if !is_acc && !is_rag && !self.has_useful_stats(ri, id) && !(id == IMPLOSION_BELT_ID && ctx.mode == Mode::Rcm) {
                continue;
            }
            if cat == Some("BOOTS") && !self.boots_allowed(id) {
                continue;
            }

            let slot: Option<GearSlot> = if is_weapon && !is_rag {
                Some(GearSlot::Weapon)
            } else if let Some(a) = armor_idx {
                Some(GearSlot::Armor(a as u8))
            } else {
                equip_idx.map(|e| GearSlot::Equipment(e as u8))
            };
            let fid = self.fresh_item(ri, id);
            let mut sets = vec![BuySet::Main];
            let splits = dragon_set
                && slot.is_some()
                && {
                    let s = slot.unwrap();
                    self.boss_specific(fid)
                        || l.main_item(s).is_some_and(|m| self.boss_specific(m))
                        || l.dragon_item(s).is_some_and(|d| self.boss_specific(d))
                };
            if splits && (if is_weapon { !is_rag } else { true }) {
                sets.push(BuySet::Dragon);
                let s = slot.unwrap();
                if l.main_item(s).is_some() && l.dragon_item(s).is_none() {
                    sets.push(BuySet::Wither);
                }
            }
            let current: Option<ItemId> = if is_weapon && !is_rag {
                l.weapon
            } else if let Some(a) = armor_idx {
                l.armor.get(a)
            } else {
                None
            };
            let src: Option<ItemId> = current.filter(|&c| {
                let cs = self.model.item(c);
                cs.id != *id && (!cs.enchants.is_empty() || (is_weapon && cs.hpb != 0))
            });
            let meta = self.item_meta(ri, fid, is_acc);
            let (recombable, best) = (meta.recombable, meta.best);
            let bc = BuyCtx { id: id.as_str(), info, ri, fid, cat: cat.unwrap_or(""), is_rag, is_weapon, armor_idx, equip_idx, is_acc, slot, sets, src };
            let mut e = BuyExtra::new();
            e.full = best.is_none() && !recombable;
            self.buy(&bc, None, e);
            if recombable {
                let mut e = BuyExtra::new();
                e.recomb = true;
                e.full = best.is_none();
                self.buy(&bc, None, e);
            }
            if is_acc {
                // Enrichments need Legendary or higher, so most come after the recombobulator.
                for recomb in if recombable { vec![false, true] } else { vec![false] } {
                    if !meta.enrichable[recomb as usize] {
                        continue;
                    }
                    for &kind in enrichment_kinds(cls, ctx.mode) {
                        let mut e = BuyExtra::new();
                        e.recomb = recomb;
                        e.enrichment = Some(kind);
                        self.buy(&bc, None, e);
                    }
                }
            } else if let Some(best) = best {
                let mut e = BuyExtra::new();
                e.full = !recombable;
                self.buy(&bc, Some(best), e);
                if recombable {
                    let mut e = BuyExtra::new();
                    e.recomb = true;
                    e.full = true;
                    self.buy(&bc, Some(best), e);
                }
            }
            if !is_acc && info.gemstone_slots.as_ref().is_some_and(|s| !s.is_empty()) {
                let mut e = BuyExtra::new();
                e.recomb = recombable;
                e.full = true;
                e.gems = true;
                self.buy(&bc, best, e);
            }
        }
    }

    /// JS `buy(reforgeKeyName, reforge, extra)`.
    fn buy(&mut self, bc: &BuyCtx<'_>, reforge: Option<&'static Reforge>, extra: BuyExtra) {
        for &set in &bc.sets {
            // The dragon-set versions come only in the fullest bundle too.
            if set != BuySet::Main && !extra.full {
                continue;
            }
            self.buy_into(bc, set, reforge, extra);
            // The copied-enchant bundles come only in the fullest version (best reforge and recombobulator).
            if bc.src.is_some() && extra.full {
                for copy in [CopyKind::Cheap, CopyKind::All] {
                    let mut e = extra;
                    e.copy = copy;
                    self.buy_into(bc, set, reforge, e);
                }
            }
        }
    }

    /// `canRecomb`, `bestReforge` and the enrichable rarities of the fresh item `fid` (remembered per resource index;
    /// `bestReforge` is cached for the whole run in the JS too, so its answer never changes).
    fn item_meta(&mut self, ri: usize, fid: ItemId, is_acc: bool) -> ItemMeta {
        if let Some(m) = self.g.meta[ri] {
            return m;
        }
        let fresh_state: ItemState = self.model.item(fid).clone();
        let recombable = can_recomb(&fresh_state, self.resource);
        let best = if is_acc { None } else { self.best_reforge(&fresh_state) };
        let enrichable = [false, true].map(|r| ENRICHABLE_RARITIES.contains(&item_rarity(&fresh_state.base_rarity, r, &fresh_state.id)));
        let m = ItemMeta { recombable, best, enrichable };
        self.g.meta[ri] = Some(m);
        m
    }

    fn buy_into(&mut self, bc: &BuyCtx<'_>, set: BuySet, reforge: Option<&'static Reforge>, extra: BuyExtra) {
        let l = self.l;
        let keep_main = bc.slot.and_then(|s| l.main_item(s));
        let key = BuyKey {
            ri: bc.ri as u32,
            set: set as u8,
            reforge: reforge.map_or(0, |r| r.index as u16 + 1),
            recomb: extra.recomb,
            enrichment: extra.enrichment.map_or(0, |k| ENRICHMENTS.iter().position(|e| e.kind == k).map_or(0, |i| i as u8 + 1)),
            gems: extra.gems,
            copy: extra.copy as u8,
            src: if extra.copy != CopyKind::None { bc.src.map_or(0, |s| s.raw()) } else { 0 },
            keep: if set == BuySet::Wither { keep_main.map_or(0, |k| k.raw()) } else { 0 },
            cloak: extra.copy == CopyKind::All && l.equipment.get(CLOAK).is_some_and(|c| SA_CLOAK_IDS.contains(&self.model.item(c).id.as_str())),
        };
        if !self.g.buy_cache.contains_key(&key) {
            let t = self.make_buy_template(bc, set, reforge, extra);
            self.g.buy_cache.insert(key, t);
        }
        let Some(t) = self.g.buy_cache.get(&key).and_then(|t| t.as_ref()) else { return };
        let bag = if bc.is_acc {
            let cache = &mut self.g.bag_with_cache;
            let model = &mut *self.model;
            let nb = *cache.entry((l.accessories, t.item)).or_insert_with(|| model.bag_with(l.accessories, t.item));
            Some((l.accessories, nb))
        } else {
            None
        };
        self.ups.push(Candidate {
            label: t.label.clone(),
            cost: t.cost,
            source: t.source.clone(),
            unverified: None,
            group: t.group.clone(),
            stacks_on: None,
            repick: Some(t.repick),
            armor_slot: t.armor_slot,
            armor_id: t.armor_id.clone(),
            bank: false,
            edit: Edit::Buy(BuyEdit {
                item: t.item,
                dragon: set == BuySet::Dragon,
                wither: set == BuySet::Wither,
                slot: bc.slot,
                is_rag: bc.is_rag,
                is_weapon: bc.is_weapon,
                armor: bc.armor_idx.map(|a| a as u8),
                equipment: bc.equip_idx.map(|e| e as u8),
                bag: if bc.is_rag || bc.is_weapon || bc.armor_idx.is_some() || bc.equip_idx.is_some() { None } else { bag },
                keep_main,
            }),
        });
    }

    /// The part of `buyInto` that does not depend on the accessory bag: label, price, the bought item.
    fn make_buy_template(&mut self, bc: &BuyCtx<'_>, set: BuySet, reforge: Option<&'static Reforge>, extra: BuyExtra) -> Option<BuyTemplate> {
        let l = self.l;
        let ctx = self.ctx;
        let resource = self.resource;
        let id = bc.id;
        let info = bc.info;
        let mut base: ItemState = self.model.item(bc.fid).clone();
        base.reforge = reforge.map(|r| r.key.to_string());
        let free = free_item(id);
        let free_up = free_upgrade(id);
        let mut parts: Vec<Part> = Vec::new();
        if free.is_some() {
            parts.push(Part::Coins(1.0));
        } else if let Some((_, fp)) = free_up {
            parts.push(Part::Coins(1.0));
            parts.extend(fp.iter().map(|p| Part::qty(p.id, p.qty)));
        } else {
            parts.push(Part::owned(buy_price_id(id, resource, ctx.cata_level).into_owned()));
        }
        if let Some(r) = reforge {
            parts.extend(reforge_cost(r, &base.id, base.category.as_deref()));
        }
        let display = clean_name(info.name.as_deref().unwrap_or(id));
        let mut first = match (free_up, free) {
            (Some((from, fp)), _) => {
                let from_name = clean_name(self.resource.get(from).and_then(|i| i.name.as_deref()).unwrap_or(from));
                let note = free_item(from).unwrap_or("");
                format!("Claim {from_name} ({note}) and upgrade it into {display} ({})", parts_text(fp))
            }
            (None, Some(note)) => format!("Claim {display} ({note})"),
            (None, None) => format!("Buy {display}"),
        };
        match set {
            BuySet::Dragon => first.push_str(" (dragon set)"),
            BuySet::Wither => {
                let m = l.main_item(bc.slot.expect("wither set has a slot")).expect("wither set keeps a main piece");
                first.push_str(&format!(" (for withers, keep {} for dragons)", clean_name(&self.model.item(m).name)));
            }
            BuySet::Main => {}
        }
        let mut labels: Vec<String> = vec![first];
        if let Some(r) = reforge {
            labels.push(format!("{} reforge", r.name));
        }
        // RCM: a bought blade comes with the 3 Wither scrolls.
        if ctx.mode == Mode::Rcm && WITHER_IMPACT_WEAPONS.contains(&id) {
            base.scrolls = WITHER_SCROLLS.iter().map(|s| s.to_string()).collect();
            parts.extend(WITHER_SCROLLS.iter().map(|&x| Part::id(x)));
            labels.push("Wither scrolls".to_string());
        }
        if extra.recomb {
            parts.push(Part::id(RECOMBOBULATOR_ID));
            labels.push("Recombobulator".to_string());
        }
        // Gem slots filled (unlocked when needed).
        if extra.gems {
            let filled = fill_gems(id, ctx.cls, resource, self.prices)?;
            base.gems = filled.gems;
            parts.extend(filled.parts);
            labels.push(filled.label);
        }
        if extra.copy != CopyKind::None {
            let src: ItemState = self.model.item(bc.src.expect("copy needs a source")).clone();
            base.enchants.clear();
            for (key, &level) in &src.enchants {
                let ench = enchant(key);
                let way = enchant_parts(key, level, 0, self.prices);
                let price = way.as_ref().and_then(|w| parts_cost(&w.parts, self.prices));
                let ok_cat = ench.is_some_and(|e| e.categories.contains(&bc.cat));
                let ok_item = ench.is_none_or(|e| e.items.is_none_or(|items| items.contains(&id)));
                let price_ok = price.is_some_and(|p| p != 0.0 && !p.is_nan());
                if !ok_cat || !ok_item || !price_ok || (extra.copy == CopyKind::Cheap && price.unwrap() > COPY_BOOK_MAX) {
                    continue;
                }
                base.enchants.insert(key.clone(), level);
                parts.extend(way.unwrap().parts);
            }
            // Potato books only matter on weapons.
            let hpb = if bc.is_weapon { src.hpb } else { 0 };
            base.hpb = hpb;
            let hot_max = HOT_POTATO.max as i32;
            if hpb != 0 {
                parts.push(Part::qty(HOT_POTATO.id, hpb.min(hot_max) as f64));
            }
            if hpb > hot_max {
                parts.push(Part::qty(FUMING_POTATO.id, (hpb - hot_max) as f64));
            }
            if src.art_of_war {
                base.art_of_war = true;
                parts.push(Part::id(ART_OF_WAR_ID));
            }
            if extra.copy == CopyKind::All && base.dungeon.unwrap_or(false) {
                base.master_stars = src.master_stars;
                for x in MASTER_STAR_IDS.iter().take(base.master_stars.max(0) as usize) {
                    parts.push(Part::id(x));
                }
            }
            // A full Necron's Blade variant with a Shadow Assassin Cloak worn also comes with the 3 Wither scrolls.
            if extra.copy == CopyKind::All
                && WITHER_IMPACT_WEAPONS.contains(&id)
                && base.scrolls.is_empty()
                && l.equipment.get(CLOAK).is_some_and(|c| SA_CLOAK_IDS.contains(&self.model.item(c).id.as_str()))
            {
                base.scrolls = WITHER_SCROLLS.iter().map(|s| s.to_string()).collect();
                parts.extend(WITHER_SCROLLS.iter().map(|&x| Part::id(x)));
                labels.push("Wither scrolls (Shadow Assassin Cloak ability)".to_string());
            }
            labels.push(format!(
                "{}{} like {}",
                if extra.copy == CopyKind::Cheap { "enchants (books up to 10M)" } else { "all enchants, master stars" },
                if bc.is_weapon { " and potato books" } else { "" },
                clean_name(&src.name)
            ));
            // Replacing a dual-wielded weapon: the full copy comes as a dual-wield pair too.
            if extra.copy == CopyKind::All
                && src.dual_wield
                && DUAL_WIELD_WEAPONS.contains(&id)
                && base.enchants.get("prosecute").is_some_and(|&v| v != 0)
            {
                base.dual_wield = true;
                let copy = parts.clone();
                parts.extend(copy);
                labels.push("dual wield (2 copies)".to_string());
            }
        }
        if let Some(kind) = extra.enrichment {
            let en = enrichment(kind).expect("known enrichment");
            parts.push(Part::owned(format!("TALISMAN_ENRICHMENT_{}", en.key)));
            labels.push(format!("{} Enrichment", enrichment_name(kind)));
        }
        let (cost, source) = price_parts(&parts, self.prices)?;
        let group = if bc.is_acc {
            None
        } else {
            Some(format!(
                "buy:{}{}",
                if set == BuySet::Dragon { "dragon:" } else { "" },
                if bc.is_rag {
                    "rag".to_string()
                } else if bc.is_weapon {
                    "weapon".to_string()
                } else {
                    match bc.equip_idx {
                        Some(e) => EQUIPMENT_SLOTS[e].to_string(),
                        None => bc.cat.to_string(),
                    }
                }
            ))
        };
        let (armor_slot, armor_id) = if bc.armor_idx.is_some() && set == BuySet::Main {
            (ARMOR_SLOTS.iter().copied().find(|&s| s == bc.cat), Some(id.to_string()))
        } else {
            (None, None)
        };
        base.recomb = extra.recomb;
        base.enrichment = extra.enrichment.map(|s| s.to_string());
        let item = self.model.intern_item(base);
        Some(BuyTemplate { label: Arc::from(labels.join(" + ")), cost, source: Arc::from(source), item, group: group.map(Arc::from), repick: bc.is_weapon, armor_slot, armor_id: armor_id.map(Arc::from) })
    }

    /// JS `bestReforge(loadout, ctx, item)`: best reforge for a fresh item of this category / rarity, tested in the
    /// current loadout. Cached per run.
    fn best_reforge(&mut self, item: &ItemState) -> Option<&'static Reforge> {
        let rarity = stat_rarity(item_rarity_of(item)).to_string();
        let dungeon = is_dungeon(item, self.resource);
        let key = format!("{}:{}:{}", item.category.as_deref().unwrap_or("undefined"), rarity, dungeon);
        if let Some(c) = self.g.reforge_cache.get(&key) {
            return c.map(|i| &all_reforges()[i]);
        }
        let cat = item.category.as_deref();
        let slot_name: &str = match cat {
            Some(c) if ARMOR_SLOTS.contains(&c) => c,
            Some(c) => equipment_slot(c).unwrap_or("weapon"),
            None => "weapon",
        };
        let slot = Slot::from_name(slot_name).expect("slot");
        let mut dummy = item.clone();
        dummy.id = "__REFORGE_TEST__".to_string();
        dummy.name = "test".to_string();
        let mut best: Option<usize> = None;
        let mut best_score = f64::NEG_INFINITY;
        for reforge in all_reforges() {
            if !reforge_allowed(reforge, &dummy.id, dummy.category.as_deref(), self.ctx.mode) {
                continue;
            }
            if !has_dps_stats(reforge.stats_for(&rarity)) && reforge.cata_stat.is_none() {
                continue;
            }
            let mut it = dummy.clone();
            it.reforge = Some(reforge.key.to_string());
            let id = self.model.intern_item(it);
            let mut trial = self.l.clone();
            trial.set_slot(slot, id);
            let r = Evaluator::new(self.model, self.ctx, self.weights).evaluate(&trial);
            let score = r.damage[0] + r.damage[1];
            if score > best_score {
                best_score = score;
                best = Some(reforge.index);
            }
        }
        self.g.reforge_cache.insert(key, best);
        best.map(|i| &all_reforges()[i])
    }

    // -----------------------------------------------------------------------------------------------------
    // Everything after the item loops
    // -----------------------------------------------------------------------------------------------------

    fn clear_boots(&mut self) {
        let l = self.l;
        let ctx = self.ctx;
        if !(CLEAR_BOOTS_CLASSES.contains(&ctx.cls) && ctx.mode != Mode::Rcm) {
            return;
        }
        let resource = self.resource;
        let clear_speed = self.boots_speed(l.clear_boots);
        for (ri, (id, info)) in resource.iter().enumerate() {
            if info.category.as_deref() != Some("BOOTS") || l.owned.contains(id) || !meets_requirements(Some(info), Some(self.player)) {
                continue;
            }
            let boots = self.fresh_item(ri, id);
            if self.boots_speed(Some(boots)) <= clear_speed {
                continue;
            }
            let label = format!("Buy {} (clear boots for speed)", clean_name(info.name.as_deref().unwrap_or(id)));
            let part = Part::owned(buy_price_id(id, resource, ctx.cata_level).into_owned());
            self.push(label, &[part], group("buy:clearBoots".to_string()), |_, _| Edit::ClearBoots(boots));
        }
    }

    fn shards(&mut self) {
        let l = self.l;
        let cls = self.ctx.cls;
        for shard in SHARDS {
            if shard.classes.is_some_and(|c| !c.contains(&cls)) {
                continue;
            }
            let hunting = self.ctx.skills.as_ref().and_then(|s| s.get("HUNTING")).copied();
            if let Some(h) = hunting {
                if (h as f64) < shard_hunting_req(shard.rarity).unwrap_or(0.0) {
                    continue;
                }
            }
            let current = l.shards.shard_level(shard.id);
            if current >= SHARD_MAX_LEVEL {
                continue;
            }
            let Some(cum) = shards_cumulative(shard.rarity) else { continue };
            let qty = cum[SHARD_MAX_LEVEL as usize - 1] - if current != 0.0 { cum.get((current as usize).wrapping_sub(1)).copied().unwrap_or(f64::NAN) } else { 0.0 };
            let label = format!("{} Shard → level {} ({} shards)", shard.name, SHARD_MAX_LEVEL, jsfmt::js_string(qty));
            let id = shard.id;
            self.push(label, &[Part::owned_qty(format!("SHARD_{id}"), qty)], Extra::default(), |_, _| Edit::Shards(l.shards.with(id, SHARD_MAX_LEVEL)));
        }
    }

    fn enrichment_swap(&mut self) {
        let l = self.l;
        let ctx = self.ctx;
        let resource = self.resource;
        let enriched: Vec<ItemId> = self
            .model
            .bag(l.accessories)
            .items
            .iter()
            .copied()
            .filter(|&a| self.model.item(a).enrichment.is_some())
            .collect();
        let is_dungeon_acc = |m: &Model, a: ItemId| resource.get(&m.item(a).id).is_some_and(|i| i.is_dungeon_item());
        for &kind in enrichment_kinds(ctx.cls, ctx.mode) {
            let mut seen: Vec<String> = Vec::new();
            for (group_name, keep) in [("all", 0u8), ("dungeon", 1), ("non-dungeon", 2)] {
                let targets: Vec<ItemId> = enriched
                    .iter()
                    .copied()
                    .filter(|&a| {
                        let k = match keep {
                            0 => true,
                            1 => is_dungeon_acc(self.model, a),
                            _ => !is_dungeon_acc(self.model, a),
                        };
                        k && self.model.item(a).enrichment.as_deref() != Some(kind)
                    })
                    .collect();
                let mut ids: Vec<&str> = targets.iter().map(|&a| self.model.item(a).id.as_str()).collect();
                ids.sort_unstable();
                let key = ids.join(",");
                if targets.is_empty() || seen.contains(&key) {
                    continue;
                }
                seen.push(key);
                let which = if group_name == "all" { "all".to_string() } else { format!("the {group_name}") };
                let label = format!(
                    "Swap {which} enriched accessories ({}) to {} (1 Accessory Enrichment Swapper{})",
                    targets.len(),
                    enrichment_name(kind),
                    if group_name == "all" { "" } else { ", take the others out first" }
                );
                self.push(label, &[Part::id(ENRICHMENT_SWAPPER_ID)], group("enrichswap".to_string()), |m, bc| {
                    let mut bag = l.accessories;
                    let mut owned = Vec::with_capacity(targets.len());
                    for &t in &targets {
                        let kind_id = m.entry(t).kind;
                        let cur = m.bag_get(bag, kind_id).unwrap_or(t);
                        let next = m.modify_item(cur, |s| s.enrichment = Some(kind.to_string()));
                        if let Some(nb) = bag_replace_cached(m, bc, bag, cur, next) {
                            bag = nb;
                        }
                        owned.push(next);
                    }
                    Edit::SetBag { base: l.accessories, bag, owned }
                });
            }
        }
    }

    fn book_of_progression(&mut self) {
        let l = self.l;
        let ctx = self.ctx;
        let sb_level = ctx.profile_stats.as_ref().map(|p| p.sb_level as f64).unwrap_or(0.0);
        let has_book = self.model.find_kind(BOOK_OF_PROGRESSION_ID).is_some_and(|k| self.model.bag_get(l.accessories, k).is_some());
        if has_book && ctx.profile_stats.is_some() && sb_level >= BOOK_DAMAGE_UNLOCK_LEVEL && l.bop_bonus.as_deref() != Some(BOOK_DAMAGE_OPTION) {
            self.push(
                "Set Book of Progression to Extra Infliction (damage)".to_string(),
                &[Part::Coins(BOOK_SWAP_COINS_PER_LEVEL * sb_level)],
                group("bop".to_string()),
                |_, _| Edit::Bop(std::sync::Arc::from(BOOK_DAMAGE_OPTION)),
            );
        }
    }

    fn bank(&mut self) {
        let l = self.l;
        let bank = self.model.bank_value(l.bank);
        let has_gd = l.pets.ids.iter().any(|&p| self.model.pet(p).kind == "GOLDEN_DRAGON");
        if has_gd && bank < GOLDEN_DRAGON_BANK_CAP {
            let next = dps_core::jsmath::min(GOLDEN_DRAGON_BANK_CAP, bank + BANK_STEP);
            let extra = Extra { bank: true, ..Default::default() };
            self.push(BANK_LABEL.to_string(), &[Part::Coins(BANK_STEP)], extra, |m, _| Edit::Bank(m.bank_id(next)));
        }
    }

    fn power_stones(&mut self) {
        let l = self.l;
        for &(power, stone) in POWER_STONES {
            let Some(pi) = POWERS.iter().position(|p| p.key == power) else { continue };
            if l.unlocked_powers.contains(&(pi as u8)) {
                continue;
            }
            let stone_name = clean_name(self.resource.get(stone).and_then(|i| i.name.as_deref()).unwrap_or(stone));
            let label = format!("Learn {} power ({} {})", title_case(power), jsfmt::js_string(POWER_STONES_TO_LEARN), stone_name);
            self.push(label, &[Part::qty(stone, POWER_STONES_TO_LEARN)], group("power".to_string()), |_, _| Edit::LearnPower(pi as u8));
        }
    }

    fn wither_scrolls(&mut self) {
        let l = self.l;
        let ctx = self.ctx;
        // The dragon weapon may be a byte-identical copy of the main one (a different object in the JS).
        let dragon = l.dragon_item(GearSlot::Weapon);
        let targets: Vec<(Option<ItemId>, bool)> = if ctx.mode == Mode::Rcm && ctx.cls == dps_core::Class::Mage {
            vec![(l.weapon, false)]
        } else {
            vec![(l.weapon, false), (dragon, dragon.is_some() && dragon == l.weapon)]
        };
        for (blade, copy) in targets.into_iter().filter_map(|(b, c)| b.map(|b| (b, c))) {
            let st: ItemState = self.model.item(blade).clone();
            if !WITHER_IMPACT_WEAPONS.contains(&st.id.as_str()) || self.model.flags(blade).has_wi {
                continue;
            }
            let missing: Vec<&'static str> = WITHER_SCROLLS.iter().copied().filter(|x| !st.scrolls.iter().any(|s| s == x)).collect();
            let names: Vec<&str> =
                missing.iter().map(|x| SCROLL_NAMES.iter().find(|(k, _)| k == x).map(|&(_, n)| n).unwrap_or("undefined")).collect();
            let label = format!(
                "{} on {}{}",
                names.join(", "),
                clean_name(&st.name),
                if ctx.mode == Mode::Rcm { "" } else { " (Shadow Assassin Cloak ability)" }
            );
            let parts: Vec<Part> = missing.iter().map(|&x| Part::id(x)).collect();
            self.push(label, &parts, group(format!("scrolls:{}", st.id)), |m, bc| {
                let done = m.modify_item(blade, |s| {
                    for x in WITHER_SCROLLS {
                        if !s.scrolls.iter().any(|y| y == x) {
                            s.scrolls.push(x.to_string());
                        }
                    }
                });
                Self::replace(l, m, bc, blade, copy, done, ReplacePost::Scrolls)
            });
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // Pets
    // -----------------------------------------------------------------------------------------------------

    /// The six stats of PET_ITEM_STATS for `petStats(pet, ctx.petProfile)`.
    fn pet_gain_stats(&mut self, pet: &Pet) -> [f64; 6] {
        if let Some(v) = self.g.pet_gain_cache.get(pet) {
            return *v;
        }
        let s = pet_stats(pet, &self.ctx.pet_profile);
        let mut out = [0.0; 6];
        for (i, k) in PET_ITEM_STATS.iter().enumerate() {
            out[i] = s.stats.get(*k).unwrap_or(0.0);
        }
        self.g.pet_gain_cache.insert(pet.clone(), out);
        out
    }

    fn raises(&mut self, before: &[f64; 6], pet: &Pet) -> bool {
        let after = self.pet_gain_stats(pet);
        (0..6).any(|i| after[i] > before[i])
    }

    /// JS `dpsPets([pet], ctx).length > 0`.
    fn pet_is_useful(&self, pet: &Pet) -> bool {
        if EXCLUDED_PETS.contains(&pet.kind.as_str()) {
            return false;
        }
        const USEFUL: [Stat; 8] = [
            Stat::Strength,
            Stat::CritDamage,
            Stat::CritChance,
            Stat::Intelligence,
            Stat::AdditiveDamage,
            Stat::AdditiveStrength,
            Stat::AttackSpeed,
            Stat::AbilityDamage,
        ];
        let s = pet_stats(pet, &self.ctx.pet_profile);
        USEFUL.iter().any(|&k| s.stats.get(k).is_some_and(|v| v != 0.0 && !v.is_nan())) || !s.dragon.is_empty() || !s.wither.is_empty()
    }

    fn build_pet_buys(&mut self) {
        let mut out: Vec<PetBuyType> = Vec::new();
        for kind in all_pet_types() {
            let Some(tier) = BUY_RARITIES.iter().copied().find(|t| pet_tiers(kind).any(|x| x == *t)) else { continue };
            let pet = Pet { kind: kind.to_string(), tier: tier.to_string(), held_item: None };
            if !self.pet_is_useful(&pet) {
                continue;
            }
            let pet_name = title_lower(kind);
            let level = jsfmt::js_string(max_pet_level(kind));
            let mut variants = Vec::new();
            let pid = self.model.pet_id(&pet);
            variants.push(PetVariant {
                id: pid,
                label: format!("Buy {pet_name} Pet ({}, level {level})", title_lower(tier)),
                parts: vec![Part::owned(format!("PET_{kind}_{tier}_MAX"))],
            });
            // Also bundled with each pet item that raises its stats.
            let bare = self.pet_gain_stats(&pet);
            for item in PET_ITEMS {
                let held = Pet { kind: kind.to_string(), tier: tier.to_string(), held_item: Some(item.id.to_string()) };
                if !self.raises(&bare, &held) {
                    continue;
                }
                let hid = self.model.pet_id(&held);
                variants.push(PetVariant {
                    id: hid,
                    label: format!("Buy {pet_name} Pet ({}, level {level}) + {}", title_lower(tier), item.name),
                    parts: vec![Part::owned(format!("PET_{kind}_{tier}_MAX")), Part::id(item.id)],
                });
            }
            // The rarity below plus a Tier Boost plays at the same rarity, often for less.
            let below = pet_tiers(kind).find(|&t| effective_tier(t, Some(TIER_BOOST_ID)) == tier && t != tier);
            if let Some(below) = below {
                let bp = Pet { kind: kind.to_string(), tier: below.to_string(), held_item: Some(TIER_BOOST_ID.to_string()) };
                let bid = self.model.pet_id(&bp);
                variants.push(PetVariant {
                    id: bid,
                    label: format!("Buy {pet_name} Pet ({}, level {level}) + Tier Boost", title_lower(below)),
                    parts: vec![Part::owned(format!("PET_{kind}_{below}_MAX")), Part::id(TIER_BOOST_ID)],
                });
            }
            out.push(PetBuyType { kind, variants });
        }
        self.g.pet_buys = Some(out);
    }

    fn pets(&mut self) {
        let l = self.l;
        let ctx = self.ctx;
        if self.g.pet_buys.is_none() {
            self.build_pet_buys();
        }
        let owned_types: FxHashSet<&str> = l.pets.ids.iter().map(|&p| self.model.pet(p).kind.as_str()).collect();
        // RCM: Golden Dragon is the clear pet for everyone, so an owner gets no other pet suggestions.
        let rcm_has_gd = ctx.mode == Mode::Rcm && owned_types.contains("GOLDEN_DRAGON");
        if rcm_has_gd {
            return;
        }
        let owned_types: Vec<String> = owned_types.into_iter().map(String::from).collect();
        let buys = self.g.pet_buys.take().expect("pet buys");
        for t in &buys {
            if owned_types.iter().any(|o| o == t.kind) || (ctx.mode == Mode::Rcm && RCM_SKIPPED_PETS.contains(&t.kind)) {
                continue;
            }
            for v in &t.variants {
                let pid = v.id;
                self.push(v.label.clone(), &v.parts, group("pet".to_string()), |_, _| Edit::BuyPet(pid));
            }
        }
        self.g.pet_buys = Some(buys);
    }

    fn active_pet_items(&mut self) {
        let l = self.l;
        let mut actives: Vec<PetId> = Vec::new();
        for a in l.active_pets.iter().flatten() {
            if !actives.contains(a) {
                actives.push(*a);
            }
        }
        for active_id in actives {
            let active: Pet = self.model.pet(active_id).clone();
            let before = self.pet_gain_stats(&active);
            for item in PET_ITEMS {
                if active.held_item.as_deref() == Some(item.id) {
                    continue;
                }
                let swapped = Pet { held_item: Some(item.id.to_string()), ..active.clone() };
                if !self.raises(&before, &swapped) {
                    continue;
                }
                let label = format!("{} on {} Pet", item.name, title_lower(&active.kind));
                let to = self.model.pet_id(&swapped);
                self.push(label, &[Part::id(item.id)], group(format!("petitem:{}", active.kind)), |_, _| Edit::SwapPet { from: active_id, to });
            }
            // A Tier Boost pet loses its rarity when the Tier Boost comes off, so a new pet item can come with a Kat
            // upgrade to that rarity.
            let kat = if active.held_item.as_deref() == Some(TIER_BOOST_ID) { kat_upgrade(&active.kind, &active.tier) } else { None };
            if let Some(kat) = kat {
                let level = max_pet_level(&active.kind);
                let mut kat_parts = vec![Part::Coins(jsfmt::math_round(kat.coins * (1.0 - KAT_LEVEL_DISCOUNT * level)))];
                kat_parts.extend(kat.items.iter().map(|p| Part::qty(p.id, p.qty)));
                kat_parts.push(Part::id(KAT_FLOWER_ID));
                for item in PET_ITEMS {
                    let label = format!("Kat upgrade {} Pet to {} + {}", title_lower(&active.kind), title_lower(kat.to), item.name);
                    let mut parts = kat_parts.clone();
                    parts.push(Part::id(item.id));
                    let np = Pet { kind: active.kind.clone(), tier: kat.to.to_string(), held_item: Some(item.id.to_string()) };
                    let to = self.model.pet_id(&np);
                    self.push(label, &parts, group(format!("petitem:{}", active.kind)), |_, _| Edit::SwapPet { from: active_id, to });
                }
            }
        }
    }

    fn essence_perks(&mut self) {
        let l = self.l;
        let ctx = self.ctx;
        if !(ctx.cata_level >= ESSENCE_MIN_CATA) {
            return;
        }
        for perk in ESSENCE_PERKS {
            if perk.profile_only || perk.pet.is_some_and(|p| !l.pets.ids.iter().any(|&x| self.model.pet(x).kind == p)) {
                continue;
            }
            let current = l.perks.get(perk.key).unwrap_or(0.0);
            let n = perk.costs.len() as f64;
            let mut level = if ctx.upgrade_mode == UpgradeMode::Max { (current + 1.0).max(n) } else { current + 1.0 };
            while level <= n {
                // perk.costs.slice(current, level).reduce((a, b) => a + b, 0)
                let from = (current.max(0.0) as usize).min(perk.costs.len());
                let to = (level.max(0.0) as usize).min(perk.costs.len());
                let mut qty = 0.0;
                for c in perk.costs.iter().take(to).skip(from) {
                    qty += c;
                }
                let label = format!(
                    "{} {} ({} {} Essence)",
                    perk.name,
                    roman(level as i32),
                    jsfmt::to_locale_string(qty),
                    title_lower(perk.essence)
                );
                let key = perk.key;
                let lv = level;
                self.push(label, &[Part::owned_qty(format!("ESSENCE_{}", perk.essence), qty)], group(format!("perk:{key}")), |_, _| {
                    Edit::Perks(l.perks.with(key, lv))
                });
                level += 1.0;
            }
        }
    }

    fn extras(&mut self) {
        let l = self.l;
        let cls = self.ctx.cls;
        for (i, entry) in EXTRA_UPGRADES.iter().enumerate() {
            if l.extras.has_index(i) {
                continue;
            }
            if entry.classes.is_some_and(|c| !c.contains(&cls)) {
                continue;
            }
            if let Some(req) = entry.requires {
                if !l.extras.has(req) {
                    continue;
                }
            }
            if entry.cost.iter().any(|p| !meets_requirements(self.resource.get(p.id), Some(self.player))) {
                continue;
            }
            let parts: Vec<Part> = entry.cost.iter().map(|p| Part::qty(p.id, p.qty.unwrap_or(1.0))).collect();
            let owned = entry.owned_items.is_some();
            let label = format!("{}{}", entry.name, if owned { "" } else { " (?)" });
            let extra = Extra {
                unverified: Some(!owned),
                group: entry.group.filter(|g| !g.is_empty()).map(|g| format!("extra:{g}")),
                ..Default::default()
            };
            self.push(label, &parts, extra, |_, _| Edit::Extra(i as u8));
        }
    }
}

struct BuyCtx<'a> {
    id: &'a str,
    info: &'a ItemInfo,
    ri: usize,
    fid: ItemId,
    cat: &'a str,
    is_rag: bool,
    is_weapon: bool,
    armor_idx: Option<usize>,
    equip_idx: Option<usize>,
    is_acc: bool,
    slot: Option<GearSlot>,
    sets: Vec<BuySet>,
    src: Option<ItemId>,
}

/// `baseItemStats(id, resource, cls)` with the JS defaults (cata level Infinity, floor mode master).
fn dps_core_base_stats(id: &str, resource: &Resource, cls: dps_core::Class) -> crate::arena::Triple {
    crate::itemstats::compute_base_stats(id, resource, Some(cls), f64::INFINITY, dps_core::FloorMode::Master)
}

/// The levels object after `{ ...shards, [id]: level }` (kept for callers that build shard edits themselves).
pub fn shards_with(l: &Loadout, id: &str, level: f64) -> std::sync::Arc<Levels> {
    l.shards.with(id, level)
}
