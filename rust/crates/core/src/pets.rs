//! src/lib/pets.js: pet tiers, levels, data lookups and the pure pet stat computation (`computePetStats`). Caching
//! (`petStats`) is the caller's job: the result depends only on (type, tier, held item, level, bank, gold collection).

use crate::config::pets::{pet_item, pet_perk, PerkProfile, PetSpecial, TIER_BOOST_ID};
use crate::data::{pet_data_set, PetTierData, PetTypeData};
use crate::jsmath;
use crate::model::{Pet, PetProfile};
use crate::stat::{Stat, StatList};
use serde::ser::{Serialize, SerializeMap, Serializer};

const TIER_ORDER: [&str; 6] = ["COMMON", "UNCOMMON", "RARE", "EPIC", "LEGENDARY", "MYTHIC"];

/// JS `effectiveTier(pet)`: one rarity higher with a Tier Boost (never up to Mythic).
pub fn effective_tier<'a>(tier: &'a str, held_item: Option<&str>) -> &'a str {
    if held_item != Some(TIER_BOOST_ID) {
        return tier;
    }
    // TIER_ORDER[indexOf + 1]: an unknown tier (-1) gives TIER_ORDER[0].
    let next = match TIER_ORDER.iter().position(|&t| t == tier) {
        Some(i) => TIER_ORDER.get(i + 1).copied(),
        None => Some(TIER_ORDER[0]),
    };
    match next {
        Some(n) if n != "MYTHIC" => n,
        _ => tier,
    }
}

/// `effective_tier` for a `Pet`.
#[inline]
pub fn effective_tier_of(pet: &Pet) -> &str {
    effective_tier(&pet.tier, pet.held_item.as_deref())
}

/// JS `maxPetLevel(type)`: 100, or the custom max (Golden Dragon 200).
#[inline]
pub fn max_pet_level(kind: &str) -> f64 {
    pet_data_set().custom(kind).map(|c| c.max_level).unwrap_or(100.0)
}

/// JS `petLevel(type, tier, exp)`: pet level from total pet XP.
pub fn pet_level(kind: &str, tier: &str, exp: f64) -> f64 {
    let data = pet_data_set();
    let offset = data.rarity_offset(tier).unwrap_or(0.0);
    let len = data.levels.len();
    // PET_DATA.levels.slice(offset, offset + 99)
    let start = slice_index(offset, len);
    let end = slice_index(offset + 99.0, len).max(start);
    let custom = data.custom(kind);
    let extra: &[f64] = match custom {
        Some(c) if c.kind == 1.0 => c.levels,
        _ => &[],
    };
    let max = max_pet_level(kind);
    let mut level = 1.0;
    let mut remaining = exp;
    for &need in data.levels[start..end].iter().chain(extra) {
        if level >= max || remaining < need {
            break;
        }
        remaining -= need;
        level += 1.0;
    }
    level
}

/// JS `Array.prototype.slice` index conversion.
fn slice_index(v: f64, len: usize) -> usize {
    if v.is_nan() {
        return 0;
    }
    let v = v.trunc();
    if v < 0.0 {
        (len as f64 + v).max(0.0) as usize
    } else {
        v.min(len as f64) as usize
    }
}

/// JS `petData(type, tier)`.
#[inline]
pub fn pet_data(kind: &str, tier: &str) -> Option<&'static PetTierData> {
    pet_data_set().pet_type(kind).and_then(|t| t.tier(tier))
}

/// JS `petTiers(type)`: tiers with data, in key order.
pub fn pet_tiers(kind: &str) -> impl Iterator<Item = &'static str> {
    pet_data_set().pet_type(kind).map(|t| t.tiers).unwrap_or(&[]).iter().map(|t| t.tier)
}

/// JS `allPetTypes()`: every pet type with data, in key order.
pub fn all_pet_types() -> impl Iterator<Item = &'static str> {
    pet_data_set().types.iter().map(|t| t.kind)
}

/// Every pet type's data, in `allPetTypes()` order.
#[inline]
pub fn pet_types() -> &'static [PetTypeData] {
    &pet_data_set().types
}

/// What one pet gives (JS `petStats` result): `{ stats, dragon, wither, special, petStrength, potionBoost }`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PetStats {
    /// The pet's own stats plus its perk's stat adds, in JS key order.
    pub stats: StatList,
    pub dragon: StatList,
    pub wither: StatList,
    pub special: PetSpecial,
    /// Strength that counts as the pet's (Ragnarock, Chimera): `base.strength ?? 0`.
    pub pet_strength: f64,
    /// Jellyfish: Dungeon potion boost.
    pub potion_boost: f64,
}

/// JS truthiness of a number.
#[inline]
fn truthy(v: f64) -> bool {
    v != 0.0 && !v.is_nan()
}

