//! src/config/shards.js: attribute shards that raise DPS.

use crate::kinds::{Class, StatTarget};
use crate::stat::Stat;

/// JS shard `family` and `echo` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShardFamily {
    Elemental,
    Unlimited,
    Ruler,
    Box,
    /// Only as an `echo` value: Tiamat boosts every echo shard.
    Echo,
}

impl ShardFamily {
    pub const fn js_name(self) -> &'static str {
        match self {
            ShardFamily::Elemental => "elemental",
            ShardFamily::Unlimited => "unlimited",
            ShardFamily::Ruler => "ruler",
            ShardFamily::Box => "box",
            ShardFamily::Echo => "echo",
        }
    }
}

/// One entry of JS `SHARDS`.
#[derive(Clone, Copy, Debug)]
pub struct Shard {
    /// Bazaar id suffix: `SHARD_<id>`.
    pub id: &'static str,
    pub name: &'static str,
    /// Key in the profile's `attributes.stacks`.
    pub attribute: &'static str,
    pub rarity: &'static str,
    pub family: Option<ShardFamily>,
    pub echo: Option<ShardFamily>,
    /// JS `s.target` (`None` = undefined, treated as global by the JS).
    pub target: Option<StatTarget>,
    pub classes: Option<&'static [Class]>,
    /// Effect of one level, in JS key order (may hold `tuningPoints` / `echo`).
    pub per_level: &'static [(Stat, f64)],
}

const B: Shard = Shard {
    id: "",
    name: "",
    attribute: "",
    rarity: "",
    family: None,
    echo: None,
    target: None,
    classes: None,
    per_level: &[],
};

/// JS `SHARDS`, in key order.
pub static SHARDS: &[Shard] = &[
    Shard { id: "FLASH", name: "Flash", attribute: "light_elemental", rarity: "COMMON", family: Some(ShardFamily::Elemental), per_level: &[(Stat::Strength, 1.0)], ..B },
    Shard { id: "QUAKE", name: "Quake", attribute: "stone_elemental", rarity: "UNCOMMON", family: Some(ShardFamily::Elemental), per_level: &[(Stat::Strength, 1.0)], ..B },
    Shard { id: "BOLT", name: "Bolt", attribute: "lightning_elemental", rarity: "RARE", family: Some(ShardFamily::Elemental), per_level: &[(Stat::Strength, 1.0)], ..B },
    Shard { id: "AERO", name: "Aero", attribute: "wind_elemental", rarity: "EPIC", family: Some(ShardFamily::Elemental), per_level: &[(Stat::Strength, 1.0)], ..B },
    Shard { id: "TEMPEST", name: "Tempest", attribute: "storm_elemental", rarity: "LEGENDARY", family: Some(ShardFamily::Elemental), per_level: &[(Stat::Strength, 1.0)], ..B },
    Shard { id: "STARBORN", name: "Starborn", attribute: "echo_of_elemental", rarity: "LEGENDARY", echo: Some(ShardFamily::Elemental), per_level: &[(Stat::Echo, 0.02)], ..B },
    Shard { id: "JORMUNG", name: "Jormung", attribute: "unlimited_power", rarity: "LEGENDARY", family: Some(ShardFamily::Unlimited), per_level: &[(Stat::AdditiveStrength, 0.001)], ..B },
    Shard { id: "ETHERDRAKE", name: "Etherdrake", attribute: "unlimited_energy", rarity: "LEGENDARY", family: Some(ShardFamily::Unlimited), per_level: &[(Stat::AdditiveCritDamage, 0.001)], ..B },
    Shard { id: "MOLTHORN", name: "Molthorn", attribute: "almighty", rarity: "LEGENDARY", echo: Some(ShardFamily::Unlimited), per_level: &[(Stat::Echo, 0.05)], ..B },
    Shard { id: "BRUISER", name: "Zealot Bruiser", attribute: "ender", rarity: "UNCOMMON", family: Some(ShardFamily::Ruler), target: Some(StatTarget::Dragon), per_level: &[(Stat::AdditiveDamage, 3.0)], ..B },
    Shard { id: "ZOMBIE_SOLDIER", name: "Zombie Soldier", attribute: "undead", rarity: "UNCOMMON", family: Some(ShardFamily::Ruler), target: Some(StatTarget::Wither), per_level: &[(Stat::AdditiveDamage, 3.0)], ..B },
    Shard { id: "CAIMAN", name: "Caiman", attribute: "echo_of_ruler", rarity: "EPIC", echo: Some(ShardFamily::Ruler), per_level: &[(Stat::Echo, 0.02)], ..B },
    Shard { id: "HIDEONBOX", name: "Hideonbox", attribute: "tuning_box", rarity: "LEGENDARY", family: Some(ShardFamily::Box), per_level: &[(Stat::TuningPoints, 1.0)], ..B },
    Shard { id: "CUBOA", name: "Cuboa", attribute: "echo_of_boxes", rarity: "UNCOMMON", echo: Some(ShardFamily::Box), per_level: &[(Stat::Echo, 0.02)], ..B },
    Shard { id: "TIAMAT", name: "Tiamat", attribute: "echo_of_echoes", rarity: "LEGENDARY", echo: Some(ShardFamily::Echo), per_level: &[(Stat::Echo, 0.05)], ..B },
    Shard { id: "BURNINGSOUL", name: "Inferno Demonlord", attribute: "attack_speed", rarity: "EPIC", per_level: &[(Stat::AttackSpeed, 1.0)], ..B },
    Shard { id: "OBSIDIAN_DEFENDER", name: "Obsidian Defender", attribute: "speed", rarity: "UNCOMMON", per_level: &[(Stat::Speed, 1.0)], ..B },
    Shard { id: "MIMIC", name: "Mimic", attribute: "faker", rarity: "EPIC", per_level: &[(Stat::BlessingShard, 0.01)], ..B },
    Shard { id: "POWER_DRAGON", name: "Power Dragon", attribute: "elite", rarity: "LEGENDARY", per_level: &[(Stat::AdditiveDamage, 3.0)], ..B },
    Shard { id: "SKELETOR", name: "Skeletor", attribute: "deadeye", rarity: "RARE", classes: Some(&[Class::Arch]), per_level: &[(Stat::AdditiveDamage, 2.5)], ..B },
    Shard { id: "THORN", name: "Thorn", attribute: "dominance", rarity: "EPIC", per_level: &[(Stat::AdditiveDamage, 1.5)], ..B },
    Shard { id: "BARBARIAN_DUKE_X", name: "Barbarian Duke X", attribute: "warrior", rarity: "EPIC", classes: Some(&[Class::Bers]), per_level: &[(Stat::AdditiveDamage, 2.5)], ..B },
];

