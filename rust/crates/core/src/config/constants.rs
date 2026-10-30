//! src/config/constants.js: every number the formulas use.

use crate::kinds::{Class, Mode, Target};
use crate::model::{Blessings, Weights};
use crate::stat::Stat;

/// One entry of JS `CLASSES`.
#[derive(Clone, Copy, Debug)]
pub struct ClassInfo {
    pub key: &'static str,
    pub label: &'static str,
    pub weights: Weights,
}

/// JS `CLASSES`, in key order (index = `Class::index()`).
pub const CLASSES: [ClassInfo; 3] = [
    ClassInfo { key: "arch", label: "Archer", weights: Weights { wither: 50.0, dragon: 50.0 } },
    ClassInfo { key: "bers", label: "Berserker", weights: Weights { wither: 20.0, dragon: 80.0 } },
    ClassInfo { key: "mage", label: "Mage", weights: Weights { wither: 80.0, dragon: 20.0 } },
];

#[inline]
pub fn class_info(cls: Class) -> &'static ClassInfo {
    &CLASSES[cls.index()]
}

pub const SOLO_CLASS_MULTIPLIER: f64 = 2.0;
pub const ARCHER_ARROW_DAMAGE_PER_LEVEL: f64 = 0.8;
pub const BERSERK_DAMAGE_BASE: f64 = 40.0;
pub const BERSERK_DAMAGE_PER_LEVEL: f64 = 0.75;
/// Berserker Lust for Blood: additive damage, 600 at Berserk 50 (12 per level; per-level parts are not doubled solo).
pub const LUST_FOR_BLOOD_PER_LEVEL: f64 = 12.0;
pub const WARRIOR_PER_LEVEL: f64 = 4.0;
pub const WARRIOR_PER_LEVEL_AFTER_50: f64 = 1.0;

pub const GLOBAL_WEIGHT: f64 = 100.0;

/// JS `TARGET_DEFENSE` (wither 9000, dragon 12500).
pub const TARGET_DEFENSE_WITHER: f64 = 9000.0;
pub const TARGET_DEFENSE_DRAGON: f64 = 12500.0;

/// JS `TARGET_DEFENSE[target]`.
#[inline]
pub const fn target_defense(t: Target) -> f64 {
    match t {
        Target::Wither => TARGET_DEFENSE_WITHER,
        Target::Dragon => TARGET_DEFENSE_DRAGON,
    }
}

/// JS `BOSS_DAMAGE_TAKEN`: boss mobs take 90% less damage; applied only to the shown boss damage.
pub const BOSS_DAMAGE_TAKEN: f64 = 0.1;

pub const BASE_DAMAGE: f64 = 5.0;

/// One dungeon blessing: JS `BLESSINGS[name]` (`{ level, stats }`).
#[derive(Clone, Copy, Debug)]
pub struct BlessingDef {
    pub name: &'static str,
    /// Default level.
    pub level: i32,
    pub stats: &'static [Stat],
}

/// JS `BLESSINGS`, in key order: power, wisdom, time.
pub const BLESSINGS: [BlessingDef; 3] = [
    BlessingDef { name: "power", level: 29, stats: &[Stat::Strength, Stat::CritDamage] },
    BlessingDef { name: "wisdom", level: 14, stats: &[Stat::Intelligence] },
    BlessingDef { name: "time", level: 5, stats: &[Stat::Strength, Stat::Intelligence] },
];

/// JS `BLESSING_LIMITS`: power 5-29, wisdom 0-14, time 0 or 5.
pub const BLESSING_POWER_MIN: i32 = 5;
pub const BLESSING_POWER_MAX: i32 = 29;
pub const BLESSING_WISDOM_MIN: i32 = 0;
pub const BLESSING_WISDOM_MAX: i32 = 14;
pub const BLESSING_TIME_VALUES: [i32; 2] = [0, 5];

/// JS `blessingsWith(levels)`: chosen levels, each falling back to its default when missing, not an integer or out
/// of its limits. Inputs are JS numbers (`None` = undefined).
pub fn blessings_with(power: Option<f64>, wisdom: Option<f64>, time: Option<f64>) -> Blessings {
    let int = |v: Option<f64>| v.filter(|x| x.is_finite() && x.fract() == 0.0);
    let pick = |v: Option<f64>, ok: &dyn Fn(f64) -> bool, default: i32| match int(v) {
        Some(x) if ok(x) => x as i32,
        _ => default,
    };
    Blessings {
        power: pick(power, &|x| x >= BLESSING_POWER_MIN as f64 && x <= BLESSING_POWER_MAX as f64, 29),
        wisdom: pick(wisdom, &|x| x >= BLESSING_WISDOM_MIN as f64 && x <= BLESSING_WISDOM_MAX as f64, 14),
        time: pick(time, &|x| BLESSING_TIME_VALUES.iter().any(|&t| t as f64 == x), 5),
    }
}

