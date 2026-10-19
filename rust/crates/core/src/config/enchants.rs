//! src/config/enchants.js: the enchant stat table.

use crate::kinds::{Class, FloorMode, StatTarget};
use crate::stat::Stat;

/// One entry of JS `ENCHANTS`. Optional JS fields that no entry sets (`classes`, `items`) are kept as `None` so the
/// port stays faithful if they are added.
#[derive(Clone, Copy, Debug)]
pub struct Enchant {
    /// NBT key (`giant_killer`).
    pub key: &'static str,
    /// Label when the NBT key differs from the in-game name (`name`), else `None`.
    pub name: Option<&'static str>,
    pub categories: &'static [&'static str],
    pub target: StatTarget,
    /// Stat values per level (index 0 = level 1), in JS key order.
    pub stats: &'static [(Stat, &'static [f64])],
    pub conflicts: &'static [&'static str],
    pub classes: Option<&'static [Class]>,
    pub items: Option<&'static [&'static str]>,
    pub melee: bool,
    pub floor_mode: Option<FloorMode>,
    pub swarm: bool,
    pub thunder: Option<&'static [f64]>,
    pub fire: Option<&'static [f64]>,
    pub clear_mobs: bool,
    pub legion: bool,
    pub chimera: bool,
    pub the_one: Option<&'static [f64]>,
    pub min_level: Option<i32>,
    pub max_level: Option<i32>,
}

const SWORDS: &[&str] = &["SWORD", "LONGSWORD"];
const SWORDS_BOW: &[&str] = &["SWORD", "LONGSWORD", "BOW"];
const BOW: &[&str] = &["BOW"];
const ARMOR: &[&str] = &["HELMET", "CHESTPLATE", "LEGGINGS", "BOOTS"];

const BASE: Enchant = Enchant {
    key: "",
    name: None,
    categories: &[],
    target: StatTarget::Global,
    stats: &[],
    conflicts: &[],
    classes: None,
    items: None,
    melee: false,
    floor_mode: None,
    swarm: false,
    thunder: None,
    fire: None,
    clear_mobs: false,
    legion: false,
    chimera: false,
    the_one: None,
    min_level: None,
    max_level: None,
};

