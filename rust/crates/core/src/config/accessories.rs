//! src/config/accessories.js: Accessory Power, powers, power stones, tuning and enrichments.

use crate::kinds::{Class, Mode};
use crate::stat::Stat;

/// JS `AP_BY_RARITY[rarity]` (`None` = not in the table; the JS then uses `?? 0`).
pub fn ap_by_rarity(rarity: &str) -> Option<f64> {
    Some(match rarity {
        "COMMON" => 3.0,
        "UNCOMMON" => 5.0,
        "RARE" => 8.0,
        "EPIC" => 12.0,
        "LEGENDARY" => 16.0,
        "MYTHIC" => 22.0,
        "DIVINE" => 28.0,
        "SPECIAL" => 3.0,
        "VERY_SPECIAL" => 5.0,
        _ => return None,
    })
}

pub const DUNGEON_AP_MULTIPLIER: f64 = 2.0;
pub const DOUBLE_AP_ITEMS: [&str; 1] = ["HEGEMONY_ARTIFACT"];
pub const RIFT_PRISM_ID: &str = "RIFT_PRISM";

/// JS `AP_OVERRIDES[id]` (only the Rift Prism: 11).
pub fn ap_override(id: &str) -> Option<f64> {
    if id == RIFT_PRISM_ID {
        Some(11.0)
    } else {
        None
    }
}

pub const POWER_SCALE: f64 = 29.97;
pub const POWER_LOG_FACTOR: f64 = 0.0019;
pub const POWER_EXPONENT: f64 = 1.2;

/// One entry of JS `POWERS`: stats per point of the power multiplier, and flat `bonus` stats (empty when the JS has
/// no `bonus`).
#[derive(Clone, Copy, Debug)]
pub struct Power {
    pub key: &'static str,
    pub stats: &'static [(Stat, f64)],
    pub bonus: &'static [(Stat, f64)],
}

// JS stone(): value = (base / 100) * 24 * STAT_FACTOR[k], evaluated in that order (compile-time IEEE arithmetic).
// STAT_FACTOR = { strength: 1, critDamage: 1, critChance: 0.4, intelligence: 1.5, speed: 0.5, attackSpeed: 0.3 }.
macro_rules! st {
    (strength, $v:expr) => { (Stat::Strength, $v / 100.0 * 24.0 * 1.0) };
    (critDamage, $v:expr) => { (Stat::CritDamage, $v / 100.0 * 24.0 * 1.0) };
    (critChance, $v:expr) => { (Stat::CritChance, $v / 100.0 * 24.0 * 0.4) };
    (intelligence, $v:expr) => { (Stat::Intelligence, $v / 100.0 * 24.0 * 1.5) };
    (speed, $v:expr) => { (Stat::Speed, $v / 100.0 * 24.0 * 0.5) };
    (attackSpeed, $v:expr) => { (Stat::AttackSpeed, $v / 100.0 * 24.0 * 0.3) };
}

