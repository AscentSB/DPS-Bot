//! The loadout (JS `cloneLoadout` shape): gear as interned item ids, shared accessory bag, copy-on-write shard, perk
//! and pet lists. Cloning a loadout copies a few dozen bytes and bumps a handful of reference counts.

use crate::arena::Model;
use crate::ids::*;
use crate::itemstats::{compute_shard_stats, ShardStats};
use dps_core::config::essence::ESSENCE_PERKS;
use dps_core::config::shards::SHARDS;
use dps_core::config::upgrades::EXTRA_UPGRADES;
use dps_core::jsmath;
use dps_core::kinds::Class;
use indexmap::IndexSet;
use rustc_hash::FxHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

// ---------------------------------------------------------------------------------------------------------------
// Level maps (JS `shards` and `perks` objects)
// ---------------------------------------------------------------------------------------------------------------

/// Which config table a [`Levels`] object is indexed by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LevelKind {
    Shards,
    Perks,
}

/// A JS object `{ KEY: level }` (`loadout.shards`, `loadout.perks`), immutable and shared between loadouts until a
/// step changes it. Keeps the JS key order and the levels of the keys the config tables know in a dense array.
pub struct Levels {
    kind: LevelKind,
    entries: Vec<(String, f64)>,
    dense: Vec<f64>,
    hash: u64,
    shard_stats: [OnceLock<ShardStats>; 3],
}

fn table_keys(kind: LevelKind) -> Vec<&'static str> {
    match kind {
        LevelKind::Shards => SHARDS.iter().map(|s| s.id).collect(),
        LevelKind::Perks => ESSENCE_PERKS.iter().map(|p| p.key).collect(),
    }
}

impl Levels {
    pub fn new(kind: LevelKind, entries: Vec<(String, f64)>) -> Arc<Levels> {
        let keys = table_keys(kind);
        let mut dense = vec![0.0; keys.len()];
        for (k, v) in &entries {
            if let Some(i) = keys.iter().position(|x| x == k) {
                dense[i] = *v;
            }
        }
        let mut h = FxHasher::default();
        for (k, v) in &entries {
            k.hash(&mut h);
            h.write_u64(v.to_bits());
        }
        Arc::new(Levels { kind, entries, dense, hash: h.finish(), shard_stats: [OnceLock::new(), OnceLock::new(), OnceLock::new()] })
    }

    pub fn empty(kind: LevelKind) -> Arc<Levels> {
        Levels::new(kind, Vec::new())
    }

    pub fn kind(&self) -> LevelKind {
        self.kind
    }

    /// JS `obj[key]`.
    pub fn get(&self, key: &str) -> Option<f64> {
        self.entries.iter().find(|(k, _)| k == key).map(|&(_, v)| v)
    }

    /// JS `shardLevel(loadout, id)`: `Math.max(0, Math.min(10, shards[id] ?? 0))`.
    pub fn shard_level(&self, id: &str) -> f64 {
        jsmath::max(0.0, jsmath::min(10.0, self.get(id).unwrap_or(0.0)))
    }

    /// The level of table entry `i` (`SHARDS[i]` / `ESSENCE_PERKS[i]`), 0 when absent (`obj[key] ?? 0`).
    #[inline]
    pub fn at(&self, i: usize) -> f64 {
        self.dense[i]
    }

