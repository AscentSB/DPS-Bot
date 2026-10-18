//! src/lib/drops.js: floor-scaled dungeon drops (items with `tiered_stats`).

use crate::config::constants::{DUNGEON_DROP_BOOST, DUNGEON_FLOOR_CATA, RARITIES};
use crate::model::Resource;
use std::borrow::Cow;

/// JS `isFloorDrop(id, resource)`: `tiered_stats` and `dungeon_item` both truthy.
#[inline]
pub fn is_floor_drop(id: &str, resource: &Resource) -> bool {
    resource.get(id).is_some_and(|info| info.tiered_stats.is_some() && info.is_dungeon_item())
}

/// JS `highestFloor(cataLevel)`: highest floor (0-10) the Catacombs level allows.
#[inline]
pub fn highest_floor(cata_level: f64) -> usize {
    DUNGEON_FLOOR_CATA.iter().filter(|&&need| cata_level >= need).count()
}

/// JS `floorTierIndex(cataLevel, length)`: index into a `tiered_stats` array (floor 10 = last entry).
#[inline]
pub fn floor_tier_index(cata_level: f64, length: usize) -> usize {
    let v = (length as i64 - 1).min(highest_floor(cata_level) as i64 - 1);
    v.max(0) as usize
}

/// JS `dropStatMultiplier(id, resource)`: 1.5 for floor drops, else 1.
#[inline]
pub fn drop_stat_multiplier(id: &str, resource: &Resource) -> f64 {
    if is_floor_drop(id, resource) {
        1.0 + DUNGEON_DROP_BOOST
    } else {
        1.0
    }
}

/// JS `dropBaseRarity(id, resource)`: the item's tier (`COMMON` when unknown), one higher for floor drops.
pub fn drop_base_rarity<'a>(id: &str, resource: &'a Resource) -> &'a str {
    let tier: &'a str = resource.get(id).and_then(|i| i.tier.as_deref()).unwrap_or("COMMON");
    if !is_floor_drop(id, resource) {
        return tier;
    }
    match RARITIES.iter().position(|&r| r == tier) {
        Some(i) if i < RARITIES.len() - 1 => RARITIES[i + 1],
        _ => tier,
    }
}

/// JS `FULL_STAT_BOOST`.
pub const FULL_STAT_BOOST: f64 = 50.0;

/// JS `floorDropKey(id, floor)`: `${id}_FLOOR${floor}`.
pub fn floor_drop_key(id: &str, floor: f64) -> String {
    let mut s = String::with_capacity(id.len() + 8);
    s.push_str(id);
    s.push_str("_FLOOR");
    crate::jsfmt::write_js_string(&mut s, floor);
    s
}

/// JS `buyPriceId(id, resource, cataLevel)`: floor drops are priced as the fully boosted piece from the highest
/// floor the player can enter.
pub fn buy_price_id<'a>(id: &'a str, resource: &Resource, cata_level: f64) -> Cow<'a, str> {
    if is_floor_drop(id, resource) {
        Cow::Owned(floor_drop_key(id, highest_floor(cata_level).max(1) as f64))
    } else {
        Cow::Borrowed(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ItemInfo;

    fn resource() -> Resource {
        let mut r = Resource::default();
        let mut sm: ItemInfo = serde_json::from_str(
            r#"{"id":"SKELETON_MASTER_CHESTPLATE","tier":"EPIC","dungeon_item":true,"tiered_stats":{"DEFENSE":[1,2]}}"#,
        )
        .unwrap();
        r.insert(sm.id.clone(), sm.clone());
        sm.id = "PLAIN".into();
        sm.tiered_stats = None;
        r.insert("PLAIN".into(), sm);
        r
    }

    #[test]
    fn floors() {
        let r = resource();
        assert!(is_floor_drop("SKELETON_MASTER_CHESTPLATE", &r));
        assert!(!is_floor_drop("PLAIN", &r));
        assert!(!is_floor_drop("MISSING", &r));
        assert_eq!(highest_floor(0.0), 0);
        assert_eq!(highest_floor(36.0), 10);
        assert_eq!(highest_floor(f64::INFINITY), 10);
        assert_eq!(highest_floor(f64::NAN), 0);
        assert_eq!(floor_tier_index(36.0, 7), 6);
        assert_eq!(floor_tier_index(0.0, 7), 0);
        assert_eq!(floor_tier_index(10.0, 7), 3);
        assert_eq!(drop_base_rarity("SKELETON_MASTER_CHESTPLATE", &r), "LEGENDARY");
        assert_eq!(drop_base_rarity("PLAIN", &r), "EPIC");
        assert_eq!(drop_base_rarity("MISSING", &r), "COMMON");
        assert_eq!(buy_price_id("SKELETON_MASTER_CHESTPLATE", &r, 0.0), "SKELETON_MASTER_CHESTPLATE_FLOOR1");
        assert_eq!(buy_price_id("SKELETON_MASTER_CHESTPLATE", &r, 34.0), "SKELETON_MASTER_CHESTPLATE_FLOOR9");
        assert_eq!(buy_price_id("PLAIN", &r, 34.0), "PLAIN");
        assert_eq!(drop_stat_multiplier("SKELETON_MASTER_CHESTPLATE", &r), 1.5);
    }
}
