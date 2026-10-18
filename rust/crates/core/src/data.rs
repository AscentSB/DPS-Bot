//! src/data/*.json (NEU repo data), embedded at compile time and parsed once on first use.
//!
//! Every string is interned as `&'static str` (leaked once; the data lives for the whole run), so lookups hand out
//! plain `&'static` references. Object key order follows the JS rules (integer-like keys first, then insertion
//! order), which decides iteration order: `all_reforges()` is `Object.entries(allReforges())`, `pet_types()` is
//! `allPetTypes()`, and so on.

use crate::config::pets::Part;
use crate::kinds::Rarity;
use crate::stat::Stat;
use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use std::sync::LazyLock;

const REFORGES_JSON: &str = include_str!("../../../../src/data/reforges.json");
const ACCESSORIES_JSON: &str = include_str!("../../../../src/data/accessories.json");
const FRAGGED_JSON: &str = include_str!("../../../../src/data/fragged.json");
const PETS_JSON: &str = include_str!("../../../../src/data/pets.json");

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn leak_slice<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

/// JS array-index keys ("0", "17"; canonical, below 2^32 - 1) iterate first, ascending.
fn array_index(k: &str) -> Option<u32> {
    if k.is_empty() || (k.len() > 1 && k.starts_with('0')) || !k.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    k.parse::<u64>().ok().filter(|&v| v < u32::MAX as u64).map(|v| v as u32)
}

/// Reorder a parsed JSON object into JS property order.
pub(crate) fn js_order<V>(m: IndexMap<String, V>) -> IndexMap<String, V> {
    if !m.keys().any(|k| array_index(k).is_some()) {
        return m;
    }
    let mut ints: Vec<(u32, String, V)> = Vec::new();
    let mut rest: Vec<(String, V)> = Vec::new();
    for (k, v) in m {
        match array_index(&k) {
            Some(i) => ints.push((i, k, v)),
            None => rest.push((k, v)),
        }
    }
    ints.sort_by_key(|e| e.0);
    ints.into_iter().map(|(_, k, v)| (k, v)).chain(rest).collect()
}

/// Stat object from the data files; keys the JS stat objects never hold are dropped (they have no effect in the
/// JS either: buckets skip them). `dropped` counts them so a test can check the data has none.
fn stat_list(m: IndexMap<String, f64>, dropped: &mut usize) -> &'static [(Stat, f64)] {
    let mut out = Vec::with_capacity(m.len());
    for (k, v) in js_order(m) {
        match Stat::from_js(&k) {
            Some(s) => out.push((s, v)),
            None => *dropped += 1,
        }
    }
    leak_slice(out)
}

// ---------------------------------------------------------------------------------------------------------------
// Reforges
// ---------------------------------------------------------------------------------------------------------------

/// A reforge (reforges.json entry, or a `REFORGE_OVERRIDES` entry). `key` is the lowercase letters-only name.
#[derive(Clone, Copy, Debug)]
pub struct ReforgeDef {
    pub key: &'static str,
    pub name: &'static str,
    /// Reforge stone item id (`None` = blacksmith reforge).
    pub stone: Option<&'static str>,
    /// Item categories it applies to (`ARMOR`, `EQUIPMENT`, `SWORD` also cover groups).
    pub types: &'static [&'static str],
    /// Specific ITEM_IDs it is limited to (JS `items`), when present.
    pub items: Option<&'static [&'static str]>,
    /// Stats by rarity, in JS key order.
    pub stats: &'static [(&'static str, &'static [(Stat, f64)])],
    /// A stat granted per Catacombs level (`cataStat`).
    pub cata_stat: Option<Stat>,
}

