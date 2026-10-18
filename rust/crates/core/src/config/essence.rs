//! src/config/essence.js: essence shop perks that raise DPS.

use crate::stat::Stat;

pub const ESSENCE_MIN_CATA: f64 = 20.0;

/// One entry of JS `ESSENCE_PERKS`.
#[derive(Clone, Copy, Debug)]
pub struct EssencePerk {
    /// Perk key in the profile (`player_data.perks`).
    pub key: &'static str,
    pub name: &'static str,
    /// Essence type: bazaar id `ESSENCE_<essence>`.
    pub essence: &'static str,
    /// Pet type the perk needs active (`petCritDamage`), if any.
    pub pet: Option<&'static str>,
    /// Stats per level, in JS key order (may hold `blessing` and `petCritDamage`).
    pub per_level: &'static [(Stat, f64)],
    /// Essence per level (index 0 = level 1).
    pub costs: &'static [f64],
    pub profile_only: bool,
}

/// JS `ESSENCE_PERKS`, in key order.
pub static ESSENCE_PERKS: &[EssencePerk] = &[
    EssencePerk {
        key: "catacombs_strength",
        name: "Strength Essence",
        essence: "UNDEAD",
        pet: None,
        per_level: &[(Stat::Strength, 10.0)],
        costs: &[1000.0, 4000.0, 6000.0, 8000.0, 10000.0],
        profile_only: false,
    },
    EssencePerk {
        key: "catacombs_crit_damage",
        name: "Critical Essence",
        essence: "UNDEAD",
        pet: None,
        per_level: &[(Stat::CritDamage, 10.0)],
        costs: &[1000.0, 3000.0, 10000.0, 20000.0, 50000.0],
        profile_only: false,
    },
    EssencePerk {
        key: "catacombs_intelligence",
        name: "Intelligence Essence",
        essence: "UNDEAD",
        pet: None,
        per_level: &[(Stat::Intelligence, 15.0)],
        costs: &[1000.0, 4000.0, 6000.0, 8000.0, 10000.0],
        profile_only: false,
    },
    EssencePerk {
        key: "permanent_strength",
        name: "Forbidden Strength",
        essence: "WITHER",
        pet: None,
        per_level: &[(Stat::Strength, 1.0)],
        costs: &[100.0, 250.0, 500.0, 1000.0, 1500.0],
        profile_only: false,
    },
    EssencePerk {
        key: "permanent_intelligence",
        name: "Forbidden Intelligence",
        essence: "WITHER",
        pet: None,
        per_level: &[(Stat::Intelligence, 2.0)],
        costs: &[100.0, 250.0, 500.0, 1000.0, 1500.0],
        profile_only: false,
    },
    EssencePerk {
        key: "permanent_speed",
        name: "Forbidden Speed",
        essence: "WITHER",
        pet: None,
        per_level: &[(Stat::Speed, 1.0)],
        costs: &[100.0, 250.0],
        profile_only: false,
    },
    EssencePerk {
        key: "forbidden_blessing",
        name: "Forbidden Blessing",
        essence: "WITHER",
        pet: None,
        per_level: &[(Stat::Blessing, 0.01)],
        costs: &[200.0, 400.0, 600.0, 800.0, 1000.0, 1200.0, 1400.0, 1600.0, 1800.0, 2000.0],
        profile_only: false,
    },
    EssencePerk {
        key: "blessing_of_time",
        name: "Blessing of Time (Rift)",
        essence: "SUN_GECKO",
        pet: None,
        per_level: &[(Stat::Strength, 2.0), (Stat::Intelligence, 2.0), (Stat::Speed, 1.0)],
        costs: &[0.0, 0.0, 0.0],
        profile_only: true,
    },
    EssencePerk {
        key: "edrag_cd",
        name: "Infused Dragon",
        essence: "DRAGON",
        pet: Some("ENDER_DRAGON"),
        per_level: &[(Stat::PetCritDamage, 0.02)],
        costs: &[500.0, 1500.0, 2500.0, 3500.0, 4000.0],
        profile_only: false,
    },
];

/// JS `ESSENCE_PERKS[key]`.
pub fn essence_perk(key: &str) -> Option<&'static EssencePerk> {
    ESSENCE_PERKS.iter().find(|p| p.key == key)
}
