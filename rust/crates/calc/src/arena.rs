//! The interning arena: every distinct item state, accessory bag, pet and bank amount gets a small id, and everything
//! derived from it (stats per context, accessory power, flags the damage model reads) is computed once and cached.
//!
//! Interning needs `&mut Model` and happens in single-threaded phases (building loadouts and candidate lists).
//! Scoring only reads the model (`&Model`) and may run on many threads: the lazily computed per-item data lives in
//! `OnceLock`s.

use crate::ctx::{Ctx, ItemCtxKey};
use crate::ids::*;
use crate::loose::Loose;
use dps_core::config::accessories::{ap_by_rarity, ap_override, DOUBLE_AP_ITEMS, DUNGEON_AP_MULTIPLIER};
use dps_core::config::armor_sets::ARMOR_SETS;
use dps_core::config::constants::*;
use dps_core::config::enchants::enchant;
use dps_core::config::item_stats::item_stats;
use dps_core::data::{accessory_chain_position, accessory_family, is_ignored_accessory};
use dps_core::kinds::{Class, FloorMode, Mode};
use dps_core::model::{ItemState, Pet, PetProfile, Resource};
use dps_core::pets::{compute_pet_stats, PetStats};
use rustc_hash::FxHashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock, RwLock};

/// Three stat objects: stats for both targets, wither only and dragon only (JS `{ global, wither, dragon }`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Triple {
    pub global: Loose,
    pub wither: Loose,
    pub dragon: Loose,
}

/// Per item and secret-digit variant, computed on first use.
#[derive(Default)]
pub struct Variant {
    pub raw: OnceLock<Box<Triple>>,
    pub contrib: OnceLock<Box<Triple>>,
    pub acc: OnceLock<Box<Loose>>,
    /// `(flat, crit)` dungeon multipliers of this item (`scaleBucket`), `None` for non-dungeon items.
    pub scale: OnceLock<Option<(f64, f64)>>,
    /// Ragnarock strength for a few pet strengths seen (`(pet strength bits, strength)`), lock free.
    pub rag: [OnceLock<(u64, f64)>; 3],
}

/// Facts about an item that the damage model reads, worked out once when it is interned.
#[derive(Clone, Copy, Debug)]
pub struct ItemFlags {
    pub is_bow: bool,
    pub is_weapon_category: bool,
    /// JS `isDungeon(item, resource)`.
    pub is_dungeon: bool,
    /// `resource.get(id)?.dungeon_item` truthy.
    pub info_dungeon_item: bool,
    /// JS `accessoryPower(item, resource)`.
    pub ap: f64,
    pub acc_ignored: bool,
    pub acc_family: Option<u32>,
    /// JS `itemFamilyRank(item)`.
    pub acc_rank: i64,
    /// `enchants.ultimate_legion ?? 0`.
    pub legion_level: f64,
    /// `enchants.ultimate_chimera ?? 0`.
    pub chimera_level: f64,
    /// JS `hasWitherImpact(item)`.
    pub has_wi: bool,
    /// `WITHER_IMPACT_WEAPONS.includes(id)`.
    pub wi_weapon_id: bool,
    pub is_rag_id: bool,
    pub fabled: bool,
    pub loving: bool,
    pub sa_cloak: bool,
    pub implosion_belt: bool,
    pub dual_wield: bool,
    /// `isBow && enchants.overload > 0`.
    pub overload: bool,
    /// JS `fireAspectPercent(item)`.
    pub fire_pct: f64,
    /// JS `firstStrikeAdditive(item)`.
    pub first_strike_add: f64,
    /// JS `thunderlordPercent(item)`.
    pub thunder_pct: f64,
    /// `ITEM_STATS[id]?.shotTicks`.
    pub shot_ticks: Option<f64>,
    /// `ITEM_STATS[id]?.arrows ?? 1`.
    pub arrows_base: f64,
    /// `Math.min(hpb, 15)`: potato books that count.
    pub potatoes: f64,
    /// Bit `s` set when the id is a piece of `ARMOR_SETS[s]`.
    pub set_mask: u32,
    pub is_medallion: bool,
    pub is_book: bool,
}

pub struct ItemEntry {
    pub state: ItemState,
    pub kind: KindId,
    pub flags: ItemFlags,
    pub variants: [Variant; 2],
}