/// One entry of `allReforges()`, with its position in that order.
#[derive(Debug)]
pub struct Reforge {
    pub def: ReforgeDef,
    /// Position in `all_reforges()` (a stable identity: JS compares reforge objects with `===`).
    pub index: usize,
    by_rarity: [Option<&'static [(Stat, f64)]>; 9],
}

impl std::ops::Deref for Reforge {
    type Target = ReforgeDef;
    fn deref(&self) -> &ReforgeDef {
        &self.def
    }
}

impl Reforge {
    fn new(def: ReforgeDef, index: usize) -> Reforge {
        let mut by_rarity = [None; 9];
        for &(r, s) in def.stats {
            if let Some(r) = Rarity::from_js(r) {
                if by_rarity[r.index()].is_none() {
                    by_rarity[r.index()] = Some(s);
                }
            }
        }
        Reforge { def, index, by_rarity }
    }

    /// JS `reforge.stats?.[rarity]`.
    #[inline]
    pub fn stats_for(&self, rarity: &str) -> Option<&'static [(Stat, f64)]> {
        match Rarity::from_js(rarity) {
            Some(r) => self.by_rarity[r.index()],
            None => self.def.stats.iter().find(|(k, _)| *k == rarity).map(|&(_, s)| s),
        }
    }

    /// `stats_for` with a parsed rarity.
    #[inline]
    pub fn stats_for_rarity(&self, rarity: Rarity) -> Option<&'static [(Stat, f64)]> {
        self.by_rarity[rarity.index()]
    }

    /// JS `reforgeAppliesTo(reforge, item)` (stats.js), for an item id and its category.
    pub fn applies_to(&self, item_id: &str, category: Option<&str>) -> bool {
        if let Some(items) = self.def.items {
            if !items.is_empty() {
                return items.contains(&item_id);
            }
        }
        let Some(cat) = category else {
            return false;
        };
        self.def.types.iter().any(|&t| {
            if t == cat {
                return true;
            }
            match t {
                "ARMOR" => crate::config::constants::ARMOR_SLOTS.contains(&cat),
                "EQUIPMENT" => matches!(cat, "NECKLACE" | "CLOAK" | "BELT" | "GLOVES" | "BRACELET"),
                "SWORD" => cat == "SWORD" || cat == "LONGSWORD",
                _ => false,
            }
        })
    }
}

#[derive(Deserialize)]
struct RawReforge {
    name: String,
    #[serde(default)]
    stone: Option<String>,
    #[serde(default)]
    types: Option<Vec<String>>,
    #[serde(default)]
    items: Option<Vec<String>>,
    #[serde(default)]
    stats: Option<IndexMap<String, IndexMap<String, f64>>>,
    #[serde(default, rename = "cataStat")]
    cata_stat: Option<String>,
}

struct Reforges {
    list: Vec<Reforge>,
    by_key: FxHashMap<&'static str, usize>,
    dropped: usize,
}

static REFORGES: LazyLock<Reforges> = LazyLock::new(|| {
    let raw: IndexMap<String, RawReforge> = serde_json::from_str(REFORGES_JSON).expect("reforges.json");
    let mut dropped = 0;
    let mut defs: IndexMap<&'static str, ReforgeDef> = IndexMap::new();
    for (key, r) in js_order(raw) {
        let stats = js_order(r.stats.unwrap_or_default())
            .into_iter()
            .map(|(rarity, s)| (leak(rarity), stat_list(s, &mut dropped)))
            .collect::<Vec<_>>();
        let key = leak(key);
        defs.insert(
            key,
            ReforgeDef {
                key,
                name: leak(r.name),
                stone: r.stone.map(leak),
                types: leak_slice(r.types.unwrap_or_default().into_iter().map(leak).collect()),
                items: r.items.map(|v| leak_slice(v.into_iter().map(leak).collect())),
                stats: leak_slice(stats),
                cata_stat: r.cata_stat.as_deref().and_then(|s| {
                    let st = Stat::from_js(s);
                    if st.is_none() {
                        dropped += 1;
                    }
                    st
                }),
            },
        );
    }
    // ALL_REFORGES = { ...REFORGE_DATA, ...REFORGE_OVERRIDES }: an override replaces the entry in place or is
    // appended. getReforge (override first, then data) is the same lookup.
    for o in crate::config::item_stats::REFORGE_OVERRIDES {
        defs.insert(o.key, *o);
    }
    let list: Vec<Reforge> = defs.into_values().enumerate().map(|(i, d)| Reforge::new(d, i)).collect();
    let by_key = list.iter().map(|r| (r.def.key, r.index)).collect();
    Reforges { list, by_key, dropped }
});