/// JS `POWERS`, in key order.
pub static POWERS: &[Power] = &[
    // Starter and intermediate powers (values already per multiplier point).
    Power {
        key: "fortuitous",
        stats: &[(Stat::Strength, 4.8), (Stat::CritChance, 4.35), (Stat::CritDamage, 4.8)],
        bonus: &[],
    },
    Power {
        key: "pretty",
        stats: &[
            (Stat::Strength, 4.8),
            (Stat::Intelligence, 10.8),
            (Stat::CritChance, 0.475),
            (Stat::CritDamage, 1.2),
            (Stat::Speed, 0.65),
        ],
        bonus: &[],
    },
    Power {
        key: "protected",
        stats: &[(Stat::Strength, 2.4), (Stat::CritChance, 0.475), (Stat::CritDamage, 1.2)],
        bonus: &[],
    },
    Power {
        key: "simple",
        stats: &[
            (Stat::Strength, 3.6),
            (Stat::Intelligence, 5.4),
            (Stat::CritChance, 1.45),
            (Stat::CritDamage, 3.6),
            (Stat::Speed, 1.2),
        ],
        bonus: &[],
    },
    Power {
        key: "warrior",
        stats: &[(Stat::Strength, 8.4), (Stat::CritChance, 2.4), (Stat::CritDamage, 6.0)],
        bonus: &[],
    },
    Power {
        key: "commando",
        stats: &[(Stat::Strength, 8.4), (Stat::CritChance, 0.475), (Stat::CritDamage, 8.4)],
        bonus: &[],
    },
    Power {
        key: "disciplined",
        stats: &[(Stat::Strength, 7.2), (Stat::CritChance, 1.45), (Stat::CritDamage, 7.2)],
        bonus: &[],
    },
    Power {
        key: "inspired",
        stats: &[(Stat::Strength, 4.8), (Stat::Intelligence, 16.2), (Stat::CritChance, 0.95), (Stat::CritDamage, 3.6)],
        bonus: &[],
    },
    Power {
        key: "ominous",
        stats: &[
            (Stat::Strength, 3.6),
            (Stat::Intelligence, 6.1),
            (Stat::CritChance, 1.45),
            (Stat::CritDamage, 3.6),
            (Stat::Speed, 0.95),
            (Stat::AttackSpeed, 0.9),
        ],
        bonus: &[],
    },
    Power {
        key: "prepared",
        stats: &[(Stat::Strength, 1.95), (Stat::CritChance, 0.4), (Stat::CritDamage, 0.95)],
        bonus: &[],
    },
    // Stone powers.
    Power { key: "forceful", stats: &[st!(strength, 75.0), st!(critDamage, 20.0)], bonus: &[] },
    Power {
        key: "bloody",
        stats: &[st!(strength, 45.0), st!(critDamage, 45.0), st!(intelligence, 10.0)],
        bonus: &[(Stat::AttackSpeed, 10.0)],
    },
    Power {
        key: "bubba",
        stats: &[st!(strength, 20.0), st!(critChance, 10.0), st!(critDamage, 45.0), st!(attackSpeed, 25.0)],
        bonus: &[],
    },
    Power {
        key: "shaded",
        stats: &[st!(strength, 20.0), st!(critDamage, 75.0), st!(speed, 5.0)],
        bonus: &[(Stat::AttackSpeed, 3.0)],
    },
    Power {
        key: "sanguisuge",
        stats: &[st!(strength, 60.0), st!(critDamage, 20.0), st!(critChance, 10.0)],
        bonus: &[(Stat::Intelligence, 100.0)],
    },
    Power { key: "bizarre", stats: &[st!(intelligence, 120.0)], bonus: &[] },
    Power { key: "adept", stats: &[st!(intelligence, 10.0)], bonus: &[] },
    Power { key: "sighted", stats: &[st!(intelligence, 100.0)], bonus: &[] },
    Power {
        key: "crumbly",
        stats: &[st!(intelligence, 15.0), st!(critChance, 5.0), st!(attackSpeed, 10.0)],
        bonus: &[(Stat::Speed, 25.0)],
    },
    Power {
        key: "itchy",
        stats: &[st!(strength, 30.0), st!(critDamage, 35.0), st!(speed, 5.0), st!(attackSpeed, 30.0)],
        bonus: &[(Stat::Strength, 15.0), (Stat::CritDamage, 15.0)],
    },
    Power { key: "frozen", stats: &[], bonus: &[(Stat::Strength, 25.0), (Stat::CritDamage, 25.0)] },
    Power {
        key: "slender",
        stats: &[
            st!(strength, 25.0),
            st!(critDamage, 25.0),
            st!(intelligence, 25.0),
            st!(speed, 5.0),
            st!(attackSpeed, 15.0),
        ],
        bonus: &[(Stat::Strength, 50.0)],
    },
    Power {
        key: "demonic",
        stats: &[st!(strength, 23.0), st!(intelligence, 77.0)],
        bonus: &[(Stat::CritDamage, 50.0)],
    },
    Power { key: "silky", stats: &[st!(critDamage, 95.0), st!(speed, 5.0)], bonus: &[(Stat::AttackSpeed, 5.0)] },
    Power {
        key: "hurtful",
        stats: &[st!(strength, 20.0), st!(critDamage, 80.0)],
        bonus: &[(Stat::AttackSpeed, 15.0)],
    },
    Power {
        key: "strong",
        stats: &[st!(strength, 50.0), st!(critDamage, 50.0)],
        bonus: &[(Stat::Strength, 25.0), (Stat::CritDamage, 25.0)],
    },
    Power {
        key: "mythical",
        stats: &[
            st!(strength, 17.0),
            st!(critChance, 17.0),
            st!(critDamage, 17.0),
            st!(intelligence, 17.0),
            st!(speed, 8.0),
        ],
        bonus: &[(Stat::Strength, 40.0)],
    },
    Power {
        key: "scorching",
        stats: &[st!(strength, 35.0), st!(critDamage, 40.0), st!(attackSpeed, 25.0)],
        bonus: &[],
    },
    Power { key: "pleasant", stats: &[], bonus: &[] },
    Power { key: "sweet", stats: &[st!(speed, 10.0)], bonus: &[(Stat::Speed, 5.0)] },
    Power { key: "healthy", stats: &[], bonus: &[] },
    Power { key: "buttery", stats: &[st!(speed, 20.0), st!(attackSpeed, 30.0)], bonus: &[(Stat::AttackSpeed, 5.0)] },
];

