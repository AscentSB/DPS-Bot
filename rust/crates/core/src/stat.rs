//! Stat keys. The JS code keeps stats in plain objects keyed by name (`{ strength: 10, critDamage: 5 }`); here every
//! key is a `Stat`, so the calc crate can use dense `[f64; Stat::BUCKET_COUNT]` buckets.
//!
//! The first `BUCKET_COUNT` variants are the keys of `emptyBucket()` in src/calc/stats.js, in that order (JS `addInto`
//! skips any other key). The rest are keys that config objects carry but buckets never hold (`tuningPoints`, `echo`,
//! `petCritDamage`): JS `addLoose` keeps them, so they exist here too.
//!
//! Config stat objects are ordered `&[(Stat, f64)]` slices in the JS key order. A `StatList` is the same thing built at
//! runtime (an inline, allocation-free ordered map with JS object semantics).

use serde::ser::{Serialize, SerializeMap, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Stat {
    // ---- bucket keys (emptyBucket order) ----
    Damage,
    Strength,
    CritDamage,
    CritChance,
    Intelligence,
    AdditiveDamage,
    AdditiveStrength,
    AdditiveStrength2,
    MultStrength,
    AdditiveCritDamage,
    AdditiveCritDamage2,
    MultCritDamage,
    AdditiveCritChance,
    MultCritChance,
    AdditiveIntelligence,
    AdditiveIntelligence2,
    MultIntelligence,
    Blessing,
    BlessingShard,
    Speed,
    BowDamage,
    AttackSpeed,
    AdditiveAttackSpeed,
    Arrows,
    AbilityDamage,
    MagicAdditive,
    Ferocity,
    AdditiveFerocity,
    // ---- non-bucket keys ----
    /// Shards: tuning points per level (Hideonbox).
    TuningPoints,
    /// Shards: echo boost per level.
    Echo,
    /// Essence perks: fraction of the active pet's crit damage (Infused Dragon).
    PetCritDamage,
}

const NAMES: [&str; Stat::COUNT] = [
    "damage",
    "strength",
    "critDamage",
    "critChance",
    "intelligence",
    "additiveDamage",
    "additiveStrength",
    "additiveStrength2",
    "multStrength",
    "additiveCritDamage",
    "additiveCritDamage2",
    "multCritDamage",
    "additiveCritChance",
    "multCritChance",
    "additiveIntelligence",
    "additiveIntelligence2",
    "multIntelligence",
    "blessing",
    "blessingShard",
    "speed",
    "bowDamage",
    "attackSpeed",
    "additiveAttackSpeed",
    "arrows",
    "abilityDamage",
    "magicAdditive",
    "ferocity",
    "additiveFerocity",
    "tuningPoints",
    "echo",
    "petCritDamage",
];

impl Stat {
    /// Number of variants.
    pub const COUNT: usize = 31;
    /// Number of bucket keys (the first variants): `emptyBucket()` has exactly these.
    pub const BUCKET_COUNT: usize = 28;

    /// Every stat, in declaration order (bucket keys first, in `emptyBucket` order).
    pub const ALL: [Stat; Stat::COUNT] = [
        Stat::Damage,
        Stat::Strength,
        Stat::CritDamage,
        Stat::CritChance,
        Stat::Intelligence,
        Stat::AdditiveDamage,
        Stat::AdditiveStrength,
        Stat::AdditiveStrength2,
        Stat::MultStrength,
        Stat::AdditiveCritDamage,
        Stat::AdditiveCritDamage2,
        Stat::MultCritDamage,
        Stat::AdditiveCritChance,
        Stat::MultCritChance,
        Stat::AdditiveIntelligence,
        Stat::AdditiveIntelligence2,
        Stat::MultIntelligence,
        Stat::Blessing,
        Stat::BlessingShard,
        Stat::Speed,
        Stat::BowDamage,
        Stat::AttackSpeed,
        Stat::AdditiveAttackSpeed,
        Stat::Arrows,
        Stat::AbilityDamage,
        Stat::MagicAdditive,
        Stat::Ferocity,
        Stat::AdditiveFerocity,
        Stat::TuningPoints,
        Stat::Echo,
        Stat::PetCritDamage,
    ];

    /// `emptyBucket()` initial values: 1 for the `mult*` stats, 0 otherwise.
    pub const BUCKET_INIT: [f64; Stat::BUCKET_COUNT] = {
        let mut a = [0.0; Stat::BUCKET_COUNT];
        a[Stat::MultStrength as usize] = 1.0;
        a[Stat::MultCritDamage as usize] = 1.0;
        a[Stat::MultCritChance as usize] = 1.0;
        a[Stat::MultIntelligence as usize] = 1.0;
        a
    };

    /// The JS key (`critDamage`).
    #[inline]
    pub const fn js_name(self) -> &'static str {
        NAMES[self as usize]
    }

    /// Parse a JS key. `None` for keys the JS objects never hold.
    pub fn from_js(s: &str) -> Option<Stat> {
        Some(match s {
            "damage" => Stat::Damage,
            "strength" => Stat::Strength,
            "critDamage" => Stat::CritDamage,
            "critChance" => Stat::CritChance,
            "intelligence" => Stat::Intelligence,
            "additiveDamage" => Stat::AdditiveDamage,
            "additiveStrength" => Stat::AdditiveStrength,
            "additiveStrength2" => Stat::AdditiveStrength2,
            "multStrength" => Stat::MultStrength,
            "additiveCritDamage" => Stat::AdditiveCritDamage,
            "additiveCritDamage2" => Stat::AdditiveCritDamage2,
            "multCritDamage" => Stat::MultCritDamage,
            "additiveCritChance" => Stat::AdditiveCritChance,
            "multCritChance" => Stat::MultCritChance,
            "additiveIntelligence" => Stat::AdditiveIntelligence,
            "additiveIntelligence2" => Stat::AdditiveIntelligence2,
            "multIntelligence" => Stat::MultIntelligence,
            "blessing" => Stat::Blessing,
            "blessingShard" => Stat::BlessingShard,
            "speed" => Stat::Speed,
            "bowDamage" => Stat::BowDamage,
            "attackSpeed" => Stat::AttackSpeed,
            "additiveAttackSpeed" => Stat::AdditiveAttackSpeed,
            "arrows" => Stat::Arrows,
            "abilityDamage" => Stat::AbilityDamage,
            "magicAdditive" => Stat::MagicAdditive,
            "ferocity" => Stat::Ferocity,
            "additiveFerocity" => Stat::AdditiveFerocity,
            "tuningPoints" => Stat::TuningPoints,
            "echo" => Stat::Echo,
            "petCritDamage" => Stat::PetCritDamage,
            _ => return None,
        })
    }

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// A key of `emptyBucket()` (JS `key in bucket`): `addInto` adds it, other keys are skipped.
    #[inline]
    pub const fn is_bucket(self) -> bool {
        (self as usize) < Stat::BUCKET_COUNT
    }

    /// JS `MULT_STATS`: multiplied into the bucket (`bucket[key] *= value`), never scaled.
    #[inline]
    pub const fn is_mult(self) -> bool {
        matches!(self, Stat::MultStrength | Stat::MultCritDamage | Stat::MultCritChance | Stat::MultIntelligence)
    }

    /// JS `FLAT_STATS` (stats.js): flat stats that get dungeon scaling (`speed` is handled next to it in the JS).
    #[inline]
    pub const fn is_flat(self) -> bool {
        matches!(self, Stat::Damage | Stat::Strength | Stat::CritDamage | Stat::CritChance | Stat::Intelligence)
    }

    /// JS `STAR_ONLY_STATS` (stats.js): dungeon items scale these by the star part only.
    #[inline]
    pub const fn is_star_only(self) -> bool {
        matches!(self, Stat::CritChance | Stat::AttackSpeed | Stat::AbilityDamage | Stat::Ferocity)
    }
}