/// JS `Object.entries(allReforges())`: every reforge in iteration order.
#[inline]
pub fn all_reforges() -> &'static [Reforge] {
    &REFORGES.list
}

/// JS `reforgeKey(name)`: lowercase, letters a-z only.
pub fn reforge_key(name: &str) -> String {
    if name.is_ascii() {
        name.bytes().filter(|b| b.is_ascii_alphabetic()).map(|b| b.to_ascii_lowercase() as char).collect()
    } else {
        name.to_lowercase().chars().filter(|c| c.is_ascii_lowercase()).collect()
    }
}

/// A reforge by its key (already `reforge_key`-normalised).
#[inline]
pub fn reforge_by_key(key: &str) -> Option<&'static Reforge> {
    REFORGES.by_key.get(key).map(|&i| &REFORGES.list[i])
}

/// JS `getReforge(name)`: by reforge name in any case (`"Fabled"`, `"fabled"`). No allocation for short ASCII names.
pub fn get_reforge(name: &str) -> Option<&'static Reforge> {
    let bytes = name.as_bytes();
    if name.is_ascii() && bytes.len() <= 64 {
        let mut buf = [0u8; 64];
        let mut n = 0;
        for &b in bytes {
            if b.is_ascii_alphabetic() {
                buf[n] = b.to_ascii_lowercase();
                n += 1;
            }
        }
        return reforge_by_key(std::str::from_utf8(&buf[..n]).expect("ascii"));
    }
    reforge_by_key(&reforge_key(name))
}

/// Stat keys in reforges.json that `Stat` does not know (dropped at load). Zero for the shipped data.
pub fn reforge_dropped_keys() -> usize {
    REFORGES.dropped
}

// ---------------------------------------------------------------------------------------------------------------
// Accessories
// ---------------------------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawAccessories {
    upgrades: IndexMap<String, Vec<String>>,
    ignored: Vec<String>,
}

