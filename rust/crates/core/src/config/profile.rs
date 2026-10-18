//! src/config/profile.js: stats from the player's profile (skills, slayers, SkyBlock level, Harp songs, cakes).

use crate::stat::Stat;

pub const BASE_CRIT_DAMAGE: f64 = 50.0;

/// One entry of JS `SKILL_BONUSES`: per-level rewards from each step's key level on.
#[derive(Clone, Copy, Debug)]
pub struct SkillBonus {
    pub skill: &'static str,
    pub max: f64,
    /// `steps`, ascending by level (JS integer keys iterate ascending).
    pub steps: &'static [(f64, &'static [(Stat, f64)])],
}

impl SkillBonus {
    /// JS `steps[keys.filter((k) => k <= l).at(-1)]`: the reward per level at level `l`.
    pub fn step_for(&self, l: f64) -> Option<&'static [(Stat, f64)]> {
        self.steps.iter().rev().find(|(k, _)| *k <= l).map(|&(_, s)| s)
    }
}

/// JS `SKILL_BONUSES`, in key order.
pub static SKILL_BONUSES: &[SkillBonus] = &[
    SkillBonus { skill: "FORAGING", max: 57.0, steps: &[(1.0, &[(Stat::Strength, 1.0)]), (15.0, &[(Stat::Strength, 2.0)])] },
    SkillBonus {
        skill: "ENCHANTING",
        max: 60.0,
        steps: &[(1.0, &[(Stat::Intelligence, 1.0)]), (15.0, &[(Stat::Intelligence, 2.0)])],
    },
    SkillBonus {
        skill: "ALCHEMY",
        max: 50.0,
        steps: &[(1.0, &[(Stat::Intelligence, 1.0)]), (15.0, &[(Stat::Intelligence, 2.0)])],
    },
];

/// One entry of JS `SLAYER_BONUSES`: rewards by level (`rewards[level]`), ascending.
#[derive(Clone, Copy, Debug)]
pub struct SlayerBonus {
    pub boss: &'static str,
    pub rewards: &'static [(f64, &'static [(Stat, f64)])],
}

impl SlayerBonus {
    /// JS `rewards[l]`.
    pub fn reward(&self, l: f64) -> Option<&'static [(Stat, f64)]> {
        self.rewards.iter().find(|(k, _)| *k == l).map(|&(_, s)| s)
    }
}

/// JS `SLAYER_BONUSES`, in key order.
pub static SLAYER_BONUSES: &[SlayerBonus] = &[
    SlayerBonus {
        boss: "spider",
        rewards: &[
            (1.0, &[(Stat::CritDamage, 1.0)]),
            (2.0, &[(Stat::CritDamage, 1.0)]),
            (3.0, &[(Stat::CritDamage, 1.0)]),
            (4.0, &[(Stat::CritDamage, 1.0)]),
            (5.0, &[(Stat::CritDamage, 2.0)]),
            (6.0, &[(Stat::CritDamage, 2.0)]),
            (7.0, &[(Stat::CritChance, 1.0)]),
            (8.0, &[(Stat::CritDamage, 3.0)]),
            (9.0, &[(Stat::CritDamage, 3.0)]),
        ],
    },
    SlayerBonus { boss: "wolf", rewards: &[(5.0, &[(Stat::CritDamage, 1.0)]), (7.0, &[(Stat::CritDamage, 2.0)])] },
    SlayerBonus {
        boss: "enderman",
        rewards: &[
            (2.0, &[(Stat::Intelligence, 1.0)]),
            (4.0, &[(Stat::Intelligence, 2.0)]),
            (6.0, &[(Stat::Intelligence, 3.0)]),
            (8.0, &[(Stat::Intelligence, 4.0)]),
        ],
    },
];

pub const SLAYER_MAX_LEVEL: f64 = 9.0;

pub const BOOK_OF_PROGRESSION_ID: &str = "BOOK_OF_PROGRESSION";
pub const BOOK_DAMAGE_PER_LEVEL: f64 = 0.0001;
pub const BOOK_MAX_LEVEL: f64 = 500.0;
pub const BOOK_DAMAGE_OPTION: &str = "EXTRA_INFLICTION";
pub const BOOK_DAMAGE_UNLOCK_LEVEL: f64 = 75.0;
pub const BOOK_SWAP_COINS_PER_LEVEL: f64 = 500.0;

pub const SB_LEVEL_XP: f64 = 100.0;
pub const SB_STRENGTH_EVERY: f64 = 5.0;

/// One entry of JS `HARP_SONGS`.
#[derive(Clone, Copy, Debug)]
pub struct HarpSong {
    pub key: &'static str,
    pub name: &'static str,
    pub intelligence: f64,
}

/// JS `HARP_SONGS`, in key order.
pub static HARP_SONGS: &[HarpSong] = &[
    HarpSong { key: "hymn_joy", name: "Hymn to the Joy", intelligence: 1.0 },
    HarpSong { key: "frere_jacques", name: "Frère Jacques", intelligence: 1.0 },
    HarpSong { key: "amazing_grace", name: "Amazing Grace", intelligence: 1.0 },
    HarpSong { key: "brahms", name: "Brahm's Lullaby", intelligence: 2.0 },
    HarpSong { key: "happy_birthday", name: "Happy Birthday to You", intelligence: 2.0 },
    HarpSong { key: "greensleeves", name: "Greensleeves", intelligence: 2.0 },
    HarpSong { key: "jeopardy", name: "Geothermy?", intelligence: 3.0 },
    HarpSong { key: "minuet", name: "Minuet", intelligence: 3.0 },
    HarpSong { key: "joy_world", name: "Joy to the World", intelligence: 3.0 },
    HarpSong { key: "pure_imagination", name: "Godly Imagination", intelligence: 4.0 },
    HarpSong { key: "vie_en_rose", name: "La Vie en Rose", intelligence: 4.0 },
];

/// JS `HARP_SONGS[key]`.
pub fn harp_song(key: &str) -> Option<&'static HarpSong> {
    HARP_SONGS.iter().find(|s| s.key == key)
}

/// JS `CAKE_STATS`, in key order: Century Cake `stat_id` -> stat.
pub static CAKE_STATS: &[(&str, Stat)] = &[
    ("strength", Stat::Strength),
    ("critical_damage", Stat::CritDamage),
    ("critical_chance", Stat::CritChance),
    ("intelligence", Stat::Intelligence),
    ("walk_speed", Stat::Speed),
];

/// JS `CAKE_STATS[statId]`.
pub fn cake_stat(stat_id: &str) -> Option<Stat> {
    CAKE_STATS.iter().find(|(k, _)| *k == stat_id).map(|&(_, s)| s)
}