/// JS `STAT_ALIASES[apiKey.toUpperCase()]` (stats.js): Hypixel items-resource stat names to stat keys.
/// Case-insensitive (ASCII), so callers need not upper-case the key.
pub fn stat_alias(api_key: &str) -> Option<Stat> {
    const ALIASES: [(&str, Stat); 14] = [
        ("DAMAGE", Stat::Damage),
        ("STRENGTH", Stat::Strength),
        ("CRITICAL_DAMAGE", Stat::CritDamage),
        ("CRIT_DAMAGE", Stat::CritDamage),
        ("CRITICAL_CHANCE", Stat::CritChance),
        ("CRIT_CHANCE", Stat::CritChance),
        ("INTELLIGENCE", Stat::Intelligence),
        ("WALK_SPEED", Stat::Speed),
        ("SPEED", Stat::Speed),
        ("ATTACK_SPEED", Stat::AttackSpeed),
        ("BONUS_ATTACK_SPEED", Stat::AttackSpeed),
        ("ABILITY_DAMAGE_PERCENT", Stat::AbilityDamage),
        ("ABILITY_DAMAGE", Stat::AbilityDamage),
        ("FEROCITY", Stat::Ferocity),
    ];
    ALIASES.iter().find(|(k, _)| k.eq_ignore_ascii_case(api_key)).map(|&(_, s)| s)
}

/// Value of `stat` in an ordered stat slice (JS `obj[stat]`), `None` when absent.
#[inline]
pub fn get(stats: &[(Stat, f64)], stat: Stat) -> Option<f64> {
    stats.iter().find(|(k, _)| *k == stat).map(|&(_, v)| v)
}

/// Capacity of a `StatList`. The largest runtime stat object (a pet's stats) has 13 keys.
pub const STAT_LIST_CAP: usize = 16;