pub struct BagBase {
    /// JS `countedAccessories(loadout)`.
    pub counted: Vec<ItemId>,
    /// Sum of `accessoryPower` over `counted`.
    pub ap: f64,
    /// Sum of the counted accessories' stats (before the power's stats).
    pub loose: Loose,
    /// `powerMultiplier(ap)`.
    pub power_mult: f64,
}

pub struct BagEntry {
    pub items: Box<[ItemId]>,
    pub kinds: Box<[KindId]>,
    pub has_medallion: bool,
    pub has_book: bool,
    pub(crate) base: [OnceLock<Box<BagBase>>; 2],
    /// Hash of the sorted item ids: equal for bags with the same items in any order.
    pub(crate) set_hash: OnceLock<u64>,
}

pub struct PetEntry {
    pub pet: Pet,
    /// Golden Dragon's stats depend on the bank.
    pub per_bank: bool,
    pub stats: Vec<PetStats>,
    /// `petStats(...).petStrength` (does not depend on the bank).
    pub pet_strength: f64,
    /// `petStats(...).potionBoost` (does not depend on the bank).
    pub potion_boost: f64,
    /// Whether `dps_pets` keeps this pet (any damage stat or dragon/wither bonus). Only meaningful when `!per_bank`
    /// (Golden Dragon depends on the bank), cached on first use.
    pub(crate) useful: OnceLock<bool>,
}

/// The arena plus the context-dependent caches (see the module docs).
pub struct Model {
    pub(crate) key: ItemCtxKey,
    pub(crate) resource: Arc<Resource>,
    pub(crate) cls: Class,
    pub(crate) cata_level: f64,
    pub(crate) floor_mode: FloorMode,
    pub(crate) mode: Mode,
    pub(crate) maxed: f64,
    pub(crate) secret_digit_medallion: f64,
    pub(crate) pet_gold: Option<f64>,
    pub(crate) items: Vec<ItemEntry>,
    item_map: FxHashMap<u64, Vec<ItemId>>,
    kinds: Vec<Arc<str>>,
    kind_map: FxHashMap<Arc<str>, u32>,
    families: FxHashMap<&'static str, u32>,
    pub(crate) bags: Vec<BagEntry>,
    bag_map: FxHashMap<u64, Vec<BagId>>,
    pub(crate) pets: Vec<PetEntry>,
    pet_map: FxHashMap<Pet, PetId>,
    banks: Vec<f64>,
    bank_map: FxHashMap<u64, u32>,
    pub(crate) base_cache: Arc<BaseCache>,
}

/// `baseItemStats` results by item id for one (resource, class, Catacombs level, floor mode); shareable between
/// models of the same item context so a run does not recompute them.
pub type BaseCache = RwLock<FxHashMap<String, Arc<Triple>>>;

fn hash_opt_str(h: &mut impl Hasher, s: &Option<String>) {
    match s {
        Some(s) => {
            h.write_u8(1);
            s.hash(h);
        }
        None => h.write_u8(0),
    }
}

/// Content hash of an item state: order of enchants and gem slots counts.
pub(crate) fn hash_state(s: &ItemState) -> u64 {
    let mut h = rustc_hash::FxHasher::default();
    s.id.hash(&mut h);
    s.name.hash(&mut h);
    hash_opt_str(&mut h, &s.category);
    s.base_rarity.hash(&mut h);
    s.recomb.hash(&mut h);
    s.stars.hash(&mut h);
    s.master_stars.hash(&mut h);
    s.dungeon.hash(&mut h);
    for (k, v) in &s.enchants {
        k.hash(&mut h);
        v.hash(&mut h);
    }
    s.hpb.hash(&mut h);
    s.art_of_war.hash(&mut h);
    hash_opt_str(&mut h, &s.reforge);
    for (k, g) in &s.gems.slots {
        k.hash(&mut h);
        g.hash(&mut h);
    }
    for u in &s.gems.unlocked {
        u.hash(&mut h);
    }
    hash_opt_str(&mut h, &s.enrichment);
    for sc in &s.scrolls {
        sc.hash(&mut h);
    }
    match &s.pet {
        Some(p) => {
            h.write_u8(1);
            hash_opt_str(&mut h, &p.kind);
            hash_opt_str(&mut h, &p.tier);
            p.exp.map(f64::to_bits).hash(&mut h);
            hash_opt_str(&mut h, &p.held_item);
        }
        None => h.write_u8(0),
    }
    s.floor.map(f64::to_bits).hash(&mut h);
    s.stat_boost.map(f64::to_bits).hash(&mut h);
    s.dual_wield.hash(&mut h);
    h.finish()
}