    /// `{ ...self, [key]: level }`: an existing key keeps its position.
    pub fn with(&self, key: &str, level: f64) -> Arc<Levels> {
        let mut entries = self.entries.clone();
        match entries.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 = level,
            None => entries.push((key.to_string(), level)),
        }
        Levels::new(self.kind, entries)
    }

    pub fn entries(&self) -> &[(String, f64)] {
        &self.entries
    }

    /// Content hash (key order included).
    #[inline]
    pub fn hash64(&self) -> u64 {
        self.hash
    }

    /// JS `shardStats(loadout, ctx)` (cached per levels object and class).
    pub fn shard_stats(&self, cls: Class) -> &ShardStats {
        debug_assert_eq!(self.kind, LevelKind::Shards);
        self.shard_stats[cls.index()].get_or_init(|| compute_shard_stats(&|i| self.dense[i], cls))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Owned items, extras, pets
// ---------------------------------------------------------------------------------------------------------------

/// JS `loadout.owned`: ids of items anywhere in the profile (a Set). Copy on write.
#[derive(Clone, Debug, Default)]
pub struct Owned {
    set: Arc<IndexSet<String>>,
    /// `owned.has(GENERAL_MEDALLION)`.
    pub medallion: bool,
}

impl Owned {
    pub fn from_ids(ids: impl IntoIterator<Item = String>) -> Owned {
        let set: IndexSet<String> = ids.into_iter().collect();
        let medallion = set.contains(dps_core::config::constants::GENERAL_MEDALLION_ID);
        Owned { set: Arc::new(set), medallion }
    }
    pub fn contains(&self, id: &str) -> bool {
        self.set.contains(id)
    }
    /// JS `owned.add(id)`.
    pub fn insert(&mut self, id: &str) {
        if !self.set.contains(id) {
            Arc::make_mut(&mut self.set).insert(id.to_string());
            if id == dps_core::config::constants::GENERAL_MEDALLION_ID {
                self.medallion = true;
            }
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.set.iter().map(|s| s.as_str())
    }
    pub fn len(&self) -> usize {
        self.set.len()
    }
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}

const _: () = assert!(EXTRA_UPGRADES.len() <= 32);

/// JS `loadout.extras` (a Set of `EXTRA_UPGRADES` keys) as a bit set over `EXTRA_UPGRADES` indexes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ExtraSet(pub u32);

impl ExtraSet {
    #[inline]
    pub fn has_index(self, i: usize) -> bool {
        self.0 & (1 << i) != 0
    }
    pub fn has(self, key: &str) -> bool {
        EXTRA_UPGRADES.iter().position(|e| e.key == key).is_some_and(|i| self.has_index(i))
    }
    pub fn insert_index(&mut self, i: usize) {
        self.0 |= 1 << i;
    }
    /// JS `extras.add(key)`; keys that are not in `EXTRA_UPGRADES` have no effect on anything and are dropped.
    pub fn insert(&mut self, key: &str) {
        if let Some(i) = EXTRA_UPGRADES.iter().position(|e| e.key == key) {
            self.insert_index(i);
        }
    }
    pub fn keys(self) -> impl Iterator<Item = &'static str> {
        EXTRA_UPGRADES.iter().enumerate().filter(move |(i, _)| self.has_index(*i)).map(|(_, e)| e.key)
    }
}

/// JS `loadout.pets`: the owned pets, immutable and shared. Derived values are cached on first use.
pub struct PetList {
    pub ids: Vec<PetId>,
    potion_boost: OnceLock<f64>,
    rag_strength: OnceLock<f64>,
    pub(crate) dps: OnceLock<Vec<PetId>>,
}

impl PetList {
    pub fn new(ids: Vec<PetId>) -> Arc<PetList> {
        Arc::new(PetList { ids, potion_boost: OnceLock::new(), rag_strength: OnceLock::new(), dps: OnceLock::new() })
    }

    /// JS `potionBoost(loadout, ctx)`: the best Jellyfish buff among the pets.
    pub fn potion_boost(&self, model: &Model) -> f64 {
        *self.potion_boost.get_or_init(|| {
            let mut best = 0.0;
            for &p in &self.ids {
                let e = &model.pets[p.index()];
                if e.pet.kind == "JELLYFISH" {
                    best = jsmath::max(best, e.potion_boost);
                }
            }
            best
        })
    }

    /// The Ragnarock cast: `Math.max(0, ...pets.map(p => petStats(p).petStrength ?? 0))`.
    pub fn rag_strength(&self, model: &Model) -> f64 {
        *self.rag_strength.get_or_init(|| {
            let mut best = 0.0;
            for &p in &self.ids {
                best = jsmath::max(best, model.pets[p.index()].pet_strength);
            }
            best
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Loadout
// ---------------------------------------------------------------------------------------------------------------

/// JS `loadout.dragonGear`: only the slots that differ from the main set (Mage LCM).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct DragonGear {
    pub weapon: Option<ItemId>,
    pub armor: SlotMap,
    pub equipment: SlotMap,
}

/// The gear used against one target (JS `gearView`): the loadout itself, or the dragon set merged over it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct GearView {
    pub weapon: Option<ItemId>,
    pub armor: SlotMap,
    pub equipment: SlotMap,
}

impl GearView {
    /// JS `loadoutGear(view)`: weapon, armor values, equipment values; missing ones skipped.
    #[inline]
    pub fn items(&self) -> impl Iterator<Item = ItemId> + '_ {
        self.weapon.into_iter().chain(self.armor.values()).chain(self.equipment.values())
    }
}

/// The loadout fields a candidate rarely changes. They live behind ONE `Arc`, so cloning a loadout (which parallel
/// candidate scoring does thousands of times per state, all threads cloning the same loadout) touches one shared
/// reference count instead of eight: with 16 threads that took a clone from 24 ns to about 750 ns of CPU.
/// `Loadout` derefs to this struct (`l.pets`, `l.shards = ...` work as before; a write copies the struct when it is
/// shared, copy-on-write like the JS).
#[derive(Clone)]
pub struct Rare {
    pub owned: Owned,
    pub shards: Arc<Levels>,
    pub perks: Arc<Levels>,
    pub pets: Arc<PetList>,
    pub bop_bonus: Option<Arc<str>>,
    pub blades: Arc<[ItemId]>,
    pub spare_blades: Arc<[ItemId]>,
    /// Learned powers (indexes into `POWERS`) in set insertion order.
    pub unlocked_powers: Arc<[u8]>,
}

/// A loadout (JS `buildLoadout` / `cloneLoadout` object).
#[derive(Clone)]
pub struct Loadout {
    pub weapon: Option<ItemId>,
    pub armor: SlotMap,
    pub equipment: SlotMap,
    /// The Ragnarock used for its ability (may also be the weapon).
    pub rag: Option<ItemId>,
    pub clear_boots: Option<ItemId>,
    pub dragon_gear: Option<DragonGear>,
    /// Accessory bag, shared until a step changes it.
    pub accessories: BagId,
    pub extras: ExtraSet,
    /// Active pet per target (`[wither, dragon]`).
    pub active_pets: [Option<PetId>; 2],
    pub power: PowerField,
    pub bank: BankId,
    pub wither_impact: bool,
    pub rcm_blade: Option<ItemId>,
    /// `owned`, `shards`, `perks`, `pets`, `bop_bonus`, `blades`, `spare_blades`, `unlocked_powers` (see [`Rare`]).
    pub rare: Arc<Rare>,
}

impl std::ops::Deref for Loadout {
    type Target = Rare;
    #[inline]
    fn deref(&self) -> &Rare {
        &self.rare
    }
}

impl std::ops::DerefMut for Loadout {
    #[inline]
    fn deref_mut(&mut self) -> &mut Rare {
        Arc::make_mut(&mut self.rare)
    }
}

impl Loadout {
    /// An empty loadout (JS `buildLoadout`'s first object).
    pub fn empty(model: &mut Model) -> Loadout {
        let bag = model.empty_bag();
        let bank = model.bank_id(0.0);
        Loadout {
            weapon: None,
            armor: SlotMap::new(),
            equipment: SlotMap::new(),
            rag: None,
            clear_boots: None,
            dragon_gear: None,
            accessories: bag,
            extras: ExtraSet::default(),
            active_pets: [None, None],
            power: PowerField::Unset,
            bank,
            wither_impact: false,
            rcm_blade: None,
            rare: Arc::new(Rare {
                owned: Owned::default(),
                shards: Levels::empty(LevelKind::Shards),
                perks: Levels::empty(LevelKind::Perks),
                pets: PetList::new(Vec::new()),
                bop_bonus: None,
                blades: Arc::from(Vec::new()),
                spare_blades: Arc::from(Vec::new()),
                unlocked_powers: Arc::from(Vec::new()),
            }),
        }
    }

    // ---- small edits the planner makes (all copy-on-write; call them on a clone) ----

    /// `{ ...shards, [id]: level }`.
    pub fn set_shard(&mut self, id: &str, level: f64) {
        self.shards = self.shards.with(id, level);
    }

    /// `{ ...perks, [key]: level }`.
    pub fn set_perk(&mut self, key: &str, level: f64) {
        self.perks = self.perks.with(key, level);
    }

    /// `loadout.extras.add(key)`.
    pub fn add_extra(&mut self, key: &str) {
        self.extras.insert(key);
    }

    /// `loadout.owned.add(id)`.
    pub fn add_owned(&mut self, id: &str) {
        self.owned.insert(id);
    }

    /// `loadout.bank = bank`.
    pub fn set_bank(&mut self, model: &mut Model, bank: f64) {
        self.bank = model.bank_id(bank);
    }

    /// The coins in the bank.
    pub fn bank_value(&self, model: &Model) -> f64 {
        model.bank_value(self.bank)
    }

    /// `loadout.power = name` (`None` = unset; a name `POWERS` does not know is kept as unknown).
    pub fn set_power(&mut self, name: Option<&str>) {
        self.power = match name {
            None => PowerField::Unset,
            Some(n) => match crate::ctx::power_index(n) {
                Some(i) => PowerField::Known(i as u8),
                None => PowerField::Unknown,
            },
        };
    }

    /// `loadout.unlockedPowers.add(name)` (unknown names are ignored: nothing reads them).
    pub fn unlock_power(&mut self, name: &str) {
        if let Some(i) = crate::ctx::power_index(name) {
            if !self.unlocked_powers.contains(&(i as u8)) {
                let mut v = self.unlocked_powers.to_vec();
                v.push(i as u8);
                self.unlocked_powers = v.into();
            }
        }
    }

    /// `loadout.pets = pets`.
    pub fn set_pets(&mut self, model: &mut Model, pets: &[dps_core::model::Pet]) {
        self.pets = model.pet_list(pets);
    }

    /// `loadout.activePets[target] = pet`.
    pub fn set_active_pet(&mut self, target: dps_core::kinds::Target, pet: Option<PetId>) {
        self.active_pets[target.index()] = pet;
    }

    /// JS `loadoutGear(loadout)`: weapon, armor values, equipment values (the main set).
    pub fn gear(&self) -> GearView {
        self.gear_view(false)
    }

    /// JS `gearView(loadout, target)` for the dragon (`dragon = true`) or the withers.
    pub fn gear_view(&self, dragon: bool) -> GearView {
        match (&self.dragon_gear, dragon) {
            (Some(d), true) => GearView {
                weapon: d.weapon.or(self.weapon),
                armor: self.armor.merged(&d.armor),
                equipment: self.equipment.merged(&d.equipment),
            },
            _ => GearView { weapon: self.weapon, armor: self.armor, equipment: self.equipment },
        }
    }

    /// JS `mainItem(l, slot)`.
    pub fn main_item(&self, slot: GearSlot) -> Option<ItemId> {
        match slot {
            GearSlot::Weapon => self.weapon,
            GearSlot::Armor(i) => self.armor.get(i as usize),
            GearSlot::Equipment(i) => self.equipment.get(i as usize),
        }
    }

    /// JS `dragonItem(l, slot)`.
    pub fn dragon_item(&self, slot: GearSlot) -> Option<ItemId> {
        let d = self.dragon_gear.as_ref()?;
        match slot {
            GearSlot::Weapon => d.weapon,
            GearSlot::Armor(i) => d.armor.get(i as usize),
            GearSlot::Equipment(i) => d.equipment.get(i as usize),
        }
    }

    /// JS `setDragonSlot(l, slot, item)`: set (or clear, `None`) a dragon slot. An item the main set already uses is
    /// not stored.
    pub fn set_dragon_slot(&mut self, slot: GearSlot, item: Option<ItemId>) {
        let value = match item {
            Some(it) if Some(it) != self.main_item(slot) => Some(it),
            _ => None,
        };
        self.store_dragon_slot(slot, value);
    }

    /// Store (or clear) a dragon slot without the "same item as the main set" check. The JS compares item objects, so a
    /// byte-identical copy of the worn piece is a different item and may sit in the dragon set; the caller decides.
    pub fn store_dragon_slot(&mut self, slot: GearSlot, value: Option<ItemId>) {
        let mut d = self.dragon_gear.unwrap_or_default();
        match slot {
            GearSlot::Weapon => d.weapon = value,
            GearSlot::Armor(i) => match value {
                Some(_) => d.armor.set(i as usize, value),
                None => d.armor.remove(i as usize),
            },
            GearSlot::Equipment(i) => match value {
                Some(_) => d.equipment.set(i as usize, value),
                None => d.equipment.remove(i as usize),
            },
        }
        self.dragon_gear =
            if d.weapon.is_some() || d.armor.key_count() > 0 || d.equipment.key_count() > 0 { Some(d) } else { None };
    }

    /// JS `setSlot(l, slot, item)`.
    pub fn set_slot(&mut self, slot: Slot, item: ItemId) {
        self.set_slot_keep_dragon(slot, item);
        // The dragon set never repeats the main set's piece.
        if let Slot::Gear(g) = slot {
            if self.dragon_item(g) == Some(item) {
                self.set_dragon_slot(g, None);
            }
        }
    }

    /// `set_slot` without the dragon-set cleanup (the caller compares item identity itself).
    pub fn set_slot_keep_dragon(&mut self, slot: Slot, item: ItemId) {
        match slot {
            Slot::Rag => self.rag = Some(item),
            Slot::Gear(GearSlot::Weapon) => self.weapon = Some(item),
            Slot::Gear(GearSlot::Armor(i)) => self.armor.set(i as usize, Some(item)),
            Slot::Gear(GearSlot::Equipment(i)) => self.equipment.set(i as usize, Some(item)),
        }
    }

    /// JS `dragonItems(l)`.
    pub fn dragon_items(&self) -> Vec<ItemId> {
        match &self.dragon_gear {
            None => Vec::new(),
            Some(d) => d.weapon.into_iter().chain(d.armor.values()).chain(d.equipment.values()).collect(),
        }
    }

    /// JS `slotName(loadout, item)`. `copy` is the dragon set's byte-identical copy of a worn piece (see
    /// [`Loadout::upgradable_items`]): a different object, so the main set does not match it.
    pub fn slot_name(&self, model: &Model, item: ItemId, copy: bool) -> String {
        if !copy {
            if self.weapon == Some(item) {
                return "weapon".into();
            }
            if self.rag == Some(item) {
                return "rag".into();
            }
            for (slot, it) in self.armor.entries() {
                if it == Some(item) {
                    return ARMOR_NAMES[slot].into();
                }
            }
            if self.clear_boots == Some(item) {
                return "clearBoots".into();
            }
            for (slot, it) in self.equipment.entries() {
                if it == Some(item) {
                    return EQUIPMENT_NAMES[slot].into();
                }
            }
        }
        if let Some(d) = &self.dragon_gear {
            if d.weapon == Some(item) {
                return "dragon:weapon".into();
            }
            // Object.entries({ ...d.armor, ...d.equipment })
            for (slot, it) in d.armor.entries() {
                if it == Some(item) {
                    return format!("dragon:{}", ARMOR_NAMES[slot]);
                }
            }
            for (slot, it) in d.equipment.entries() {
                if it == Some(item) {
                    return format!("dragon:{}", EQUIPMENT_NAMES[slot]);
                }
            }
        }
        model.item(item).id.clone()
    }

    /// JS `replaceItem(loadout, oldItem, next)`: replace an item everywhere it is used (a Ragnarock can be both
    /// weapon and ability item), in the bag too, and mark the new id owned.
    pub fn replace_item(&self, model: &mut Model, old: ItemId, next: ItemId) -> Loadout {
        let mut l = self.clone();
        let swap = |x: Option<ItemId>| if x == Some(old) { Some(next) } else { x };
        l.weapon = swap(l.weapon);
        l.rag = swap(l.rag);
        l.clear_boots = swap(l.clear_boots);
        l.armor = l.armor.map_values(swap);
        l.equipment = l.equipment.map_values(swap);
        if self.dragon_items().contains(&old) {
            if let Some(d) = &mut l.dragon_gear {
                d.weapon = swap(d.weapon);
                d.armor = d.armor.map_values(swap);
                d.equipment = d.equipment.map_values(swap);
            }
        }
        if let Some(b) = model.bag_replace(l.accessories, old, next) {
            l.accessories = b;
        }
        // A fragged item is a new id the player now owns, so it is never offered as a buy.
        let id = model.item(next).id.clone();
        l.owned.insert(&id);
        l
    }

    /// Is the dragon set's piece in this slot a byte-identical copy of the main set's piece? The JS compares item
    /// objects, so the copy is a different item that is upgraded, replaced and named separately.
    #[inline]
    fn dragon_copy(&self, slot: GearSlot) -> bool {
        matches!((self.main_item(slot), self.dragon_item(slot)), (Some(m), Some(d)) if m == d)
    }

    /// JS `upgradableItems(loadout)`: every worn item object once. Equal ids are one object, except the dragon set's
    /// copy of a worn piece (flag `true`; the JS `Set` of objects keeps it as an item of its own).
    pub fn upgradable_items(&self, model: &Model) -> Vec<(ItemId, bool)> {
        let mut out: Vec<(ItemId, bool)> = Vec::new();
        let mut push = |x: Option<ItemId>, copy: bool| {
            if let Some(i) = x {
                if copy || !out.contains(&(i, false)) {
                    out.push((i, copy));
                }
            }
        };
        push(self.weapon, false);
        push(self.rag, false);
        push(self.clear_boots, false);
        for i in self.armor.values() {
            push(Some(i), false);
        }
        for i in self.equipment.values() {
            push(Some(i), false);
        }
        if let Some(d) = &self.dragon_gear {
            push(d.weapon, self.dragon_copy(GearSlot::Weapon));
            for (s, i) in d.armor.entries() {
                push(i, self.dragon_copy(GearSlot::Armor(s as u8)));
            }
            for (s, i) in d.equipment.entries() {
                push(i, self.dragon_copy(GearSlot::Equipment(s as u8)));
            }
        }
        for &i in model.bag(self.accessories).items.iter() {
            push(Some(i), false);
        }
        out
    }

    /// JS `loadout.accessories.has(GENERAL_MEDALLION_ID) || loadout.owned.has(...)`: gives the secret-digit bonus.
    #[inline]
    pub fn has_medallion(&self, model: &Model) -> bool {
        self.owned.medallion || model.bag(self.accessories).has_medallion
    }
}