/// JS `POWERS[key]`.
pub fn power(key: &str) -> Option<&'static Power> {
    POWERS.iter().find(|p| p.key == key)
}

/// JS `POWER_STONES`, in key order: power -> stone item id.
pub static POWER_STONES: &[(&str, &str)] = &[
    ("silky", "LUXURIOUS_SPOOL"),
    ("shaded", "DARK_ORB"),
    ("bloody", "BEATING_HEART"),
    ("bubba", "BUBBA_BLISTER"),
    ("sanguisuge", "DISPLACED_LEECH"),
    ("bizarre", "ECCENTRIC_PAINTING"),
    ("sighted", "ENDER_MONOCLE"),
    ("adept", "END_STONE_SHULKER"),
    ("pleasant", "PRECIOUS_PEARL"),
    ("hurtful", "MAGMA_URCHIN"),
    ("demonic", "HORNS_OF_TORMENT"),
    ("sweet", "ROCK_CANDY"),
    ("frozen", "GLACITE_SHARD"),
    ("slender", "HAZMAT_ENDERMAN"),
    ("healthy", "VITAMIN_DEATH"),
    ("buttery", "SUNFLOWER_BUTTER"),
    ("crumbly", "CHOCOLATE_CHIP"),
    ("itchy", "FURBALL"),
    ("mythical", "OBSIDIAN_TABLET"),
    ("forceful", "ACACIA_BIRDHOUSE"),
    ("scorching", "SCORCHED_BOOKS"),
    ("strong", "MANDRAA"),
];

pub const FREE_POWERS: [&str; 10] =
    ["fortuitous", "pretty", "protected", "simple", "warrior", "commando", "disciplined", "inspired", "ominous", "prepared"];

pub const ENRICHMENT_SWAPPER_ID: &str = "TALISMAN_ENRICHMENT_SWAPPER";

/// JS `DEFAULT_POWER[cls]`.
#[inline]
pub const fn default_power(cls: Class) -> &'static str {
    match cls {
        Class::Arch | Class::Bers => "silky",
        Class::Mage => "sighted",
    }
}

pub const AP_PER_TUNING_POINT: f64 = 10.0;

/// JS `TUNING_PER_POINT`, in key order.
pub static TUNING_PER_POINT: &[(Stat, f64)] = &[
    (Stat::Strength, 1.0),
    (Stat::CritDamage, 1.0),
    (Stat::CritChance, 0.2),
    (Stat::Intelligence, 2.0),
    (Stat::Speed, 1.5),
    (Stat::AttackSpeed, 0.3),
];

/// JS `TUNING_PER_POINT[stat]` (`None` = undefined).
#[inline]
pub fn tuning_per_point(stat: Stat) -> Option<f64> {
    crate::stat::get(TUNING_PER_POINT, stat)
}
pub const TUNING_PER_POINT_ATTACK_SPEED: f64 = 0.3;
pub const TUNING_PER_POINT_SPEED: f64 = 1.5;

/// JS `TUNING_KEYS`, in key order: profile tuning key -> stat.
pub static TUNING_KEYS: &[(&str, Stat)] = &[
    ("strength", Stat::Strength),
    ("critical_damage", Stat::CritDamage),
    ("crit_damage", Stat::CritDamage),
    ("critical_chance", Stat::CritChance),
    ("crit_chance", Stat::CritChance),
    ("intelligence", Stat::Intelligence),
    ("walk_speed", Stat::Speed),
    ("speed", Stat::Speed),
    ("attack_speed", Stat::AttackSpeed),
];

/// JS `TUNING_KEYS[key]`.
pub fn tuning_key(key: &str) -> Option<Stat> {
    TUNING_KEYS.iter().find(|(k, _)| *k == key).map(|&(_, s)| s)
}

/// JS `DEFAULT_TUNING_SPLIT[cls]` (the same for every class).
#[inline]
pub const fn default_tuning_split(_cls: Class) -> &'static [(Stat, f64)] {
    &[(Stat::Strength, 0.5), (Stat::CritDamage, 0.5)]
}

/// JS `HATCESSORY.test(id)`: `/^(PARTY_HAT_|BALLOON_HAT_|CAKE_HAT_)/`.
#[inline]
pub fn is_hatcessory(id: &str) -> bool {
    id.starts_with("PARTY_HAT_") || id.starts_with("BALLOON_HAT_") || id.starts_with("CAKE_HAT_")
}