/// accessories.json plus the families built from it (stats.js `ACCESSORY_FAMILY`, `CHAIN_POSITION`).
pub struct AccessoryData {
    /// `ACCESSORY_DATA.upgrades`, in key order: first tier -> upgrade chain.
    pub upgrades: Vec<(&'static str, &'static [&'static str])>,
    /// `ACCESSORY_DATA.ignored`.
    pub ignored: &'static [&'static str],
    ignored_set: FxHashSet<&'static str>,
    family: FxHashMap<&'static str, &'static str>,
    chain_position: FxHashMap<&'static str, usize>,
}

static ACCESSORIES: LazyLock<AccessoryData> = LazyLock::new(|| {
    let raw: RawAccessories = serde_json::from_str(ACCESSORIES_JSON).expect("accessories.json");
    let upgrades: Vec<(&'static str, &'static [&'static str])> = js_order(raw.upgrades)
        .into_iter()
        .map(|(k, v)| (leak(k), leak_slice(v.into_iter().map(leak).collect())))
        .collect();
    let ignored = leak_slice(raw.ignored.into_iter().map(leak).collect());
    // The JS union-find, step for step (no path compression), so every family keeps the same root id.
    let mut parent: IndexMap<&'static str, &'static str> = IndexMap::new();
    fn find(parent: &mut IndexMap<&'static str, &'static str>, x: &'static str) -> &'static str {
        if !parent.contains_key(x) {
            parent.insert(x, x);
        }
        let mut x = x;
        while parent[x] != x {
            x = parent[x];
        }
        x
    }
    let mut chain_position: FxHashMap<&'static str, usize> = FxHashMap::default();
    for &(id, ups) in &upgrades {
        let chain = std::iter::once(id).chain(ups.iter().copied());
        for (i, member) in chain.enumerate() {
            let pos = chain_position.entry(member).or_insert(0);
            *pos = (*pos).max(i);
            let a = find(&mut parent, member);
            let b = find(&mut parent, id);
            parent.insert(a, b);
        }
    }
    let keys: Vec<&'static str> = parent.keys().copied().collect();
    let mut family = FxHashMap::default();
    for id in keys {
        let root = find(&mut parent, id);
        family.insert(id, root);
    }
    AccessoryData { upgrades, ignored, ignored_set: ignored.iter().copied().collect(), family, chain_position }
});

/// accessories.json and its derived families.
#[inline]
pub fn accessory_data() -> &'static AccessoryData {
    &ACCESSORIES
}

/// JS `accessoryFamily(id)`: `HATCESSORY` for hatcessories, else the family root id, else `None`.
#[inline]
pub fn accessory_family(id: &str) -> Option<&'static str> {
    if crate::config::accessories::is_hatcessory(id) {
        return Some("HATCESSORY");
    }
    ACCESSORIES.family.get(id).copied()
}

/// JS `accessoryChainPosition(id)`: position in its upgrade chain (0 for the first tier or no chain).
#[inline]
pub fn accessory_chain_position(id: &str) -> usize {
    ACCESSORIES.chain_position.get(id).copied().unwrap_or(0)
}

/// JS `IGNORED_ACCESSORIES.has(id)` (stats.js).
#[inline]
pub fn is_ignored_accessory(id: &str) -> bool {
    ACCESSORIES.ignored_set.contains(id)
}

// ---------------------------------------------------------------------------------------------------------------
// Fragged items
// ---------------------------------------------------------------------------------------------------------------

/// One fragged.json entry: `from` -> `{ id, parts }` (the STARRED_ item and its fragments).
#[derive(Clone, Copy, Debug)]
pub struct Fragged {
    pub from: &'static str,
    pub id: &'static str,
    pub parts: &'static [Part],
}

#[derive(Deserialize)]
struct RawPart {
    id: String,
    qty: f64,
}

#[derive(Deserialize)]
struct RawFragged {
    id: String,
    parts: Vec<RawPart>,
}

static FRAGGED: LazyLock<(Vec<Fragged>, FxHashMap<&'static str, usize>)> = LazyLock::new(|| {
    let raw: IndexMap<String, RawFragged> = serde_json::from_str(FRAGGED_JSON).expect("fragged.json");
    let list: Vec<Fragged> = js_order(raw)
        .into_iter()
        .map(|(from, f)| Fragged {
            from: leak(from),
            id: leak(f.id),
            parts: leak_slice(f.parts.into_iter().map(|p| Part { id: leak(p.id), qty: p.qty }).collect()),
        })
        .collect();
    let index = list.iter().enumerate().map(|(i, f)| (f.from, i)).collect();
    (list, index)
});

/// fragged.json, in key order.
#[inline]
pub fn all_fragged() -> &'static [Fragged] {
    &FRAGGED.0
}

/// JS `FRAGGED[fromId]`.
#[inline]
pub fn fragged(from: &str) -> Option<&'static Fragged> {
    FRAGGED.1.get(from).map(|&i| &FRAGGED.0[i])
}

// ---------------------------------------------------------------------------------------------------------------
// Pets
// ---------------------------------------------------------------------------------------------------------------

/// `PET_DATA.stats[type][tier]`: stat numbers at level 1 (`min`) and max level (`max`), perk numbers, curve.
#[derive(Clone, Copy, Debug)]
pub struct PetTierData {
    pub tier: &'static str,
    pub min: &'static [(Stat, f64)],
    pub max: &'static [(Stat, f64)],
    pub other_min: &'static [f64],
    pub other_max: &'static [f64],
    /// Raw `curve` string ("101:200:1"), if any.
    pub curve: Option<&'static str>,
    /// `curve.split(':').map(Number)`: `[from, to]` (NaN for missing parts).
    pub curve_range: Option<(f64, f64)>,
}