/// Equality that respects key order of enchants and gem slots (an `IndexMap` compares as an unordered map).
pub(crate) fn state_eq(a: &ItemState, b: &ItemState) -> bool {
    a.id == b.id
        && a.name == b.name
        && a.category == b.category
        && a.base_rarity == b.base_rarity
        && a.recomb == b.recomb
        && a.stars == b.stars
        && a.master_stars == b.master_stars
        && a.dungeon == b.dungeon
        && a.hpb == b.hpb
        && a.art_of_war == b.art_of_war
        && a.reforge == b.reforge
        && a.enrichment == b.enrichment
        && a.scrolls == b.scrolls
        && a.gems.unlocked == b.gems.unlocked
        && a.pet == b.pet
        && a.floor.map(f64::to_bits) == b.floor.map(f64::to_bits)
        && a.stat_boost.map(f64::to_bits) == b.stat_boost.map(f64::to_bits)
        && a.dual_wield == b.dual_wield
        && a.enchants.len() == b.enchants.len()
        && a.enchants.iter().zip(b.enchants.iter()).all(|(x, y)| x == y)
        && a.gems.slots.len() == b.gems.slots.len()
        && a.gems.slots.iter().zip(b.gems.slots.iter()).all(|(x, y)| x == y)
}

impl Model {
    /// A model for contexts with the same [`ItemCtxKey`] as `ctx`.
    pub fn new(ctx: &Ctx) -> Model {
        Model::with_base_cache(ctx, Arc::new(RwLock::new(FxHashMap::default())))
    }

    /// Like [`Model::new`], sharing a base item stats cache made for the same resource, class, Catacombs level and
    /// floor mode.
    pub fn with_base_cache(ctx: &Ctx, base_cache: Arc<BaseCache>) -> Model {
        Model {
            key: ctx.d.item_key,
            resource: ctx.resource.clone(),
            cls: ctx.cls,
            cata_level: ctx.cata_level,
            floor_mode: ctx.floor_mode,
            mode: ctx.mode,
            maxed: ctx.d.maxed_collections,
            secret_digit_medallion: ctx.d.secret_digit_medallion,
            pet_gold: ctx.pet_profile.gold_collection,
            items: Vec::new(),
            item_map: FxHashMap::default(),
            kinds: Vec::new(),
            kind_map: FxHashMap::default(),
            families: FxHashMap::default(),
            bags: Vec::new(),
            bag_map: FxHashMap::default(),
            pets: Vec::new(),
            pet_map: FxHashMap::default(),
            banks: Vec::new(),
            bank_map: FxHashMap::default(),
            base_cache,
        }
    }

    /// True when `ctx` has the item-relevant fields this model was built for.
    pub fn accepts(&self, ctx: &Ctx) -> bool {
        self.key == ctx.d.item_key
    }

    pub fn resource(&self) -> &Arc<Resource> {
        &self.resource
    }

    // ------------------------------------------------------------------------------------------------------
    // Kinds (item id strings)
    // ------------------------------------------------------------------------------------------------------

    pub fn kind_id(&mut self, id: &str) -> KindId {
        if let Some(&k) = self.kind_map.get(id) {
            return KindId(k);
        }
        let s: Arc<str> = Arc::from(id);
        let k = self.kinds.len() as u32;
        self.kinds.push(s.clone());
        self.kind_map.insert(s, k);
        KindId(k)
    }

    /// The kind of an id string if some interned item or bag already uses it.
    pub fn find_kind(&self, id: &str) -> Option<KindId> {
        self.kind_map.get(id).map(|&k| KindId(k))
    }

    pub fn kind_name(&self, k: KindId) -> &str {
        &self.kinds[k.0 as usize]
    }

    fn family_id(&mut self, fam: &'static str) -> u32 {
        let n = self.families.len() as u32;
        *self.families.entry(fam).or_insert(n)
    }

    // ------------------------------------------------------------------------------------------------------
    // Items
    // ------------------------------------------------------------------------------------------------------