/// JS `SHARDS[id]`.
pub fn shard(id: &str) -> Option<&'static Shard> {
    SHARDS.iter().find(|s| s.id == id)
}

/// Index of a shard in `SHARDS` (dense per-shard arrays).
pub fn shard_index(id: &str) -> Option<usize> {
    SHARDS.iter().position(|s| s.id == id)
}

impl Shard {
    /// JS `s.perLevel.echo` (echo shards only).
    pub fn echo_per_level(&self) -> f64 {
        crate::stat::get(self.per_level, Stat::Echo).unwrap_or(0.0)
    }
}

pub const SHARD_MAX_LEVEL: f64 = 10.0;

/// JS `SHARD_HUNTING_REQ[rarity]`.
pub fn shard_hunting_req(rarity: &str) -> Option<f64> {
    Some(match rarity {
        "COMMON" => 0.0,
        "UNCOMMON" => 5.0,
        "RARE" => 10.0,
        "EPIC" => 15.0,
        "LEGENDARY" => 20.0,
        _ => return None,
    })
}

/// JS `SHARDS_CUMULATIVE[rarity]`: shards needed for each level (index 0 = level 1).
pub fn shards_cumulative(rarity: &str) -> Option<&'static [f64; 10]> {
    Some(match rarity {
        "COMMON" => &[1.0, 4.0, 9.0, 15.0, 22.0, 30.0, 40.0, 54.0, 72.0, 96.0],
        "UNCOMMON" => &[1.0, 3.0, 6.0, 10.0, 15.0, 21.0, 28.0, 36.0, 48.0, 64.0],
        "RARE" => &[1.0, 3.0, 6.0, 9.0, 13.0, 17.0, 22.0, 28.0, 36.0, 48.0],
        "EPIC" => &[1.0, 2.0, 4.0, 6.0, 9.0, 12.0, 16.0, 20.0, 25.0, 32.0],
        "LEGENDARY" => &[1.0, 2.0, 3.0, 5.0, 7.0, 9.0, 12.0, 15.0, 19.0, 24.0],
        _ => return None,
    })
}
