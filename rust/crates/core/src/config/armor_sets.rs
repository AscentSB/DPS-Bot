//! src/config/armorSets.js: armor set and piece bonuses that raise DPS.

use crate::kinds::Class;
use crate::stat::Stat;

/// One entry of JS `ARMOR_SETS`.
#[derive(Clone, Copy, Debug)]
pub struct ArmorSet {
    pub name: &'static str,
    /// ITEM_IDs of the set (any slot), in JS order.
    pub pieces: &'static [&'static str],
    /// Stats per piece worn (empty when the JS has none).
    pub per_piece: &'static [(Stat, f64)],
    /// Stats when all 4 pieces are worn (empty when the JS has none).
    pub full: &'static [(Stat, f64)],
    pub classes: Option<&'static [Class]>,
}

/// JS `ARMOR_SETS`, in order.
pub static ARMOR_SETS: &[ArmorSet] = &[
    ArmorSet {
        name: "Skeleton Master",
        pieces: &["SKELETON_MASTER_HELMET", "SKELETON_MASTER_CHESTPLATE", "SKELETON_MASTER_LEGGINGS", "SKELETON_MASTER_BOOTS"],
        per_piece: &[(Stat::BowDamage, 5.0)],
        full: &[(Stat::BowDamage, 25.0)],
        classes: None,
    },
    ArmorSet {
        name: "Skeleton Soldier",
        pieces: &["SKELETON_SOLDIER_HELMET", "SKELETON_SOLDIER_CHESTPLATE", "SKELETON_SOLDIER_LEGGINGS", "SKELETON_SOLDIER_BOOTS"],
        per_piece: &[(Stat::BowDamage, 5.0)],
        full: &[(Stat::BowDamage, 25.0)],
        classes: None,
    },
    ArmorSet {
        name: "Maxor's",
        pieces: &["SPEED_WITHER_HELMET", "SPEED_WITHER_CHESTPLATE", "SPEED_WITHER_LEGGINGS", "SPEED_WITHER_BOOTS"],
        per_piece: &[(Stat::BowDamage, 5.0)],
        full: &[],
        classes: None,
    },
    ArmorSet {
        name: "Superior Dragon",
        pieces: &["SUPERIOR_DRAGON_HELMET", "SUPERIOR_DRAGON_CHESTPLATE", "SUPERIOR_DRAGON_LEGGINGS", "SUPERIOR_DRAGON_BOOTS"],
        per_piece: &[],
        full: &[
            (Stat::AdditiveStrength, 0.05),
            (Stat::AdditiveCritDamage, 0.05),
            (Stat::AdditiveCritChance, 0.05),
            (Stat::AdditiveIntelligence, 0.05),
        ],
        classes: None,
    },
    ArmorSet {
        name: "Young Dragon",
        pieces: &["YOUNG_DRAGON_HELMET", "YOUNG_DRAGON_CHESTPLATE", "YOUNG_DRAGON_LEGGINGS", "YOUNG_DRAGON_BOOTS"],
        per_piece: &[],
        full: &[(Stat::Speed, 70.0)],
        classes: None,
    },
    ArmorSet {
        name: "Adaptive (Berserk)",
        pieces: &[
            "ADAPTIVE_HELMET",
            "ADAPTIVE_CHESTPLATE",
            "ADAPTIVE_LEGGINGS",
            "ADAPTIVE_BOOTS",
            "STARRED_ADAPTIVE_HELMET",
            "STARRED_ADAPTIVE_CHESTPLATE",
            "STARRED_ADAPTIVE_LEGGINGS",
            "STARRED_ADAPTIVE_BOOTS",
        ],
        per_piece: &[(Stat::Strength, 20.0)],
        full: &[],
        classes: Some(&[Class::Bers]),
    },
    ArmorSet {
        name: "Adaptive (Mage)",
        pieces: &[
            "ADAPTIVE_HELMET",
            "ADAPTIVE_CHESTPLATE",
            "ADAPTIVE_LEGGINGS",
            "ADAPTIVE_BOOTS",
            "STARRED_ADAPTIVE_HELMET",
            "STARRED_ADAPTIVE_CHESTPLATE",
            "STARRED_ADAPTIVE_LEGGINGS",
            "STARRED_ADAPTIVE_BOOTS",
        ],
        per_piece: &[(Stat::Intelligence, 50.0)],
        full: &[],
        classes: Some(&[Class::Mage]),
    },
];