    /// Intern an item state (hash-consing on its content).
    pub fn intern_item(&mut self, state: ItemState) -> ItemId {
        let h = hash_state(&state);
        if let Some(chain) = self.item_map.get(&h) {
            for &id in chain {
                if state_eq(&self.items[id.index()].state, &state) {
                    return id;
                }
            }
        }
        let id = ItemId::from_index(self.items.len());
        let kind = self.kind_id(&state.id);
        let flags = self.make_flags(&state);
        let family = flags.1;
        let mut flags = flags.0;
        flags.acc_family = family.map(|f| self.family_id(f));
        self.items.push(ItemEntry { state, kind, flags, variants: [Variant::default(), Variant::default()] });
        self.item_map.entry(h).or_default().push(id);
        id
    }

    /// Intern a modified copy of an item (`{ ...item, ...changes }` in the JS).
    pub fn modify_item(&mut self, id: ItemId, f: impl FnOnce(&mut ItemState)) -> ItemId {
        let mut s = self.items[id.index()].state.clone();
        f(&mut s);
        self.intern_item(s)
    }

    /// Intern a pet list.
    pub fn pet_list(&mut self, pets: &[Pet]) -> Arc<crate::loadout::PetList> {
        let ids: Vec<PetId> = pets.iter().map(|p| self.pet_id(p)).collect();
        crate::loadout::PetList::new(ids)
    }

    #[inline]
    pub fn entry(&self, id: ItemId) -> &ItemEntry {
        &self.items[id.index()]
    }

    #[inline]
    pub fn item(&self, id: ItemId) -> &ItemState {
        &self.items[id.index()].state
    }

    #[inline]
    pub fn flags(&self, id: ItemId) -> &ItemFlags {
        &self.items[id.index()].flags
    }

