//! Small pure helpers from src/calc/stats.js and src/calc/upgrades.js that do not need the arena.

use dps_core::config::accessories::NO_RECOMB;
use dps_core::config::constants::*;
use dps_core::config::item_upgrades::item_upgrades;
use dps_core::data::{accessory_chain_position, accessory_family, fragged};
use dps_core::drops::drop_base_rarity;
use dps_core::model::{Gems, ItemState, Resource};
use dps_core::text::clean_name;

/// JS `itemRarity(item)` for an item state.
pub fn item_rarity_of(item: &ItemState) -> &str {
    item_rarity(&item.base_rarity, item.recomb, &item.id)
}

/// JS `isDungeon(item, resource)`.
pub fn is_dungeon(item: &ItemState, resource: &Resource) -> bool {
    if let Some(d) = dps_core::config::item_stats::item_stats(&item.id).and_then(|o| o.dungeon) {
        return d;
    }
    if let Some(d) = item.dungeon {
        return d;
    }
    // Dungeonizable items (e.g. Terminator) are assumed converted and starred.
    resource.get(&item.id).is_some_and(|i| i.is_dungeonizable())
}

/// JS `hasWitherImpact(item)`: a Necron's Blade variant with all 3 scrolls, or the Ultimate Wither Scroll.
pub fn has_wither_impact(item: &ItemState) -> bool {
    if !WITHER_IMPACT_WEAPONS.contains(&item.id.as_str()) {
        return false;
    }
    item.scrolls.iter().any(|s| s == ULTIMATE_WITHER_SCROLL) || WITHER_SCROLLS.iter().all(|w| item.scrolls.iter().any(|s| s == w))
}

/// JS `canRecomb(item, resource)`.
pub fn can_recomb(item: &ItemState, resource: &Resource) -> bool {
    if NO_RECOMB.contains(&item.id.as_str()) || is_max_reforge_item(&item.id) {
        return false;
    }
    if resource.get(&item.id).and_then(|i| i.can_recombobulate) == Some(false) {
        return false;
    }
    !item.recomb && item_rarity(&item.base_rarity, true, &item.id) != item.base_rarity
}

/// JS `newItem(id, resource)`: a clean item bought from the market. Dungeonizable gear is assumed converted and
/// 5-starred.
pub fn new_item(id: &str, resource: &Resource) -> ItemState {
    let info = resource.get(id);
    let dungeonizable = info.is_some_and(|i| i.is_dungeonizable());
    ItemState {
        id: id.to_string(),
        name: clean_name(info.and_then(|i| i.name.as_deref()).unwrap_or(id)),
        category: info.and_then(|i| i.category.clone()),
        base_rarity: if info.is_some() { drop_base_rarity(id, resource).to_string() } else { "COMMON".to_string() },
        recomb: false,
        stars: if dungeonizable { MAX_STARS as i32 } else { 0 },
        master_stars: 0,
        dungeon: if dungeonizable { Some(true) } else { None },
        enchants: Default::default(),
        hpb: 0,
        art_of_war: false,
        reforge: None,
        gems: Gems::default(),
        enrichment: None,
        scrolls: Vec::new(),
        pet: None,
        floor: None,
        stat_boost: None,
        dual_wield: false,
    }
}

/// JS `upgradesInto(fromId, toId)`: `fromId` turns into `toId` and keeps its upgrades (fragging, Golden -> Diamond
/// heads, Wither armor -> Necron's / Storm's / Maxor's / Goldor's).
pub fn upgrades_into(from_id: &str, to_id: &str) -> bool {
    fragged(from_id).is_some_and(|f| f.id == to_id) || item_upgrades(from_id).is_some_and(|u| u.iter().any(|x| x.id == to_id))
}

fn stat_rarity_index(rarity: &str) -> i64 {
    rarity_index(stat_rarity(rarity)).map(|i| i as i64).unwrap_or(-1)
}

fn family_rank(rarity: &str, id: &str) -> i64 {
    stat_rarity_index(rarity) * 100 + accessory_chain_position(id) as i64
}

/// JS `accessorySuperseded(id, accessoryIds, resource)`: true when an owned accessory of the same family is at least
/// as good (by base rarity when the resource is given, otherwise by chain position).
pub fn accessory_superseded<'a>(id: &str, accessory_ids: impl IntoIterator<Item = &'a str>, resource: Option<&Resource>) -> bool {
    let Some(family) = accessory_family(id) else { return false };
    let rank = |x: &str| match resource {
        Some(r) => family_rank(drop_base_rarity(x, r), x),
        None => accessory_chain_position(x) as i64,
    };
    let mine = rank(id);
    accessory_ids.into_iter().any(|other| other != id && accessory_family(other) == Some(family) && rank(other) >= mine)
}

/// JS `statRarity(rarity)`.
pub use dps_core::config::constants::stat_rarity;

/// JS `isWeaponCategory(category)`.
pub use dps_core::config::constants::is_weapon_category;

/// JS `reforgeKey(name)` / `getReforge(name)` / `allReforges()` / `reforgeAppliesTo(reforge, item)`.
pub use dps_core::data::{all_reforges, get_reforge, reforge_key};

/// JS `reforgeAppliesTo(reforge, item)`.
pub fn reforge_applies_to(reforge: &dps_core::data::Reforge, item: &ItemState) -> bool {
    reforge.applies_to(&item.id, item.category.as_deref())
}

/// JS `gemStat(gem, quality, rarity)`.
pub use dps_core::config::gemstones::gem_stat;