/// `blessingsWith` for an already-typed `Blessings` (each level validated the same way).
pub fn blessings_validated(b: Blessings) -> Blessings {
    blessings_with(Some(b.power as f64), Some(b.wisdom as f64), Some(b.time as f64))
}

pub const BLESSING_FLAT_PER_LEVEL: f64 = 4.0;
pub const BLESSING_PERCENT_PER_LEVEL: f64 = 0.02;
pub const BLESSING_FLOOR_BUFF: f64 = 0.2;

pub const BASE_SPEED: f64 = 100.0;
pub const SPEED_CAP: f64 = 500.0;
/// JS `SPEED_CAP_BY_CLASS` (only mage: 450).
pub const SPEED_CAP_MAGE: f64 = 450.0;

/// JS `speedCap(cls, mode)`: 500 in RCM, otherwise the class cap (Mage 450) or 500.
#[inline]
pub fn speed_cap(cls: Class, mode: Mode) -> f64 {
    if mode == Mode::Rcm {
        SPEED_CAP
    } else if cls == Class::Mage {
        SPEED_CAP_MAGE
    } else {
        SPEED_CAP
    }
}

/// In dungeons every point of speed over 100 counts for 2/3 (200 -> 167, 300 -> 233). The caps above are dungeon
/// speed, so 500 needs 700 on the stat sheet.
pub const DUNGEON_SPEED_FREE: f64 = 100.0;
pub const DUNGEON_SPEED_KEPT: f64 = 2.0 / 3.0;

/// JS `dungeonSpeed(speed)`.
#[inline]
pub fn dungeon_speed(speed: f64) -> f64 {
    if speed > DUNGEON_SPEED_FREE {
        DUNGEON_SPEED_FREE + (speed - DUNGEON_SPEED_FREE) * DUNGEON_SPEED_KEPT
    } else {
        speed
    }
}

/// JS `sheetSpeedFor(dungeon)`.
#[inline]
pub fn sheet_speed_for(dungeon: f64) -> f64 {
    if dungeon > DUNGEON_SPEED_FREE {
        DUNGEON_SPEED_FREE + (dungeon - DUNGEON_SPEED_FREE) / DUNGEON_SPEED_KEPT
    } else {
        dungeon
    }
}

pub const CLEAR_BOOTS_CLASSES: [Class; 2] = [Class::Arch, Class::Bers];
pub const EXTRA_SPEED: f64 = 0.0;

/// JS `DUNGEON_POTION`, in key order. The Jellyfish boost scales every entry.
pub static DUNGEON_POTION: &[(Stat, f64)] = &[
    (Stat::Strength, 40.0),
    (Stat::CritChance, 20.0),
    (Stat::CritDamage, 30.0),
    (Stat::BowDamage, 50.0),
    (Stat::Speed, 15.0),
];

pub const ATTACK_SPEED_CAP: f64 = 100.0;
pub const TICKS_PER_SECOND: f64 = 20.0;

// Solo Archer Doubleshot: 1.01 + 1% per level, 151% at Archer 50 (stats team, 2026-10-30).
pub const DOUBLESHOT_BASE: f64 = 1.01;
pub const DOUBLESHOT_PER_LEVEL: f64 = 0.01;

pub const WOLF_SLAYER_XP: [f64; 9] = [10.0, 30.0, 250.0, 1500.0, 5000.0, 20000.0, 100000.0, 400000.0, 1000000.0];
pub const WOLF_SLAYER_SPEED_LEVELS: [i32; 3] = [1, 3, 8];

pub const BASE_CRIT_CHANCE: f64 = 30.0;
pub const COMBAT_CRIT_CHANCE_PER_LEVEL: f64 = 0.5;
pub const COMBAT_MAX_LEVEL: i32 = 60;

