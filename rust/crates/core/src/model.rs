//! Shared data types: the contract between the crates.
//!
//! Field names and JSON shapes match the JavaScript bot's objects (camelCase, `null` / missing for `undefined`), so
//! everything the JS oracle dumps into `rust/testdata` deserializes straight into these types. Maps that the JS code
//! iterates keep their insertion order (`IndexMap`), because iteration order decides candidate order, tie-breaks
//! and float summation order.

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::Arc;

pub type Map<V> = IndexMap<String, V>;

/// JS truthiness for an optional JSON value (`false`, `0`, `""`, `null` and missing are falsy).
pub fn truthy(v: &Option<serde_json::Value>) -> bool {
    match v {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Some(serde_json::Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// A JSON object whose values the JS code only reads when `typeof value === 'number'`: other values are dropped.
pub fn de_number_map<'de, D: Deserializer<'de>>(d: D) -> Result<Map<f64>, D::Error> {
    let raw: Option<Map<serde_json::Value>> = Option::deserialize(d)?;
    Ok(raw
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(k, v)| v.as_f64().map(|x| (k, x)))
        .collect())
}

// ---------------------------------------------------------------------------------------------------------------
// Hypixel items resource (`/v2/resources/skyblock/items`): only the fields the bot reads.
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GemCost {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub coins: Option<f64>,
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub amount: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GemSlot {
    #[serde(default)]
    pub slot_type: String,
    /// `None` = free slot (JS `!slot.costs`). `Some(vec![])` still counts as locked.
    #[serde(default)]
    pub costs: Option<Vec<GemCost>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Requirement {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub level: Option<f64>,
    #[serde(default)]
    pub tier: Option<f64>,
    #[serde(default)]
    pub dungeon_type: Option<String>,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub slayer_boss_type: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ItemInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tier: Option<String>,
    /// API stats; non-number values are dropped (JS reads them only when they are numbers).
    #[serde(default, deserialize_with = "de_number_map")]
    pub stats: Map<f64>,
    /// Per-floor stats (floor-scaled dungeon gear). Kept raw: JS only uses arrays with at least one entry.
    #[serde(default)]
    pub tiered_stats: Option<Map<serde_json::Value>>,
    #[serde(default)]
    pub dungeon_item: Option<serde_json::Value>,
    #[serde(default)]
    pub dungeon_item_conversion_cost: Option<serde_json::Value>,
    #[serde(default)]
    pub can_recombobulate: Option<bool>,
    #[serde(default)]
    pub gemstone_slots: Option<Vec<GemSlot>>,
    #[serde(default)]
    pub requirements: Option<Vec<Requirement>>,
    #[serde(default)]
    pub catacombs_requirements: Option<Vec<Requirement>>,
    #[serde(default)]
    pub soulbound: Option<serde_json::Value>,
}

impl ItemInfo {
    /// JS `info.dungeon_item` truthiness.
    pub fn is_dungeon_item(&self) -> bool {
        truthy(&self.dungeon_item)
    }
    /// JS `Boolean(info?.dungeon_item || info?.dungeon_item_conversion_cost)`: dungeonizable gear.
    pub fn is_dungeonizable(&self) -> bool {
        truthy(&self.dungeon_item) || truthy(&self.dungeon_item_conversion_cost)
    }
}

/// The items resource keyed by ITEM_ID, in API order (JS `Map` insertion order).
pub type Resource = IndexMap<String, ItemInfo>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    pub max: f64,
}

/// Collections resource: id -> { name, max }, in API order.
pub type CollectionsResource = IndexMap<String, CollectionInfo>;

// ---------------------------------------------------------------------------------------------------------------
// Items and pets read from a profile (src/lib/nbt.js toItemState, src/api/hypixel.js readMember).
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Gem {
    pub gem: String,
    pub quality: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gems {
    /// Slot key (`COMBAT_0`, `JASPER_0`) -> gem, in NBT order.
    #[serde(default)]
    pub slots: Map<Gem>,
    #[serde(default)]
    pub unlocked: Vec<String>,
}

/// Parsed `petInfo` NBT string (auction pets; pricing only).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetInfo {
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub exp: Option<f64>,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub held_item: Option<String>,
}

/// One item (toItemState output, or an item the planner created). Never mutated once built: every change is a new
/// item, like in the JS code.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemState {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub base_rarity: String,
    #[serde(default)]
    pub recomb: bool,
    #[serde(default)]
    pub stars: i32,
    #[serde(default)]
    pub master_stars: i32,
    /// `true` or JS `undefined` (never `false` from toItemState / newItem).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dungeon: Option<bool>,
    /// Enchant key (lower case) -> level, in NBT order.
    #[serde(default)]
    pub enchants: Map<i32>,
    #[serde(default)]
    pub hpb: i32,
    #[serde(default)]
    pub art_of_war: bool,
    #[serde(default)]
    pub reforge: Option<String>,
    #[serde(default)]
    pub gems: Gems,
    #[serde(default)]
    pub enrichment: Option<String>,
    #[serde(default)]
    pub scrolls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pet: Option<PetInfo>,
    /// Floor-scaled dungeon drops: the floor it dropped on (`item_tier`) and its stat boost (0-50). Pricing only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stat_boost: Option<f64>,
    /// Set by the planner on a dual-wielded weapon.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dual_wield: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pet {
    #[serde(rename = "type")]
    pub kind: String,
    pub tier: String,
    #[serde(default)]
    pub held_item: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UnmaxedCollection {
    pub id: String,
    pub name: String,
    pub amount: f64,
    pub max: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Collections {
    pub maxed: i32,
    pub unmaxed: Vec<UnmaxedCollection>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tuning {
    #[serde(default)]
    pub allocation: Map<f64>,
    #[serde(default)]
    pub split: Option<Map<f64>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cake {
    pub stat: String,
    pub amount: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStats {
    #[serde(default)]
    pub sb_level: i32,
    #[serde(default)]
    pub bop_bonus: Option<String>,
    #[serde(default)]
    pub harp_songs: Vec<String>,
    #[serde(default)]
    pub cakes: Vec<Cake>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetProfile {
    /// Coins in the bank (Golden Dragon). `None` = bank API off (JS `undefined`).
    #[serde(default)]
    pub bank: Option<f64>,
    /// The player's own Gold collection. `None` = unknown (counts as maxed).
    #[serde(default)]
    pub gold_collection: Option<f64>,
}

/// readMember output: everything the planner needs from one profile member.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub items: Vec<ItemState>,
    pub cata_level: i32,
    pub archer_level: i32,
    pub berserk_level: i32,
    pub mage_level: i32,
    pub cata_xp: f64,
    pub secrets: f64,
    /// Skill levels by name (`COMBAT`, `HUNTING`, ...); `None` when the skills API is off.
    #[serde(default)]
    pub skills: Option<Map<i32>>,
    /// Slayer levels by boss (`zombie`, `wolf`, ...); `None` when the profile hides them.
    #[serde(default)]
    pub slayers: Option<Map<i32>>,
    pub combat_level: i32,
    #[serde(default)]
    pub collections: Option<Collections>,
    pub pets: Vec<Pet>,
    #[serde(default)]
    pub power: Option<String>,
    #[serde(default)]
    pub unlocked_powers: Vec<String>,
    #[serde(default)]
    pub tuning: Option<Tuning>,
    #[serde(default)]
    pub shards: Map<i32>,
    /// Essence shop perks: key -> level.
    #[serde(default, deserialize_with = "de_number_map")]
    pub perks: Map<f64>,
    #[serde(default)]
    pub rift_prism: Option<bool>,
    #[serde(default)]
    pub highest_ap: Option<f64>,
    pub profile_speed: i32,
    pub profile_stats: ProfileStats,
    pub pet_profile: PetProfile,
}

/// A Minecraft account (Mojang lookup).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub uuid: String,
    pub name: String,
}

// ---------------------------------------------------------------------------------------------------------------
// Prices.
// ---------------------------------------------------------------------------------------------------------------

/// One frozen set of price tables (bazaar instant buy, auction house smoothed lowest BIN). Swapped as a whole on
/// refresh, never edited, so a plan prices every step the same way.
#[derive(Clone, Debug, Default)]
pub struct PriceTables {
    pub bazaar: rustc_hash::FxHashMap<String, f64>,
    pub auctions: rustc_hash::FxHashMap<String, f64>,
    /// Bumped on every refresh; plan results can be cached per version.
    pub version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceSource {
    Bazaar,
    Auction,
}

impl PriceSource {
    pub fn code(self) -> &'static str {
        match self {
            PriceSource::Bazaar => "BZ",
            PriceSource::Auction => "AH",
        }
    }
}

impl PriceTables {
    /// JS priceLookup: bazaar first, then the auction house.
    pub fn get(&self, id: &str) -> Option<(f64, PriceSource)> {
        if let Some(p) = self.bazaar.get(id) {
            return Some((*p, PriceSource::Bazaar));
        }
        self.auctions.get(id).map(|p| (*p, PriceSource::Auction))
    }
}

pub type SharedPrices = Arc<PriceTables>;

// ---------------------------------------------------------------------------------------------------------------
// A /dps job and its result (src/calc/run.js runDps).
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Blessings {
    pub power: i32,
    pub wisdom: i32,
    pub time: i32,
}

impl Default for Blessings {
    fn default() -> Self {
        Blessings { power: 29, wisdom: 14, time: 5 }
    }
}

/// What to plan. Strings use the JS values: cls `arch` / `bers` / `mage`, target `wither` / `dragon` / none,
/// mode `lcm` / `rcm`, floor_mode `master` / `normal`, upgrade_mode `all` / `max`, menu `coins` / `profile`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DpsJob {
    pub cls: String,
    #[serde(default)]
    pub target: Option<String>,
    pub mode: String,
    pub floor_mode: String,
    pub upgrade_mode: String,
    pub menu: String,
    /// Plan steps (PLAN_STEPS = 40 in the bot).
    #[serde(default)]
    pub limit: Option<usize>,
    pub blessings: Blessings,
    #[serde(default)]
    pub any_chest: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Weights {
    pub wither: f64,
    pub dragon: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatLine {
    pub strength: f64,
    pub crit_damage: f64,
    pub crit_chance: f64,
    pub intelligence: f64,
    pub ability_damage: f64,
    pub attack_speed: f64,
    pub speed: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TargetStats {
    pub wither: StatLine,
    pub dragon: StatLine,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TuningPoints {
    pub attack_speed: f64,
    pub speed: f64,
    pub dps: f64,
}

/// Damage shown with a plan: per-target damage, Accessory Power, tuning points and final stats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Damage {
    pub wither: f64,
    pub dragon: f64,
    pub ap: f64,
    pub tuning: TuningPoints,
    pub stats: TargetStats,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub label: String,
    pub cost: f64,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unverified: Option<bool>,
    pub gain: f64,
    pub per_million: f64,
    pub wither_gain: f64,
    pub dragon_gain: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub steps: Vec<Step>,
    pub start_damage: Damage,
    pub end_damage: Damage,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileRow {
    pub label: String,
    pub gain: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileMenu {
    pub rows: Vec<ProfileRow>,
    pub damage: Damage,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DpsResult {
    pub weights: Weights,
    pub menu: String,
    pub floor_mode: String,
    /// Coin plans only (the best leftover tuning split, stat -> share).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuning_split: Option<Map<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Plan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProfileMenu>,
}