    /// Number of interned items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    fn make_flags(&self, st: &ItemState) -> (ItemFlags, Option<&'static str>) {
        let info = self.resource.get(&st.id);
        let cat = st.category.as_deref();
        let is_bow = cat == Some("BOW");
        let ench = |k: &str| st.enchants.get(k).copied().unwrap_or(0);
        let ov = item_stats(&st.id);
        // JS isDungeon
        let is_dungeon = match ov.and_then(|o| o.dungeon) {
            Some(d) => d,
            None => match st.dungeon {
                Some(d) => d,
                None => info.is_some_and(|i| i.is_dungeonizable()),
            },
        };
        let info_dungeon_item = info.is_some_and(|i| i.is_dungeon_item());
        let ap = match ap_override(&st.id) {
            Some(o) => o,
            None => {
                let mut ap = ap_by_rarity(item_rarity(&st.base_rarity, st.recomb, &st.id)).unwrap_or(0.0);
                if info_dungeon_item {
                    ap *= DUNGEON_AP_MULTIPLIER;
                }
                if DOUBLE_AP_ITEMS.contains(&st.id.as_str()) {
                    ap *= 2.0;
                }
                ap
            }
        };
        let family = accessory_family(&st.id);
        let rarity = item_rarity(&st.base_rarity, st.recomb, &st.id);
        let rank_idx = rarity_index(stat_rarity(rarity)).map(|i| i as i64).unwrap_or(-1);
        let acc_rank = (rank_idx * 100 + accessory_chain_position(&st.id) as i64) * 2 + st.recomb as i64;
        let has_wi = WITHER_IMPACT_WEAPONS.contains(&st.id.as_str())
            && (st.scrolls.iter().any(|s| s == ULTIMATE_WITHER_SCROLL)
                || WITHER_SCROLLS.iter().all(|w| st.scrolls.iter().any(|s| s == w)));
        let level_or0 = |name: &str, table: Option<&'static [f64]>| -> f64 {
            let level = ench(name);
            let Some(table) = table else { return 0.0 };
            let Some(e) = enchant(name) else { return 0.0 };
            if level == 0 || !cat.is_some_and(|c| e.applies_to_category(c)) {
                return 0.0;
            }
            let idx = (level.min(table.len() as i32)) as i64 - 1;
            if idx < 0 {
                0.0
            } else {
                table[idx as usize]
            }
        };
        let fire_pct = level_or0("fire_aspect", enchant("fire_aspect").and_then(|e| e.fire));
        let thunder_pct = level_or0("thunderlord", enchant("thunderlord").and_then(|e| e.thunder));
        let first_strike_add = level_or0(
            "first_strike",
            enchant("first_strike").and_then(|e| e.stat_values(dps_core::stat::Stat::AdditiveDamage)),
        );
        let mut set_mask = 0u32;
        for (i, s) in ARMOR_SETS.iter().enumerate() {
            if s.pieces.contains(&st.id.as_str()) {
                set_mask |= 1 << i;
            }
        }
        let flags = ItemFlags {
            is_bow,
            is_weapon_category: cat.is_some_and(is_weapon_category),
            is_dungeon,
            info_dungeon_item,
            ap,
            acc_ignored: is_ignored_accessory(&st.id),
            acc_family: None,
            acc_rank,
            legion_level: ench("ultimate_legion") as f64,
            chimera_level: ench("ultimate_chimera") as f64,
            has_wi,
            wi_weapon_id: WITHER_IMPACT_WEAPONS.contains(&st.id.as_str()),
            is_rag_id: st.id == RAGNAROCK_ID,
            fabled: st.reforge.as_deref() == Some("fabled"),
            loving: st.reforge.as_deref() == Some("loving"),
            sa_cloak: SA_CLOAK_IDS.contains(&st.id.as_str()),
            implosion_belt: st.id == IMPLOSION_BELT_ID,
            dual_wield: st.dual_wield,
            overload: is_bow && ench("overload") > 0,
            fire_pct,
            first_strike_add,
            thunder_pct,
            shot_ticks: ov.and_then(|o| o.shot_ticks),
            arrows_base: ov.and_then(|o| o.arrows).unwrap_or(1.0),
            potatoes: dps_core::jsmath::min(st.hpb as f64, HOT_POTATO.max + FUMING_POTATO.max),
            set_mask,
            is_medallion: st.id == GENERAL_MEDALLION_ID,
            is_book: st.id == dps_core::config::profile::BOOK_OF_PROGRESSION_ID,
        };
        (flags, family)
    }

    // ------------------------------------------------------------------------------------------------------
    // Accessory bags
    // ------------------------------------------------------------------------------------------------------

    /// Intern a bag from its entries in order. Entries with the same item id collapse like a JS Map built from them
    /// (the later item replaces the earlier one in place).
    pub fn intern_bag(&mut self, items: &[ItemId]) -> BagId {
        let mut ids: Vec<ItemId> = Vec::with_capacity(items.len());
        let mut kinds: Vec<KindId> = Vec::with_capacity(items.len());
        for &it in items {
            let k = self.items[it.index()].kind;
            match kinds.iter().position(|&x| x == k) {
                Some(p) => ids[p] = it,
                None => {
                    ids.push(it);
                    kinds.push(k);
                }
            }
        }
        self.intern_bag_unique(ids, kinds)
    }

    fn intern_bag_unique(&mut self, ids: Vec<ItemId>, kinds: Vec<KindId>) -> BagId {
        let mut h = rustc_hash::FxHasher::default();
        for i in &ids {
            h.write_u32(i.raw());
        }
        let h = h.finish();
        if let Some(chain) = self.bag_map.get(&h) {
            for &b in chain {
                if *self.bags[b.index()].items == *ids {
                    return b;
                }
            }
        }
        let has_medallion = ids.iter().any(|i| self.items[i.index()].flags.is_medallion);
        let has_book = ids.iter().any(|i| self.items[i.index()].flags.is_book);
        let id = BagId(self.bags.len() as u32);
        self.bags.push(BagEntry {
            items: ids.into_boxed_slice(),
            kinds: kinds.into_boxed_slice(),
            has_medallion,
            has_book,
            base: [OnceLock::new(), OnceLock::new()],
            set_hash: OnceLock::new(),
        });
        self.bag_map.entry(h).or_default().push(id);
        id
    }

    #[inline]
    pub fn bag(&self, id: BagId) -> &BagEntry {
        &self.bags[id.index()]
    }

    /// Order-insensitive hash of a bag's items (JS `loadoutKey` sorts the accessories' content keys).
    pub fn bag_set_hash(&self, bag: BagId) -> u64 {
        let b = &self.bags[bag.index()];
        *b.set_hash.get_or_init(|| {
            let mut ids: Vec<u32> = b.items.iter().map(|i| i.raw()).collect();
            ids.sort_unstable();
            let mut h = rustc_hash::FxHasher::default();
            for i in ids {
                h.write_u32(i);
            }
            h.finish()
        })
    }

    /// The empty bag.
    pub fn empty_bag(&mut self) -> BagId {
        self.intern_bag_unique(Vec::new(), Vec::new())
    }

    /// JS `loadout.accessories.get(id)`.
    pub fn bag_get(&self, bag: BagId, kind: KindId) -> Option<ItemId> {
        let b = &self.bags[bag.index()];
        b.kinds.iter().position(|&k| k == kind).map(|p| b.items[p])
    }

    /// `new Map(bag); map.set(item.id, item)`: replaces in place when the id exists, else appends.
    pub fn bag_with(&mut self, bag: BagId, item: ItemId) -> BagId {
        let kind = self.items[item.index()].kind;
        let b = &self.bags[bag.index()];
        let mut ids = b.items.to_vec();
        let mut kinds = b.kinds.to_vec();
        match kinds.iter().position(|&k| k == kind) {
            Some(p) => ids[p] = item,
            None => {
                ids.push(item);
                kinds.push(kind);
            }
        }
        self.intern_bag_unique(ids, kinds)
    }

    /// `new Map(bag); map.delete(kind)`.
    pub fn bag_without(&mut self, bag: BagId, kind: KindId) -> BagId {
        let b = &self.bags[bag.index()];
        let mut ids = b.items.to_vec();
        let mut kinds = b.kinds.to_vec();
        if let Some(p) = kinds.iter().position(|&k| k == kind) {
            ids.remove(p);
            kinds.remove(p);
        }
        self.intern_bag_unique(ids, kinds)
    }

    /// The part of `replaceItem` that touches the bag: when the bag holds `old` under its id, delete it (if the new
    /// id differs) and set `next`. `None` when `old` is not in the bag.
    pub fn bag_replace(&mut self, bag: BagId, old: ItemId, next: ItemId) -> Option<BagId> {
        let old_kind = self.items[old.index()].kind;
        if self.bag_get(bag, old_kind) != Some(old) {
            return None;
        }
        let next_kind = self.items[next.index()].kind;
        let mut b = bag;
        if next_kind != old_kind {
            b = self.bag_without(b, old_kind);
        }
        Some(self.bag_with(b, next))
    }

    // ------------------------------------------------------------------------------------------------------
    // Banks and pets
    // ------------------------------------------------------------------------------------------------------

    /// Intern a bank amount. Golden Dragon stats are computed for it right away.
    pub fn bank_id(&mut self, bank: f64) -> BankId {
        if let Some(&b) = self.bank_map.get(&bank.to_bits()) {
            return BankId(b);
        }
        let id = self.banks.len() as u32;
        self.banks.push(bank);
        self.bank_map.insert(bank.to_bits(), id);
        let gold = self.pet_gold;
        for e in &mut self.pets {
            if e.per_bank {
                e.stats.push(pet_stats_for(&e.pet, bank, gold));
            }
        }
        BankId(id)
    }

    pub fn bank_value(&self, id: BankId) -> f64 {
        self.banks[id.index()]
    }

    /// Intern a pet; its stats are computed for every known bank (one entry for pets the bank does not affect).
    pub fn pet_id(&mut self, pet: &Pet) -> PetId {
        if let Some(&p) = self.pet_map.get(pet) {
            return p;
        }
        let id = PetId(self.pets.len() as u32);
        let per_bank = pet.kind == "GOLDEN_DRAGON";
        let gold = self.pet_gold;
        let stats = if per_bank {
            self.banks.iter().map(|&b| pet_stats_for(pet, b, gold)).collect()
        } else {
            vec![pet_stats_for(pet, 0.0, gold)]
        };
        let fixed = pet_stats_for(pet, 0.0, gold);
        self.pets.push(PetEntry {
            pet: pet.clone(),
            per_bank,
            stats,
            pet_strength: fixed.pet_strength,
            potion_boost: fixed.potion_boost,
            useful: OnceLock::new(),
        });
        self.pet_map.insert(pet.clone(), id);
        id
    }

    #[inline]
    pub fn pet(&self, id: PetId) -> &Pet {
        &self.pets[id.index()].pet
    }

    /// JS `petStats(pet, petProfileOf(loadout, ctx))` for a loadout with this bank.
    #[inline]
    pub fn pet_stats(&self, id: PetId, bank: BankId) -> &PetStats {
        let e = &self.pets[id.index()];
        if e.per_bank {
            &e.stats[bank.index()]
        } else {
            &e.stats[0]
        }
    }
}

fn pet_stats_for(pet: &Pet, bank: f64, gold: Option<f64>) -> PetStats {
    let profile = PetProfile { bank: Some(bank), gold_collection: gold };
    compute_pet_stats(&pet.kind, &pet.tier, pet.held_item.as_deref(), None, &profile)
}