/// JS `progress(data, type, level)`: 0..1 progress between the level-1 and max stats.
fn progress(data: &PetTierData, level: f64) -> f64 {
    if let (Some(c), Some((from, to))) = (data.curve, data.curve_range) {
        if !c.is_empty() {
            return jsmath::clamp01((level - from) / (to - from));
        }
    }
    jsmath::clamp01((level - 1.0) / 99.0)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// JS `computePetStats(pet, profile)` for a pet's fields. `level` = the pet's level (`None` = max level, which is
/// always the case for profile pets: readMember keeps no level).
pub fn compute_pet_stats(kind: &str, tier: &str, held_item: Option<&str>, level: Option<f64>, profile: &PetProfile) -> PetStats {
    let Some(data) = pet_data(kind, effective_tier(tier, held_item)) else {
        return PetStats::default();
    };
    let t = progress(data, level.unwrap_or_else(|| max_pet_level(kind)));
    let mut base = StatList::new();
    let get = |s: &[(Stat, f64)], k: Stat| crate::stat::get(s, k).unwrap_or(0.0);
    for &(k, _) in data.min {
        base.set(k, lerp(get(data.min, k), get(data.max, k), t));
    }
    for &(k, _) in data.max {
        if !base.contains(k) {
            base.set(k, lerp(get(data.min, k), get(data.max, k), t));
        }
    }
    const MAX_N: usize = 32;
    assert!(data.other_max.len() <= MAX_N, "pet perk numbers");
    let mut n = [0.0f64; MAX_N];
    for (i, &v) in data.other_max.iter().enumerate() {
        n[i] = lerp(data.other_min.get(i).copied().unwrap_or(0.0), v, t);
    }
    let perk_profile = PerkProfile { bank: profile.bank.unwrap_or(0.0), gold_collection: profile.gold_collection.unwrap_or(1e9) };
    let perk = pet_perk(kind, &n[..data.other_max.len()], perk_profile).unwrap_or_default();

    // Perk strength that counts as the pet's own stat (boosted by pet items, copied by Chimera).
    if let Some(ps) = perk.pet_strength {
        if truthy(ps) {
            base.set(Stat::Strength, base.get(Stat::Strength).unwrap_or(0.0) + ps);
        }
    }
    // Tyrant (T-Rex) makes pet item stats stronger.
    let item_boost = 1.0 + perk.special.item_boost.unwrap_or(0.0);
    if let Some(item) = held_item.and_then(pet_item) {
        for &(k, m) in item.mult {
            if let Some(v) = base.get_mut(k) {
                if truthy(*v) {
                    *v *= 1.0 + (m - 1.0) * item_boost;
                }
            }
        }
        for &(k, v) in item.flat {
            base.set(k, base.get(k).unwrap_or(0.0) + v * item_boost);
        }
    }
    // stats: { ...base, ...addTo(base, extra) }
    let mut stats = base;
    for (k, v) in perk.extra.iter() {
        stats.set(k, base.get(k).unwrap_or(0.0) + v);
    }
    PetStats {
        stats,
        dragon: perk.dragon,
        wither: perk.wither,
        special: perk.special,
        pet_strength: base.get(Stat::Strength).unwrap_or(0.0),
        potion_boost: perk.potion_boost.unwrap_or(0.0),
    }
}

/// JS `petStats(pet, profile)` without the cache.
#[inline]
pub fn pet_stats(pet: &Pet, profile: &PetProfile) -> PetStats {
    compute_pet_stats(&pet.kind, &pet.tier, pet.held_item.as_deref(), None, profile)
}

impl Serialize for PetSpecial {
    /// As the JS `special` object (only the keys the perk sets).
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        if let Some(v) = self.deaths_touch {
            m.serialize_entry("deathsTouch", &v)?;
        }
        if let Some(v) = self.combo_strength {
            m.serialize_entry("comboStrength", &v)?;
        }
        if let Some(v) = self.potato_multiplier {
            m.serialize_entry("potatoMultiplier", &v)?;
        }
        if let Some(v) = self.item_boost {
            m.serialize_entry("itemBoost", &v)?;
        }
        if let Some(w) = self.weapon_stats {
            let ws: StatList = StatList::from_slice(&w.as_stats());
            m.serialize_entry("weaponStats", &ws)?;
        }
        if let Some(v) = self.first_strike_boost {
            m.serialize_entry("firstStrikeBoost", &v)?;
        }
        if let Some(v) = self.perk_strength {
            m.serialize_entry("perkStrength", &v)?;
        }
        if let Some(v) = self.perk_speed {
            m.serialize_entry("perkSpeed", &v)?;
        }
        if let Some(v) = self.speed_cap_bonus {
            m.serialize_entry("speedCapBonus", &v)?;
        }
        m.end()
    }
}

impl Serialize for PetStats {
    /// As the JS `petStats` result.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(6))?;
        m.serialize_entry("stats", &self.stats)?;
        m.serialize_entry("dragon", &self.dragon)?;
        m.serialize_entry("wither", &self.wither)?;
        m.serialize_entry("special", &self.special)?;
        m.serialize_entry("petStrength", &self.pet_strength)?;
        m.serialize_entry("potionBoost", &self.potion_boost)?;
        m.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers() {
        assert_eq!(effective_tier("EPIC", Some(TIER_BOOST_ID)), "LEGENDARY");
        assert_eq!(effective_tier("LEGENDARY", Some(TIER_BOOST_ID)), "LEGENDARY");
        assert_eq!(effective_tier("EPIC", None), "EPIC");
        assert_eq!(effective_tier("WEIRD", Some(TIER_BOOST_ID)), "COMMON");
        assert_eq!(max_pet_level("GOLDEN_DRAGON"), 200.0);
        assert_eq!(max_pet_level("TIGER"), 100.0);
        assert_eq!(pet_level("TIGER", "LEGENDARY", 0.0), 1.0);
        assert_eq!(pet_level("TIGER", "LEGENDARY", 1e12), 100.0);
        assert_eq!(pet_level("GOLDEN_DRAGON", "LEGENDARY", 1e12), 200.0);
    }
}