/// One pet type of `PET_DATA.stats`, with its tiers in key order.
#[derive(Clone, Copy, Debug)]
pub struct PetTypeData {
    pub kind: &'static str,
    pub tiers: &'static [PetTierData],
}

/// `PET_DATA.custom[type]`.
#[derive(Clone, Copy, Debug)]
pub struct PetCustom {
    pub levels: &'static [f64],
    pub max_level: f64,
    /// JS `type` (1 = the custom levels extend the table).
    pub kind: f64,
}

/// pets.json.
pub struct PetDataSet {
    /// `PET_DATA.levels`.
    pub levels: &'static [f64],
    /// `PET_DATA.rarityOffset`, in key order.
    pub rarity_offset: Vec<(&'static str, f64)>,
    /// `PET_DATA.custom`, in key order.
    pub custom: Vec<(&'static str, PetCustom)>,
    /// `PET_DATA.stats`, in key order (= `allPetTypes()`).
    pub types: Vec<PetTypeData>,
    type_index: FxHashMap<&'static str, usize>,
    pub(crate) dropped: usize,
}

#[derive(Deserialize)]
struct RawPetCustom {
    #[serde(default)]
    levels: Vec<f64>,
    #[serde(rename = "maxLevel", default)]
    max_level: Option<f64>,
    #[serde(rename = "type", default)]
    kind: Option<f64>,
}

#[derive(Deserialize)]
struct RawPetTier {
    #[serde(default)]
    min: IndexMap<String, f64>,
    #[serde(default)]
    max: IndexMap<String, f64>,
    #[serde(rename = "otherMin", default)]
    other_min: Vec<f64>,
    #[serde(rename = "otherMax", default)]
    other_max: Vec<f64>,
    #[serde(default)]
    curve: Option<String>,
}

#[derive(Deserialize)]
struct RawPets {
    levels: Vec<f64>,
    #[serde(rename = "rarityOffset")]
    rarity_offset: IndexMap<String, f64>,
    custom: IndexMap<String, RawPetCustom>,
    stats: IndexMap<String, IndexMap<String, RawPetTier>>,
}

static PETS: LazyLock<PetDataSet> = LazyLock::new(|| {
    let raw: RawPets = serde_json::from_str(PETS_JSON).expect("pets.json");
    let mut dropped = 0;
    let custom = js_order(raw.custom)
        .into_iter()
        .map(|(k, c)| {
            (leak(k), PetCustom { levels: leak_slice(c.levels), max_level: c.max_level.unwrap_or(100.0), kind: c.kind.unwrap_or(0.0) })
        })
        .collect();
    let types: Vec<PetTypeData> = js_order(raw.stats)
        .into_iter()
        .map(|(kind, tiers)| {
            let tiers = js_order(tiers)
                .into_iter()
                .map(|(tier, d)| {
                    let curve_range = d.curve.as_deref().map(|c| {
                        let mut parts = c.split(':').map(crate::text::js_number);
                        (parts.next().unwrap_or(f64::NAN), parts.next().unwrap_or(f64::NAN))
                    });
                    PetTierData {
                        tier: leak(tier),
                        min: stat_list(d.min, &mut dropped),
                        max: stat_list(d.max, &mut dropped),
                        other_min: leak_slice(d.other_min),
                        other_max: leak_slice(d.other_max),
                        curve: d.curve.map(leak),
                        curve_range,
                    }
                })
                .collect();
            PetTypeData { kind: leak(kind), tiers: leak_slice(tiers) }
        })
        .collect();
    let type_index = types.iter().enumerate().map(|(i, t)| (t.kind, i)).collect();
    PetDataSet {
        levels: leak_slice(raw.levels),
        rarity_offset: js_order(raw.rarity_offset).into_iter().map(|(k, v)| (leak(k), v)).collect(),
        custom,
        types,
        type_index,
        dropped,
    }
});

/// pets.json.
#[inline]
pub fn pet_data_set() -> &'static PetDataSet {
    &PETS
}

impl PetDataSet {
    /// `PET_DATA.stats[type]`.
    #[inline]
    pub fn pet_type(&self, kind: &str) -> Option<&PetTypeData> {
        self.type_index.get(kind).map(|&i| &self.types[i])
    }
    /// `PET_DATA.custom[type]`.
    pub fn custom(&self, kind: &str) -> Option<&PetCustom> {
        self.custom.iter().find(|(k, _)| *k == kind).map(|(_, c)| c)
    }
    /// `PET_DATA.rarityOffset[tier]`.
    pub fn rarity_offset(&self, tier: &str) -> Option<f64> {
        self.rarity_offset.iter().find(|(k, _)| *k == tier).map(|&(_, v)| v)
    }
}

impl PetTypeData {
    /// `PET_DATA.stats[type][tier]`.
    #[inline]
    pub fn tier(&self, tier: &str) -> Option<&'static PetTierData> {
        self.tiers.iter().find(|t| t.tier == tier)
    }
}