/// JS `ENCHANTS`, in key order.
pub static ENCHANTS: &[Enchant] = &[
    Enchant {
        key: "giant_killer",
        categories: SWORDS,
        floor_mode: Some(FloorMode::Normal),
        stats: &[(Stat::AdditiveDamage, &[5.0, 10.0, 15.0, 20.0, 30.0, 45.0, 65.0])],
        conflicts: &["titan_killer"],
        ..BASE
    },
    Enchant {
        key: "titan_killer",
        categories: SWORDS,
        floor_mode: Some(FloorMode::Master),
        stats: &[(Stat::AdditiveDamage, &[6.0, 12.0, 18.0, 24.0, 40.0, 60.0, 80.0])],
        conflicts: &["giant_killer"],
        ..BASE
    },
    Enchant {
        key: "prosecute",
        categories: SWORDS,
        stats: &[(Stat::AdditiveDamage, &[10.0, 20.0, 30.0, 40.0, 70.0, 100.0])],
        ..BASE
    },
    Enchant {
        key: "first_strike",
        categories: SWORDS,
        melee: true,
        floor_mode: Some(FloorMode::Normal),
        stats: &[(Stat::AdditiveDamage, &[25.0, 50.0, 75.0, 100.0, 125.0])],
        conflicts: &["triple_strike"],
        ..BASE
    },
    Enchant {
        key: "triple_strike",
        categories: SWORDS,
        melee: true,
        floor_mode: Some(FloorMode::Master),
        stats: &[(Stat::AdditiveDamage, &[10.0, 20.0, 30.0, 40.0, 50.0])],
        conflicts: &["first_strike"],
        ..BASE
    },
    Enchant {
        key: "fire_aspect",
        categories: SWORDS,
        target: StatTarget::Wither,
        fire: Some(&[3.0, 6.0, 9.0]),
        max_level: Some(3),
        ..BASE
    },
    Enchant {
        key: "thunderlord",
        categories: SWORDS,
        melee: true,
        thunder: Some(&[8.0, 16.0, 24.0, 32.0, 40.0, 50.0, 60.0]),
        max_level: Some(7),
        conflicts: &["thunderbolt"],
        ..BASE
    },
    Enchant {
        key: "ultimate_swarm",
        name: Some("Swarm"),
        categories: SWORDS_BOW,
        swarm: true,
        stats: &[(Stat::AdditiveDamage, &[2.0, 4.0, 6.0, 8.0, 10.0])],
        conflicts: &["ultimate_chimera", "ultimate_reiterate"],
        ..BASE
    },
    Enchant { key: "vicious", categories: SWORDS_BOW, stats: &[(Stat::Ferocity, &[0.0, 0.0, 3.0, 4.0, 5.0])], ..BASE },
    // Hypixel lets Sharpness and Smite sit on the same sword, so they do not conflict.
    Enchant {
        key: "sharpness",
        categories: SWORDS,
        melee: true,
        stats: &[(Stat::AdditiveDamage, &[5.0, 10.0, 15.0, 20.0, 30.0, 45.0, 65.0])],
        conflicts: &["bane_of_arthropods"],
        ..BASE
    },
    Enchant {
        key: "power",
        categories: BOW,
        stats: &[(Stat::AdditiveDamage, &[8.0, 16.0, 24.0, 32.0, 40.0, 50.0, 65.0])],
        ..BASE
    },
    Enchant {
        key: "critical",
        categories: SWORDS,
        stats: &[(Stat::CritDamage, &[10.0, 20.0, 30.0, 40.0, 50.0, 70.0, 100.0])],
        ..BASE
    },
    Enchant {
        key: "smite",
        categories: SWORDS,
        target: StatTarget::Wither,
        clear_mobs: true,
        stats: &[(Stat::AdditiveDamage, &[10.0, 20.0, 30.0, 40.0, 60.0, 80.0, 100.0])],
        conflicts: &["bane_of_arthropods"],
        ..BASE
    },
    Enchant {
        key: "ender_slayer",
        categories: SWORDS,
        target: StatTarget::Dragon,
        stats: &[(Stat::AdditiveDamage, &[15.0, 30.0, 45.0, 60.0, 80.0, 100.0, 130.0])],
        ..BASE
    },
    Enchant {
        key: "dragon_hunter",
        categories: BOW,
        target: StatTarget::Dragon,
        stats: &[(Stat::AdditiveDamage, &[8.0, 16.0, 24.0, 32.0, 40.0, 50.0])],
        ..BASE
    },
    Enchant { key: "snipe", categories: BOW, stats: &[(Stat::AdditiveDamage, &[1.0, 2.0, 3.0, 5.0])], ..BASE },
    Enchant {
        key: "overload",
        categories: BOW,
        stats: &[(Stat::CritDamage, &[1.0, 2.0, 3.0, 4.0, 5.0]), (Stat::CritChance, &[1.0, 2.0, 3.0, 4.0, 5.0])],
        ..BASE
    },
    Enchant { key: "toxophilite", categories: BOW, stats: &[(Stat::CritChance, &[10.0])], ..BASE },
    Enchant {
        key: "ultimate_reiterate",
        name: Some("Duplex"),
        categories: BOW,
        conflicts: &["ultimate_swarm"],
        stats: &[(Stat::Arrows, &[0.04, 0.08, 0.12, 0.16, 0.2])],
        ..BASE
    },
    Enchant { key: "sugar_rush", categories: &["BOOTS"], stats: &[(Stat::Speed, &[2.0, 4.0, 6.0])], ..BASE },
    Enchant {
        key: "big_brain",
        categories: &["HELMET"],
        stats: &[(Stat::Intelligence, &[5.0, 10.0, 15.0, 20.0, 25.0])],
        ..BASE
    },
    Enchant {
        key: "smarty_pants",
        categories: &["LEGGINGS"],
        stats: &[(Stat::Intelligence, &[5.0, 10.0, 15.0, 20.0, 25.0])],
        ..BASE
    },
    Enchant { key: "ultimate_legion", categories: ARMOR, legion: true, max_level: Some(5), ..BASE },
    Enchant {
        key: "ultimate_chimera",
        categories: SWORDS,
        chimera: true,
        max_level: Some(5),
        conflicts: &["ultimate_swarm"],
        ..BASE
    },
    Enchant {
        key: "ultimate_the_one",
        name: Some("The One"),
        categories: &["NECKLACE"],
        the_one: Some(&[0.0, 0.0, 0.0, 0.1, 0.2]),
        min_level: Some(4),
        max_level: Some(5),
        ..BASE
    },
];

/// Index of an enchant in `ENCHANTS` (JS `ENCHANTS[key]`).
pub fn enchant_index(key: &str) -> Option<usize> {
    Some(match key {
        "giant_killer" => 0,
        "titan_killer" => 1,
        "prosecute" => 2,
        "first_strike" => 3,
        "triple_strike" => 4,
        "fire_aspect" => 5,
        "thunderlord" => 6,
        "ultimate_swarm" => 7,
        "vicious" => 8,
        "sharpness" => 9,
        "power" => 10,
        "critical" => 11,
        "smite" => 12,
        "ender_slayer" => 13,
        "dragon_hunter" => 14,
        "snipe" => 15,
        "overload" => 16,
        "toxophilite" => 17,
        "ultimate_reiterate" => 18,
        "sugar_rush" => 19,
        "big_brain" => 20,
        "smarty_pants" => 21,
        "ultimate_legion" => 22,
        "ultimate_chimera" => 23,
        "ultimate_the_one" => 24,
        _ => return None,
    })
}

/// JS `ENCHANTS[key]`.
#[inline]
pub fn enchant(key: &str) -> Option<&'static Enchant> {
    enchant_index(key).map(|i| &ENCHANTS[i])
}

