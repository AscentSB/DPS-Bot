//! Ports of src/config/*.js. Every table keeps the JS key order (tables are ordered slices; `static` so each entry
//! has one address). Lookups by key are small functions (`enchant(key)`, `power(key)`, ...).

pub mod accessories;
pub mod armor_sets;
pub mod constants;
pub mod enchants;
pub mod essence;
pub mod gemstones;
pub mod item_stats;
pub mod item_upgrades;
pub mod pets;
pub mod profile;
pub mod shards;
pub mod upgrades;