/// Stat keys in pets.json that `Stat` does not know (dropped at load). Zero for the shipped data.
pub fn pet_dropped_keys() -> usize {
    PETS.dropped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_loads() {
        assert!(all_reforges().len() > 100);
        assert_eq!(reforge_dropped_keys(), 0);
        let fabled = get_reforge("Fabled").expect("fabled");
        assert_eq!(fabled.key, "fabled");
        assert!(std::ptr::eq(get_reforge("FABLED").unwrap(), fabled));
        assert_eq!(get_reforge("ancient").unwrap().cata_stat, Some(Stat::CritDamage));
        assert!(get_reforge("not a reforge").is_none());
        assert!(fabled.stats_for("LEGENDARY").is_some());
        assert!(fabled.stats_for("SPECIAL").is_none());
        assert!(fabled.applies_to("HYPERION", Some("SWORD")));
        assert!(fabled.applies_to("X", Some("LONGSWORD")));
        assert!(!fabled.applies_to("X", Some("BOW")));
        assert!(!fabled.applies_to("X", None));

        let acc = accessory_data();
        assert!(acc.upgrades.len() > 100 && !acc.ignored.is_empty());
        assert!(is_ignored_accessory(acc.ignored[0]));
        let (first, chain) = acc.upgrades[0];
        assert_eq!(accessory_family(first), accessory_family(chain[0]));
        assert!(accessory_family(first).is_some());
        assert_eq!(accessory_family("PARTY_HAT_CRAB"), Some("HATCESSORY"));
        assert_eq!(accessory_family("NOT_AN_ACCESSORY"), None);
        assert!(accessory_chain_position(chain[0]) >= 1);

        assert!(!all_fragged().is_empty());
        assert_eq!(fragged("SHADOW_FURY").unwrap().id, "STARRED_SHADOW_FURY");

        let pets = pet_data_set();
        assert!(pets.types.len() > 50 && pets.levels.len() > 100);
        assert_eq!(pet_dropped_keys(), 0);
        let gd = pets.pet_type("GOLDEN_DRAGON").unwrap().tier("LEGENDARY").unwrap();
        assert_eq!(gd.curve_range, Some((101.0, 200.0)));
        assert_eq!(pets.custom("GOLDEN_DRAGON").unwrap().max_level, 200.0);
        assert_eq!(pets.rarity_offset("LEGENDARY"), Some(20.0));
    }

    #[test]
    fn js_key_order() {
        let m: IndexMap<String, i32> =
            [("b", 1), ("10", 2), ("a", 3), ("2", 4), ("01", 5)].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let keys: Vec<String> = js_order(m).into_keys().collect();
        assert_eq!(keys, ["2", "10", "b", "a", "01"]);
    }
}