impl Enchant {
    /// JS `enchantMaxLevel(ench)`: `maxLevel` when set (and nonzero), else the longest stat table.
    pub fn max_level(&self) -> i32 {
        if let Some(m) = self.max_level {
            if m != 0 {
                return m;
            }
        }
        self.stats.iter().map(|(_, v)| v.len() as i32).max().unwrap_or(0).max(0)
    }

    /// JS `ench.stats?.[stat]`.
    pub fn stat_values(&self, stat: Stat) -> Option<&'static [f64]> {
        self.stats.iter().find(|(s, _)| *s == stat).map(|&(_, v)| v)
    }

    /// JS `ench.categories.includes(category)`.
    #[inline]
    pub fn applies_to_category(&self, category: &str) -> bool {
        self.categories.contains(&category)
    }
}

/// JS `SKIPPED_SWORD_ENCHANTS`: non-DPS sword enchants, never bought when a blade copies another (spare blade,
/// dual-wield copy).
pub static SKIPPED_SWORD_ENCHANTS: &[&str] = &["divine_gift", "looting", "luck", "experience", "cleave", "smoldering", "cubism"];

/// JS `skippedSwordEnchant(key, category)`.
pub fn skipped_sword_enchant(key: &str, category: Option<&str>) -> bool {
    category.is_some_and(|c| SWORDS.contains(&c)) && SKIPPED_SWORD_ENCHANTS.contains(&key)
}

/// JS `enchantMaxLevel(ench)`.
#[inline]
pub fn enchant_max_level(ench: &Enchant) -> i32 {
    ench.max_level()
}

pub const GRAND_EXP_BOTTLE_ID: &str = "GRAND_EXP_BOTTLE";
pub const ENCHANT_TABLE_TWO_BOTTLES: f64 = 75.0;

/// JS `TABLE_ENCHANT_XP`, in key order: XP levels per level the enchanting table sells.
pub static TABLE_ENCHANT_XP: &[(&str, &[f64])] = &[
    ("sharpness", &[10.0, 15.0, 20.0, 25.0, 30.0]),
    ("smite", &[10.0, 15.0, 20.0, 25.0, 30.0]),
    ("giant_killer", &[10.0, 20.0, 30.0, 40.0, 50.0]),
    ("titan_killer", &[10.0, 20.0, 30.0, 40.0, 50.0]),
    ("critical", &[10.0, 20.0, 30.0, 40.0, 50.0]),
    ("first_strike", &[20.0, 30.0, 40.0, 75.0]),
    ("triple_strike", &[20.0, 30.0, 40.0, 75.0]),
    ("ender_slayer", &[10.0, 20.0, 25.0, 30.0, 40.0]),
    ("thunderlord", &[20.0, 25.0, 30.0, 40.0, 50.0]),
    ("prosecute", &[20.0, 25.0, 30.0, 40.0, 50.0]),
    ("power", &[10.0, 20.0, 30.0, 40.0, 50.0]),
];

/// JS `TABLE_ENCHANT_XP[key]`.
pub fn table_enchant_xp(key: &str) -> Option<&'static [f64]> {
    TABLE_ENCHANT_XP.iter().find(|(k, _)| *k == key).map(|&(_, v)| v)
}

/// One entry of JS `ENCHANT_UPGRADE_ITEMS`.
#[derive(Clone, Copy, Debug)]
pub struct EnchantUpgradeItem {
    pub enchant: &'static str,
    pub level: i32,
    pub id: &'static str,
    pub name: &'static str,
}

pub static ENCHANT_UPGRADE_ITEMS: &[EnchantUpgradeItem] = &[
    EnchantUpgradeItem { enchant: "smite", level: 7, id: "SEVERED_HAND", name: "Severed Hand" },
    EnchantUpgradeItem { enchant: "ender_slayer", level: 7, id: "ENDSTONE_IDOL", name: "End Stone Idol" },
];

/// JS `ENCHANT_UPGRADE_ITEMS[key]`.
pub fn enchant_upgrade_item(key: &str) -> Option<&'static EnchantUpgradeItem> {
    ENCHANT_UPGRADE_ITEMS.iter().find(|e| e.enchant == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_matches_table() {
        for (i, e) in ENCHANTS.iter().enumerate() {
            assert_eq!(enchant_index(e.key), Some(i), "{}", e.key);
        }
        assert_eq!(enchant("sharpness").unwrap().max_level(), 7);
        assert_eq!(enchant("thunderlord").unwrap().max_level(), 7);
        assert_eq!(enchant("ultimate_the_one").unwrap().max_level(), 5);
        assert_eq!(enchant("toxophilite").unwrap().max_level(), 1);
        assert!(enchant("thunderbolt").is_none());
    }

    #[test]
    fn sharpness_and_smite_stack() {
        assert!(!enchant("sharpness").unwrap().conflicts.contains(&"smite"));
        assert!(!enchant("smite").unwrap().conflicts.contains(&"sharpness"));
    }
}