/// Skill XP needed for each level (index 0 = level 1).
pub const SKILL_XP_PER_LEVEL: [f64; 60] = [
    50.0, 125.0, 200.0, 300.0, 500.0, 750.0, 1000.0, 1500.0, 2000.0, 3500.0, 5000.0, 7500.0, 10000.0, 15000.0, 20000.0,
    30000.0, 50000.0, 75000.0, 100000.0, 200000.0, 300000.0, 400000.0, 500000.0, 600000.0, 700000.0, 800000.0,
    900000.0, 1000000.0, 1100000.0, 1200000.0, 1300000.0, 1400000.0, 1500000.0, 1600000.0, 1700000.0, 1800000.0,
    1900000.0, 2000000.0, 2100000.0, 2200000.0, 2300000.0, 2400000.0, 2500000.0, 2600000.0, 2750000.0, 2900000.0,
    3100000.0, 3400000.0, 3700000.0, 4000000.0, 4300000.0, 4600000.0, 4900000.0, 5200000.0, 5500000.0, 5800000.0,
    6100000.0, 6400000.0, 6700000.0, 7000000.0,
];

pub const OVERLOAD_MULTIPLIER: f64 = 1.5;

pub const MAGE_BEAM_BASE: f64 = 0.3;
pub const MAGE_BEAM_PER_INTELLIGENCE: f64 = 0.0009;
pub const MAGE_BEAM_TICKS: f64 = 10.0;
pub const MAGE_INT_BASE: f64 = 250.0;
pub const MAGE_INT_PER_LEVEL: f64 = 5.0;

pub const WITHER_IMPACT_BASE: f64 = 10000.0;
pub const WITHER_IMPACT_SCALING: f64 = 0.3;
pub const WITHER_IMPACT_WEAPONS: [&str; 5] = ["HYPERION", "ASTRAEA", "VALKYRIE", "SCYLLA", "NECRON_BLADE"];
pub const WITHER_SCROLLS: [&str; 3] = ["IMPLOSION_SCROLL", "SHADOW_WARP_SCROLL", "WITHER_SHIELD_SCROLL"];
pub const ULTIMATE_WITHER_SCROLL: &str = "ULTIMATE_WITHER_SCROLL";
pub const DEFAULT_MAXED_COLLECTIONS: f64 = 90.0;
pub const SA_CLOAK_IDS: [&str; 2] = ["SHADOW_ASSASSIN_CLOAK", "STARRED_SHADOW_ASSASSIN_CLOAK"];
pub const SA_CLOAK_MULTIPLIER: f64 = 1.1;
pub const SA_CLOAK_COOLDOWN_S: f64 = 3.0;
pub const MAGE_COOLDOWN_REDUCTION_BASE: f64 = 25.0;
pub const MAGE_COOLDOWN_REDUCTION_PER_LEVEL: f64 = 0.5;
pub const MAGE_COOLDOWN_REDUCTION_MAX: f64 = 25.0;
pub const MAGE_ABILITY_DAMAGE_BASE: f64 = 5.0;
pub const MAGE_ABILITY_DAMAGE_PER_5_LEVELS: f64 = 1.0;
pub const ENCHANTING_ABILITY_DAMAGE_PER_LEVEL: f64 = 0.5;
pub const ENCHANTING_MAX_LEVEL: i32 = 60;
pub const LOVING_ABILITY_MULTIPLIER: f64 = 1.05;
pub const IMPLOSION_BELT_ID: &str = "IMPLOSION_BELT";
pub const IMPLOSION_BELT_MULTIPLIER: f64 = 1.25;

pub const FEROCITY_EFFECTIVENESS: f64 = 0.5;

/// JS `ARROW_DAMAGE[cls] ?? 0` (arch 5, bers 5, mage none).
#[inline]
pub const fn arrow_damage(cls: Class) -> f64 {
    match cls {
        Class::Arch | Class::Bers => 5.0,
        Class::Mage => 0.0,
    }
}

pub const BOSS_SWARM_ENEMIES: f64 = 1.0;
pub const RCM_SWARM_ENEMIES: f64 = 7.0;
pub const RCM_ENCHANTS: [&str; 4] = ["giant_killer", "prosecute", "ultimate_swarm", "smite"];
/// A Hyperion with one of these ultimates is the RCM one, with Chimera the LCM one. Neither is enchanted into the other.
pub const HYPERION_ID: &str = "HYPERION";
pub const RCM_ULTIMATES: [&str; 3] = ["ultimate_wise", "ultimate_swarm", "ultimate_combo"];
pub const THUNDERLORD_EVERY: f64 = 3.0;
pub const MAGE_FIRST_STRIKE_DPS: f64 = 1e9;

pub const FABLED_MULTIPLIER: f64 = 1.075;
pub const NECRON_BLADE_WITHER_MULTIPLIER: f64 = 1.5;