/// An ordered stat object built at runtime, with JS plain-object semantics: assigning an existing key keeps its
/// position, a new key goes to the end. Inline (no heap), `Copy`.
#[derive(Clone, Copy)]
pub struct StatList {
    len: u8,
    items: [(Stat, f64); STAT_LIST_CAP],
}

impl Default for StatList {
    fn default() -> Self {
        StatList::new()
    }
}

impl PartialEq for StatList {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl std::fmt::Debug for StatList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter().map(|(k, v)| (k.js_name(), v))).finish()
    }
}

impl StatList {
    pub const fn new() -> StatList {
        StatList { len: 0, items: [(Stat::Damage, 0.0); STAT_LIST_CAP] }
    }

    /// A copy of an ordered stat slice (JS `{ ...obj }`).
    pub fn from_slice(stats: &[(Stat, f64)]) -> StatList {
        let mut out = StatList::new();
        for &(k, v) in stats {
            out.set(k, v);
        }
        out
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn as_slice(&self) -> &[(Stat, f64)] {
        &self.items[..self.len as usize]
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (Stat, f64)> + '_ {
        self.as_slice().iter().copied()
    }

    /// JS `obj[stat]`.
    #[inline]
    pub fn get(&self, stat: Stat) -> Option<f64> {
        get(self.as_slice(), stat)
    }

    #[inline]
    pub fn contains(&self, stat: Stat) -> bool {
        self.get(stat).is_some()
    }

    /// JS `obj[stat] = value`.
    pub fn set(&mut self, stat: Stat, value: f64) {
        let n = self.len as usize;
        for item in &mut self.items[..n] {
            if item.0 == stat {
                item.1 = value;
                return;
            }
        }
        assert!(n < STAT_LIST_CAP, "StatList capacity exceeded");
        self.items[n] = (stat, value);
        self.len += 1;
    }

    /// Mutable access to an existing key.
    pub fn get_mut(&mut self, stat: Stat) -> Option<&mut f64> {
        let n = self.len as usize;
        self.items[..n].iter_mut().find(|(k, _)| *k == stat).map(|(_, v)| v)
    }

    /// JS `obj[stat] = (obj[stat] ?? 0) + value`.
    #[inline]
    pub fn add(&mut self, stat: Stat, value: f64) {
        match self.get_mut(stat) {
            Some(v) => *v += value,
            None => self.set(stat, value),
        }
    }

    /// JS `addLoose(target, stats, scale)` (stats.js): every key is kept; `mult*` keys multiply (starting from 1),
    /// the others add `value * scale` (starting from 0).
    pub fn add_loose(&mut self, stats: &[(Stat, f64)], scale: f64) {
        for &(k, v) in stats {
            if k.is_mult() {
                match self.get_mut(k) {
                    Some(t) => *t *= v,
                    None => self.set(k, 1.0 * v),
                }
            } else {
                match self.get_mut(k) {
                    Some(t) => *t += v * scale,
                    None => self.set(k, 0.0 + v * scale),
                }
            }
        }
    }
}

impl Serialize for StatList {
    /// As the JS object (`{"strength":10,...}`), in key order.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.len()))?;
        for (k, v) in self.iter() {
            m.serialize_entry(k.js_name(), &v)?;
        }
        m.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for (i, s) in Stat::ALL.iter().enumerate() {
            assert_eq!(s.index(), i);
            assert_eq!(Stat::from_js(s.js_name()), Some(*s));
        }
        assert_eq!(Stat::from_js("nope"), None);
        assert!(Stat::AdditiveFerocity.is_bucket());
        assert!(!Stat::TuningPoints.is_bucket());
        assert_eq!(Stat::BUCKET_INIT[Stat::MultCritChance.index()], 1.0);
    }

    #[test]
    fn stat_list_keeps_js_order() {
        let mut l = StatList::new();
        l.set(Stat::Strength, 1.0);
        l.add(Stat::Speed, 2.0);
        l.add(Stat::Strength, 3.0);
        l.add_loose(&[(Stat::MultStrength, 1.5), (Stat::Strength, 1.0)], 2.0);
        assert_eq!(l.as_slice(), &[(Stat::Strength, 6.0), (Stat::Speed, 2.0), (Stat::MultStrength, 1.5)]);
        assert_eq!(serde_json::to_string(&l).unwrap(), r#"{"strength":6.0,"speed":2.0,"multStrength":1.5}"#);
    }

    #[test]
    fn aliases() {
        assert_eq!(stat_alias("walk_speed"), Some(Stat::Speed));
        assert_eq!(stat_alias("BONUS_ATTACK_SPEED"), Some(Stat::AttackSpeed));
        assert_eq!(stat_alias("HEALTH"), None);
        assert_eq!(stat_alias(""), None);
    }
}
