//! src/config/itemStats.js: per-item stat overrides (`ITEM_STATS`), always-on buffs (`MISC_BUFFS`) and reforge
//! overrides (`REFORGE_OVERRIDES`, empty).

use crate::kinds::Class;
use crate::stat::Stat;

/// One entry of JS `ITEM_STATS`. `None` / empty = the JS key is absent.
#[derive(Clone, Copy, Debug)]
pub struct ItemStatDef {
    pub id: &'static str,
    /// `replaceStats: true`: ignore API stats.
    pub replace_stats: bool,
    /// `dungeon`: force dungeon scaling on or off.
    pub dungeon: Option<bool>,
    /// `stats`, in JS key order.
    pub stats: &'static [(Stat, f64)],
    /// `wither` / `dragon` (JS `override.wither ?? {}`).
    pub wither: &'static [(Stat, f64)],
    pub dragon: &'static [(Stat, f64)],
    pub classes: Option<&'static [Class]>,
    /// Shortbows: arrows per shot.
    pub arrows: Option<f64>,
    /// Shortbows: base ticks between shots.
    pub shot_ticks: Option<f64>,
    /// Multiplies every flat stat on the item (Necron heads).
    pub stat_multiplier: Option<f64>,
    /// Flat stats per Catacombs level (Necron's Blade refinements).
    pub per_cata_level: &'static [(Stat, f64)],
    /// Note shown next to the item's name in upgrade lists.
    pub note: Option<&'static str>,
    pub master_mode_only: bool,
}

const B: ItemStatDef = ItemStatDef {
    id: "",
    replace_stats: false,
    dungeon: None,
    stats: &[],
    wither: &[],
    dragon: &[],
    classes: None,
    arrows: None,
    shot_ticks: None,
    stat_multiplier: None,
    per_cata_level: &[],
    note: None,
    master_mode_only: false,
};

/// JS `ITEM_STATS`, in key order.
pub static ITEM_STATS: &[ItemStatDef] = &[
    ItemStatDef { id: "TERMINATOR", stats: &[(Stat::MultCritChance, 0.25)], arrows: Some(3.0), shot_ticks: Some(10.0), ..B },
    ItemStatDef { id: "JUJU_SHORTBOW", arrows: Some(1.0), shot_ticks: Some(10.0), ..B },
    ItemStatDef { id: "ITEM_SPIRIT_BOW", arrows: Some(1.0), shot_ticks: Some(15.0), ..B },
    ItemStatDef { id: "STARRED_ITEM_SPIRIT_BOW", arrows: Some(1.0), shot_ticks: Some(14.0), ..B },
    ItemStatDef { id: "HYPERION", per_cata_level: &[(Stat::Damage, 1.0), (Stat::Intelligence, 2.0)], ..B },
    ItemStatDef { id: "VALKYRIE", per_cata_level: &[(Stat::Damage, 1.0), (Stat::Strength, 1.0)], ..B },
    ItemStatDef { id: "SCYLLA", per_cata_level: &[(Stat::Damage, 1.0), (Stat::CritDamage, 1.0)], ..B },
    ItemStatDef { id: "ASTRAEA", per_cata_level: &[(Stat::Damage, 1.0)], ..B },
    ItemStatDef { id: "IQ_POINT", stats: &[(Stat::AdditiveIntelligence, 0.01)], note: Some("+1% intelligence"), ..B },
    ItemStatDef { id: "TWO_IQ_POINT", stats: &[(Stat::AdditiveIntelligence, 0.02)], note: Some("+2% intelligence"), ..B },
    ItemStatDef { id: "DIAMOND_NECRON_HEAD", stat_multiplier: Some(2.0), ..B },
    ItemStatDef { id: "GOLD_NECRON_HEAD", stat_multiplier: Some(2.0), ..B },
    ItemStatDef { id: "MASTER_SKULL_TIER_1", stats: &[(Stat::MultStrength, 1.01)], master_mode_only: true, ..B },
    ItemStatDef { id: "MASTER_SKULL_TIER_2", stats: &[(Stat::MultStrength, 1.02)], master_mode_only: true, ..B },
    ItemStatDef { id: "MASTER_SKULL_TIER_3", stats: &[(Stat::MultStrength, 1.03)], master_mode_only: true, ..B },
    ItemStatDef { id: "MASTER_SKULL_TIER_4", stats: &[(Stat::MultStrength, 1.04)], master_mode_only: true, ..B },
    ItemStatDef { id: "MASTER_SKULL_TIER_5", stats: &[(Stat::MultStrength, 1.06)], master_mode_only: true, ..B },
    ItemStatDef { id: "MASTER_SKULL_TIER_6", stats: &[(Stat::MultStrength, 1.08)], master_mode_only: true, ..B },
    ItemStatDef { id: "MASTER_SKULL_TIER_7", stats: &[(Stat::MultStrength, 1.1)], master_mode_only: true, ..B },
];