pub const DUAL_WIELD_WEAPONS: [&str; 2] = ["HYPERION", "DARK_CLAYMORE"];
pub const DUAL_WIELD_ADDITIVE: f64 = 50.0;

pub const DUNGEON_BASE: f64 = 1.0;
pub const STAR_STEP: f64 = 0.1;
pub const MAX_STARS: f64 = 5.0;
pub const MASTER_STAR_STEP: f64 = 0.05;
pub const MAX_MASTER_STARS: f64 = 5.0;
pub const SECRET_STEP: f64 = 0.01;
pub const SECRET_DIGIT_CAP: f64 = 5.0;
pub const GENERAL_MEDALLION_ID: &str = "GENERAL_MEDALLION";

/// Catacombs dungeon item stat bonus in percent, index = Catacombs level (0-50).
pub const CATA_STAT_BONUS: [f64; 51] = [
    10.0, 15.0, 20.0, 25.0, 31.0, 38.0, 46.0, 55.0, 65.0, 75.0, 85.0, 95.0, 105.0, 115.0, 125.0, 135.0, 145.0, 155.0,
    165.0, 175.0, 185.0, 195.0, 205.0, 215.0, 225.0, 235.0, 245.0, 255.0, 265.0, 275.0, 285.0, 295.0, 305.0, 315.0,
    325.0, 335.0, 345.0, 355.0, 365.0, 375.0, 385.0, 395.0, 405.0, 415.0, 425.0, 435.0, 445.0, 455.0, 465.0, 475.0,
    485.0,
];

/// XP needed for each Catacombs level (not cumulative), index 0 = level 1.
pub const CATA_XP_PER_LEVEL: [f64; 50] = [
    50.0, 75.0, 110.0, 160.0, 230.0, 330.0, 470.0, 670.0, 950.0, 1340.0, 1890.0, 2665.0, 3760.0, 5260.0, 7380.0,
    10300.0, 14400.0, 20000.0, 27600.0, 38000.0, 52500.0, 71500.0, 97000.0, 132000.0, 180000.0, 243000.0, 328000.0,
    445000.0, 600000.0, 800000.0, 1065000.0, 1410000.0, 1900000.0, 2500000.0, 3300000.0, 4300000.0, 5600000.0,
    7200000.0, 9200000.0, 12000000.0, 15000000.0, 19000000.0, 24000000.0, 30000000.0, 38000000.0, 48000000.0,
    60000000.0, 75000000.0, 93000000.0, 116250000.0,
];

/// JS `HOT_POTATO` / `FUMING_POTATO`.
#[derive(Clone, Copy, Debug)]
pub struct PotatoBook {
    pub id: &'static str,
    pub max: f64,
    pub damage: f64,
    pub strength: f64,
}
pub const HOT_POTATO: PotatoBook = PotatoBook { id: "HOT_POTATO_BOOK", max: 10.0, damage: 2.0, strength: 2.0 };
pub const FUMING_POTATO: PotatoBook = PotatoBook { id: "FUMING_POTATO_BOOK", max: 5.0, damage: 2.0, strength: 2.0 };

/// JS `ART_OF_WAR`.
pub const ART_OF_WAR_ID: &str = "THE_ART_OF_WAR";
pub const ART_OF_WAR_STRENGTH: f64 = 5.0;

pub const RECOMBOBULATOR_ID: &str = "RECOMBOBULATOR_3000";

pub const MASTER_STAR_IDS: [&str; 5] =
    ["FIRST_MASTER_STAR", "SECOND_MASTER_STAR", "THIRD_MASTER_STAR", "FOURTH_MASTER_STAR", "FIFTH_MASTER_STAR"];

/// JS `CLASS_WEAPON_CATEGORIES[cls]`.
#[inline]
pub const fn class_weapon_categories(cls: Class) -> &'static [&'static str] {
    match cls {
        Class::Arch | Class::Bers => &["BOW"],
        Class::Mage => &["SWORD", "WAND", "LONGSWORD"],
    }
}

/// JS `ALL_WEAPON_CATEGORIES` (stats.js: every class's weapon categories) membership: `isWeaponCategory`.
#[inline]
pub fn is_weapon_category(category: &str) -> bool {
    matches!(category, "BOW" | "SWORD" | "WAND" | "LONGSWORD")
}

pub const ARMOR_SLOTS: [&str; 4] = ["HELMET", "CHESTPLATE", "LEGGINGS", "BOOTS"];
pub const EQUIPMENT_SLOTS: [&str; 4] = ["NECKLACE", "CLOAK", "BELT", "GLOVES"];

