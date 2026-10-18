//! A lossless evaluation memo: results keyed by exactly what `evaluate` reads from a loadout.
//!
//! Owned pets matter only through the best Jellyfish `potionBoost` and the Ragnarock pet strength, so the key holds
//! those two numbers instead of the pet list; the dragon set, accessory bag, shards and perks are in it as ids and
//! content hashes. A memo is valid for one context (item context, tuning split, blessings) and one set of weights:
//! create a new one when either changes ([`EvalMemo::new`] remembers the tuning plan and the evaluator checks it).

use crate::arena::Model;
use crate::ctx::{Ctx, TuningPlan};
use crate::eval::EvalResult;
use crate::ids::*;
use crate::loadout::{DragonGear, ExtraSet, Loadout};
use dps_core::config::profile::BOOK_DAMAGE_OPTION;
use parking_lot::RwLock;
use rustc_hash::{FxHashMap, FxHasher};
use std::hash::{Hash, Hasher};

/// Everything `evaluate` reads from a loadout.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EvalKey {
    weapon: Option<ItemId>,
    rag: Option<ItemId>,
    clear_boots: Option<ItemId>,
    armor: SlotMap,
    equipment: SlotMap,
    dragon_gear: Option<DragonGear>,
    bag: BagId,
    shards: u64,
    perks: u64,
    extras: ExtraSet,
    power: PowerField,
    extra_infliction: bool,
    owned_medallion: bool,
    active_pets: [Option<PetId>; 2],
    potion_boost: u64,
    rag_strength: u64,
    bank: BankId,
}

impl EvalKey {
    pub fn new(model: &Model, l: &Loadout) -> EvalKey {
        EvalKey {
            weapon: l.weapon,
            rag: l.rag,
            clear_boots: l.clear_boots,
            armor: l.armor,
            equipment: l.equipment,
            dragon_gear: l.dragon_gear,
            bag: l.accessories,
            shards: l.shards.hash64(),
            perks: l.perks.hash64(),
            extras: l.extras,
            power: l.power,
            extra_infliction: l.bop_bonus.as_deref() == Some(BOOK_DAMAGE_OPTION),
            owned_medallion: l.owned.medallion,
            active_pets: l.active_pets,
            potion_boost: l.pets.potion_boost(model).to_bits(),
            rag_strength: if l.rag.is_some() { l.pets.rag_strength(model).to_bits() } else { 0 },
            bank: l.bank,
        }
    }

    pub fn hash64(&self) -> u64 {
        let mut h = FxHasher::default();
        self.hash(&mut h);
        h.finish()
    }
}

const SHARDS: usize = 64;

/// Thread-safe memo of evaluation results (sharded, read-mostly).
pub struct EvalMemo {
    plan: TuningPlan,
    /// The weights of the first evaluator that used the memo: the active extras of a group depend on them.
    weights: std::sync::OnceLock<[u64; 2]>,
    maps: Vec<RwLock<FxHashMap<EvalKey, EvalResult>>>,
}

impl EvalMemo {
    pub fn new(ctx: &Ctx) -> EvalMemo {
        EvalMemo { plan: ctx.tuning_plan, weights: std::sync::OnceLock::new(), maps: (0..SHARDS).map(|_| RwLock::new(FxHashMap::default())).collect() }
    }

    /// True when this memo was made for `ctx`'s tuning split.
    pub fn matches(&self, ctx: &Ctx) -> bool {
        self.plan == ctx.tuning_plan
    }

    /// True when these are the weights the memo was first used with (the first call fixes them).
    pub fn bind_weights(&self, w: &dps_core::model::Weights) -> bool {
        let bits = [w.wither.to_bits(), w.dragon.to_bits()];
        *self.weights.get_or_init(|| bits) == bits
    }

    pub fn get(&self, key: &EvalKey) -> Option<EvalResult> {
        let s = (key.hash64() >> 40) as usize % SHARDS;
        self.maps[s].read().get(key).copied()
    }

    pub fn insert(&self, key: EvalKey, r: EvalResult) {
        let s = (key.hash64() >> 40) as usize % SHARDS;
        self.maps[s].write().insert(key, r);
    }

    pub fn len(&self) -> usize {
        self.maps.iter().map(|m| m.read().len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        for m in &self.maps {
            m.write().clear();
        }
    }
}

/// Content key of a whole loadout, like JS `loadoutKey(l)`: two loadouts with the same key are the same planner state
/// (the accessory bag counts as a set, slot objects by slot name regardless of property order). Everything the
/// planner can change is in it, so it is a superset of what `evaluate` reads (see [`EvalKey`]).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LoadoutKey {
    weapon: Option<ItemId>,
    rag: Option<ItemId>,
    clear_boots: Option<ItemId>,
    armor: SlotKey,
    equipment: SlotKey,
    bag_set: u64,
    bag_len: u32,
    pets: Vec<PetId>,
    active_pets: [Option<PetId>; 2],
    shards: u64,
    perks: u64,
    extras: ExtraSet,
    power: PowerField,
    bop_bonus: Option<std::sync::Arc<str>>,
    wither_impact: bool,
    owned_medallion: bool,
    dragon: Option<DragonKey>,
    bank: BankId,
}

type SlotKey = [Option<Option<ItemId>>; 4];
type DragonKey = (Option<ItemId>, SlotKey, SlotKey);

fn by_slot(m: &SlotMap) -> SlotKey {
    let mut out = [None; 4];
    for (slot, item) in m.entries() {
        out[slot] = Some(item);
    }
    out
}

impl LoadoutKey {
    pub fn new(model: &Model, l: &Loadout) -> LoadoutKey {
        LoadoutKey {
            weapon: l.weapon,
            rag: l.rag,
            clear_boots: l.clear_boots,
            armor: by_slot(&l.armor),
            equipment: by_slot(&l.equipment),
            bag_set: model.bag_set_hash(l.accessories),
            bag_len: model.bag(l.accessories).items.len() as u32,
            pets: l.pets.ids.clone(),
            active_pets: l.active_pets,
            shards: l.shards.hash64(),
            perks: l.perks.hash64(),
            extras: l.extras,
            power: l.power,
            bop_bonus: l.bop_bonus.clone(),
            wither_impact: l.wither_impact,
            owned_medallion: l.owned.medallion,
            dragon: l.dragon_gear.as_ref().map(|d| (d.weapon, by_slot(&d.armor), by_slot(&d.equipment))),
            bank: l.bank,
        }
    }
}