/// JS `ITEM_STATS[id]`.
pub fn item_stats(id: &str) -> Option<&'static ItemStatDef> {
    let i = match id {
        "TERMINATOR" => 0,
        "JUJU_SHORTBOW" => 1,
        "ITEM_SPIRIT_BOW" => 2,
        "STARRED_ITEM_SPIRIT_BOW" => 3,
        "HYPERION" => 4,
        "VALKYRIE" => 5,
        "SCYLLA" => 6,
        "ASTRAEA" => 7,
        "IQ_POINT" => 8,
        "TWO_IQ_POINT" => 9,
        "DIAMOND_NECRON_HEAD" => 10,
        "GOLD_NECRON_HEAD" => 11,
        "MASTER_SKULL_TIER_1" => 12,
        "MASTER_SKULL_TIER_2" => 13,
        "MASTER_SKULL_TIER_3" => 14,
        "MASTER_SKULL_TIER_4" => 15,
        "MASTER_SKULL_TIER_5" => 16,
        "MASTER_SKULL_TIER_6" => 17,
        "MASTER_SKULL_TIER_7" => 18,
        _ => return None,
    };
    Some(&ITEM_STATS[i])
}

/// JS `BEACON_SCORCHED`.
pub const BEACON_SCORCHED: f64 = 1.2;

/// One entry of JS `MISC_BUFFS`.
#[derive(Clone, Copy, Debug)]
pub struct MiscBuffs {
    pub stats: &'static [(Stat, f64)],
    pub wither: &'static [(Stat, f64)],
    pub dragon: &'static [(Stat, f64)],
}

const ARCH_BERS_BUFFS: MiscBuffs =
    MiscBuffs { stats: &[(Stat::CritChance, 5.0 * BEACON_SCORCHED)], wither: &[], dragon: &[] };
const MAGE_BUFFS: MiscBuffs = MiscBuffs { stats: &[(Stat::AbilityDamage, 5.0 * BEACON_SCORCHED)], wither: &[], dragon: &[] };

/// JS `MISC_BUFFS[cls]`.
#[inline]
pub const fn misc_buffs(cls: Class) -> &'static MiscBuffs {
    match cls {
        Class::Arch | Class::Bers => &ARCH_BERS_BUFFS,
        Class::Mage => &MAGE_BUFFS,
    }
}

/// JS `REFORGE_OVERRIDES` (empty). `crate::data::reforges` merges it over `reforges.json`.
pub static REFORGE_OVERRIDES: &[crate::data::ReforgeDef] = &[];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_matches_table() {
        for (i, d) in ITEM_STATS.iter().enumerate() {
            assert!(std::ptr::eq(item_stats(d.id).unwrap(), &ITEM_STATS[i]), "{}", d.id);
        }
        assert!(item_stats("NOPE").is_none());
        assert_eq!(misc_buffs(Class::Arch).stats[0].1, 6.0);
    }
}