/// JS `equipmentSlot(category)`: the equipment slot for a category (bracelets go in GLOVES), or `None`.
pub fn equipment_slot(category: &str) -> Option<&'static str> {
    let slot = if category == "BRACELET" { "GLOVES" } else { category };
    EQUIPMENT_SLOTS.iter().copied().find(|&s| s == slot)
}

pub const ACCESSORY_CATEGORY: &str = "ACCESSORY";

/// JS `MAX_REFORGE_ITEMS.test(id)`: `/^(DIAMOND|GOLD)_[A-Z_]+_HEAD$/` (boss heads).
pub fn is_max_reforge_item(id: &str) -> bool {
    let rest = match id.strip_prefix("DIAMOND_").or_else(|| id.strip_prefix("GOLD_")) {
        Some(r) => r,
        None => return false,
    };
    match rest.strip_suffix("_HEAD") {
        Some(mid) => !mid.is_empty() && mid.bytes().all(|b| b.is_ascii_uppercase() || b == b'_'),
        None => false,
    }
}

pub const RARITIES: [&str; 7] = ["COMMON", "UNCOMMON", "RARE", "EPIC", "LEGENDARY", "MYTHIC", "DIVINE"];

/// JS `RARITIES.indexOf(r)` (`None` for -1).
#[inline]
pub fn rarity_index(r: &str) -> Option<usize> {
    RARITIES.iter().position(|&x| x == r)
}

pub const DUNGEON_FLOOR_CATA: [f64; 10] = [1.0, 3.0, 5.0, 9.0, 14.0, 19.0, 24.0, 32.0, 34.0, 36.0];
pub const DUNGEON_DROP_BOOST: f64 = 0.5;
pub const MASTER_FLOOR_CATA: [f64; 7] = [24.0, 26.0, 28.0, 30.0, 32.0, 34.0, 36.0];

/// JS `SLAYER_XP`, in key order (cumulative XP per level).
pub const SLAYER_XP: [(&str, &[f64]); 6] = [
    ("zombie", &[5.0, 15.0, 200.0, 1000.0, 5000.0, 20000.0, 100000.0, 400000.0, 1000000.0]),
    ("spider", &[5.0, 15.0, 200.0, 1000.0, 5000.0, 20000.0, 100000.0, 400000.0, 1000000.0]),
    ("wolf", &[10.0, 30.0, 250.0, 1500.0, 5000.0, 20000.0, 100000.0, 400000.0, 1000000.0]),
    ("enderman", &[10.0, 30.0, 250.0, 1500.0, 5000.0, 20000.0, 100000.0, 400000.0, 1000000.0]),
    ("blaze", &[10.0, 30.0, 250.0, 1500.0, 5000.0, 20000.0, 100000.0, 400000.0, 1000000.0]),
    ("vampire", &[20.0, 75.0, 240.0, 840.0, 2400.0]),
];

pub const SPECIAL_RARITIES: [&str; 2] = ["SPECIAL", "VERY_SPECIAL"];

/// JS `itemRarity(item)` (stats.js): the rarity after recombobulation. `base` = `item.baseRarity ?? 'COMMON'`.
/// Boss heads (`MAX_REFORGE_ITEMS`) never change; SPECIAL becomes VERY_SPECIAL; DIVINE and unknown tiers stay.
pub fn item_rarity<'a>(base: &'a str, recomb: bool, id: &str) -> &'a str {
    if !recomb || is_max_reforge_item(id) {
        return base;
    }
    if base == SPECIAL_RARITIES[0] {
        return SPECIAL_RARITIES[1];
    }
    match rarity_index(base) {
        Some(i) if i < RARITIES.len() - 1 => RARITIES[i + 1],
        _ => base,
    }
}

/// JS `statRarity(rarity)` (stats.js): SPECIAL uses MYTHIC stats, VERY_SPECIAL uses DIVINE.
#[inline]
pub fn stat_rarity(rarity: &str) -> &str {
    match rarity {
        "SPECIAL" => "MYTHIC",
        "VERY_SPECIAL" => "DIVINE",
        r => r,
    }
}

pub const RAGNAROCK_ID: &str = "RAGNAROCK_AXE";
pub const RAGNAROCK_MULTIPLIER: f64 = 1.5;
pub const CHIMERA_PER_LEVEL: f64 = 0.2;

pub const LEGION_PER_LEVEL: f64 = 0.0007;
pub const LEGION_PLAYERS: f64 = 1.0;
pub const LEGION_GROUPED_PLAYERS: f64 = 4.0;