pub const NO_RECOMB: [&str; 3] = ["BOOK_OF_PROGRESSION", "PANDORAS_BOX", "RIFT_PRISM"];

/// One entry of JS `ENRICHMENTS`.
#[derive(Clone, Copy, Debug)]
pub struct Enrichment {
    /// Enrichment kind as stored on items (`critical_damage`).
    pub kind: &'static str,
    /// Item id suffix: `TALISMAN_ENRICHMENT_<key>`.
    pub key: &'static str,
    pub stats: &'static [(Stat, f64)],
}

/// JS `ENRICHMENTS`, in key order.
pub static ENRICHMENTS: &[Enrichment] = &[
    Enrichment { kind: "strength", key: "STRENGTH", stats: &[(Stat::Strength, 1.0)] },
    Enrichment { kind: "critical_damage", key: "CRITICAL_DAMAGE", stats: &[(Stat::CritDamage, 1.0)] },
    Enrichment { kind: "critical_chance", key: "CRITICAL_CHANCE", stats: &[(Stat::CritChance, 1.0)] },
    Enrichment { kind: "intelligence", key: "INTELLIGENCE", stats: &[(Stat::Intelligence, 2.0)] },
    Enrichment { kind: "walk_speed", key: "WALK_SPEED", stats: &[(Stat::Speed, 1.0)] },
    Enrichment { kind: "attack_speed", key: "ATTACK_SPEED", stats: &[(Stat::AttackSpeed, 0.5)] },
];

/// JS `ENRICHMENTS[kind]`.
pub fn enrichment(kind: &str) -> Option<&'static Enrichment> {
    ENRICHMENTS.iter().find(|e| e.kind == kind)
}

pub const ENRICHABLE_RARITIES: [&str; 5] = ["LEGENDARY", "MYTHIC", "DIVINE", "SPECIAL", "VERY_SPECIAL"];

/// JS `BEST_ENRICHMENT[cls]`.
#[inline]
pub const fn best_enrichment(_cls: Class) -> &'static str {
    "critical_damage"
}

/// JS `CLASS_ENRICHMENTS[cls]`.
#[inline]
pub const fn class_enrichments(cls: Class) -> &'static [&'static str] {
    match cls {
        Class::Arch | Class::Bers => &["critical_damage", "critical_chance", "strength", "attack_speed", "walk_speed"],
        Class::Mage => &["critical_damage", "critical_chance", "strength", "intelligence", "attack_speed", "walk_speed"],
    }
}

/// JS `CLASS_TUNING_STATS[cls]`.
#[inline]
pub const fn class_tuning_stats(cls: Class) -> &'static [Stat] {
    match cls {
        Class::Arch | Class::Bers => &[Stat::Strength, Stat::CritDamage, Stat::CritChance],
        Class::Mage => &[Stat::Strength, Stat::CritDamage, Stat::CritChance, Stat::Intelligence],
    }
}

pub const RCM_TUNING_STATS: [Stat; 1] = [Stat::Intelligence];

/// JS `tuningStats(ctx)`: the Mage clear set tunes intelligence only.
#[inline]
pub fn tuning_stats(cls: Class, mode: Mode) -> &'static [Stat] {
    if cls == Class::Mage && mode == Mode::Rcm {
        &RCM_TUNING_STATS
    } else {
        class_tuning_stats(cls)
    }
}

/// JS `enrichmentKinds(ctx)`: none for the Mage clear set.
#[inline]
pub fn enrichment_kinds(cls: Class, mode: Mode) -> &'static [&'static str] {
    if cls == Class::Mage && mode == Mode::Rcm {
        &[]
    } else {
        class_enrichments(cls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stone_conversion() {
        let silky = power("silky").unwrap();
        assert_eq!(silky.stats, &[(Stat::CritDamage, 95.0 / 100.0 * 24.0), (Stat::Speed, 5.0 / 100.0 * 24.0 * 0.5)]);
        assert_eq!(silky.stats[0].1, 22.799999999999997);
        let crumbly = power("crumbly").unwrap();
        assert_eq!(crumbly.stats[1], (Stat::CritChance, 5.0 / 100.0 * 24.0 * 0.4));
        assert!(power("nope").is_none());
        for (p, _) in POWER_STONES {
            assert!(power(p).is_some(), "{p}");
        }
        assert!(is_hatcessory("BALLOON_HAT_2024"));
        assert!(!is_hatcessory("BALLOON_SNAKE"));
    }
}