pub const MAX_UPGRADES_LISTED: usize = 15;

pub const MAXOR_BOOTS_ID: &str = "SPEED_WITHER_BOOTS";
pub const NECRON_BOOTS_ID: &str = "POWER_WITHER_BOOTS";

pub const SKELETON_MASTER_CHEST_CLASSES: [Class; 2] = [Class::Arch, Class::Bers];
pub const SKELETON_MASTER_CHEST_CATA: f64 = 36.0;
pub const SKELETON_MASTER_CHESTS: [&str; 2] = ["SKELETON_MASTER_CHESTPLATE", "STARRED_SKELETON_MASTER_CHESTPLATE"];

/// JS `CLASS_EXCLUDED_ITEMS[cls]?.test(id)`. Mage:
/// `/^(POWER_WITHER_(HELMET|CHESTPLATE|LEGGINGS|BOOTS)|DIAMOND_NECRON_HEAD|GOLD_NECRON_HEAD)$/`.
pub fn class_excluded_item(cls: Class, id: &str) -> bool {
    cls == Class::Mage
        && matches!(
            id,
            "POWER_WITHER_HELMET"
                | "POWER_WITHER_CHESTPLATE"
                | "POWER_WITHER_LEGGINGS"
                | "POWER_WITHER_BOOTS"
                | "DIAMOND_NECRON_HEAD"
                | "GOLD_NECRON_HEAD"
        )
}

pub const BLACKSMITH_REFORGE_COST: f64 = 500.0;
pub const ARMOR_REFORGE: &str = "ancient";

pub const AUCTION_REFRESH_MS: u64 = 5 * 60 * 1000;
pub const BAZAAR_REFRESH_MS: u64 = 5 * 60 * 1000;
pub const ITEMS_REFRESH_MS: u64 = 60 * 60 * 1000;
pub const AUCTION_SMOOTH_DOWNLOADS: usize = 3;
pub const PROFILE_CACHE_MS: u64 = 5 * 60 * 1000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blessing_limits() {
        let b = blessings_with(Some(10.0), Some(3.0), Some(0.0));
        assert_eq!((b.power, b.wisdom, b.time), (10, 3, 0));
        let bad = blessings_with(Some(4.0), Some(15.0), Some(3.0));
        assert_eq!((bad.power, bad.wisdom, bad.time), (29, 14, 5));
        let frac = blessings_with(Some(10.5), None, Some(5.0));
        assert_eq!((frac.power, frac.wisdom, frac.time), (29, 14, 5));
    }

    #[test]
    fn regex_ports() {
        assert!(is_max_reforge_item("DIAMOND_NECRON_HEAD"));
        assert!(is_max_reforge_item("GOLD_BONZO_HEAD"));
        assert!(is_max_reforge_item("GOLD_X_Y_HEAD"));
        assert!(!is_max_reforge_item("DIAMOND_HEAD"));
        assert!(!is_max_reforge_item("GOLD__HEAD"));
        assert!(is_max_reforge_item("GOLD___HEAD"));
        assert!(!is_max_reforge_item("GOLD_N1_HEAD"));
        assert!(!is_max_reforge_item("IRON_NECRON_HEAD"));
        assert!(class_excluded_item(Class::Mage, "POWER_WITHER_BOOTS"));
        assert!(!class_excluded_item(Class::Arch, "POWER_WITHER_BOOTS"));
        assert_eq!(equipment_slot("BRACELET"), Some("GLOVES"));
        assert_eq!(equipment_slot("HELMET"), None);
    }

    #[test]
    fn rarities() {
        assert_eq!(item_rarity("EPIC", true, "HYPERION"), "LEGENDARY");
        assert_eq!(item_rarity("EPIC", false, "HYPERION"), "EPIC");
        assert_eq!(item_rarity("DIVINE", true, "X"), "DIVINE");
        assert_eq!(item_rarity("SPECIAL", true, "X"), "VERY_SPECIAL");
        assert_eq!(item_rarity("VERY_SPECIAL", true, "X"), "VERY_SPECIAL");
        assert_eq!(item_rarity("LEGENDARY", true, "DIAMOND_NECRON_HEAD"), "LEGENDARY");
        assert_eq!(item_rarity("ADMIN", true, "X"), "ADMIN");
        assert_eq!(stat_rarity("SPECIAL"), "MYTHIC");
        assert_eq!(stat_rarity("RARE"), "RARE");
    }
}
