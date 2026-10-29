//! src/calc/formulas.js: stat formulas, Catacombs scaling, Accessory Power and the damage formulas. Arithmetic is in
//! the JS order (do not re-associate), so results are bit-identical.

// `!(x > 0.0)` is the JS test (true for NaN too); keep it as written.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::config::accessories::{POWER_EXPONENT, POWER_LOG_FACTOR, POWER_SCALE};
use crate::config::constants::*;
use crate::jsmath;
use crate::model::Blessings;
use crate::stat::Stat;

/// JS `BLESSINGS` default levels (power 29, wisdom 14, time 5).
pub const DEFAULT_BLESSINGS: Blessings = Blessings { power: 29, wisdom: 14, time: 5 };

// ---------- Stat calculations ----------

/// JS `calcDamage(weaponDamage)`.
#[inline]
pub fn calc_damage(weapon_damage: f64) -> f64 {
    BASE_DAMAGE + weapon_damage
}

/// JS `blessingEffect(forbiddenBlessing, mimic)` (defaults 0, 0).
#[inline]
pub fn blessing_effect(forbidden_blessing: f64, mimic: f64) -> f64 {
    (1.0 + BLESSING_FLOOR_BUFF) * (1.0 + forbidden_blessing) * (1.0 + mimic)
}

/// `{ flat, pct }` from `applyBlessings`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blessed {
    pub flat: f64,
    pub pct: f64,
}

/// JS `applyBlessings(value, stat, effect, blessings)` with an explicit blessing list (JS `Object.values` order).
pub fn apply_blessings_with(value: f64, stat: Stat, effect: f64, blessings: &[BlessingDef]) -> Blessed {
    let mut flat = value;
    let mut pct = 1.0;
    for b in blessings {
        if b.level == 0 || !b.stats.contains(&stat) {
            continue;
        }
        let level = b.level as f64;
        flat += BLESSING_FLAT_PER_LEVEL * level * effect;
        pct *= 1.0 + BLESSING_PERCENT_PER_LEVEL * level * effect;
    }
    Blessed { flat, pct }
}

/// The blessing list for chosen levels (JS `blessingsWith(...)` output: power, wisdom, time).
#[inline]
pub fn blessing_defs(b: &Blessings) -> [BlessingDef; 3] {
    [
        BlessingDef { level: b.power, ..BLESSINGS[0] },
        BlessingDef { level: b.wisdom, ..BLESSINGS[1] },
        BlessingDef { level: b.time, ..BLESSINGS[2] },
    ]
}

/// JS `applyBlessings(value, stat, effect, ctx.blessings)`.
#[inline]
pub fn apply_blessings(value: f64, stat: Stat, effect: f64, b: &Blessings) -> Blessed {
    apply_blessings_with(value, stat, effect, &blessing_defs(b))
}

#[inline]
fn blessed(stat: Stat, flat: f64, additive: f64, additive2: f64, multiplicative: f64, effect: f64, b: &Blessings) -> f64 {
    let r = apply_blessings(flat, stat, effect, b);
    r.flat * (1.0 + additive + additive2) * multiplicative * r.pct
}

/// JS `calcStrength(strength, additive, additive2, multiplicative, effect, blessings)`.
#[inline]
pub fn calc_strength(strength: f64, additive: f64, additive2: f64, multiplicative: f64, effect: f64, b: &Blessings) -> f64 {
    blessed(Stat::Strength, strength, additive, additive2, multiplicative, effect, b)
}

/// JS `calcCritDamage(...)`.
#[inline]
pub fn calc_crit_damage(crit_damage: f64, additive: f64, additive2: f64, multiplicative: f64, effect: f64, b: &Blessings) -> f64 {
    blessed(Stat::CritDamage, crit_damage, additive, additive2, multiplicative, effect, b)
}

/// JS `calcIntelligence(...)`.
#[inline]
pub fn calc_intelligence(intelligence: f64, additive: f64, additive2: f64, multiplicative: f64, effect: f64, b: &Blessings) -> f64 {
    blessed(Stat::Intelligence, intelligence, additive, additive2, multiplicative, effect, b)
}

/// JS `calcCritChance(critChance, additive, multiplicative)`.
#[inline]
pub fn calc_crit_chance(crit_chance: f64, additive: f64, multiplicative: f64) -> f64 {
    (crit_chance + BASE_CRIT_CHANCE) * (1.0 + additive) * multiplicative
}

// ---------- Catacombs and dungeon item scaling ----------

/// JS `skillLevelFromXp(xp, maxLevel)` (`SKILL_XP_PER_LEVEL.slice(0, maxLevel)`, negative counts from the end).
pub fn skill_level_from_xp(xp: f64, max_level: i64) -> i32 {
    let len = SKILL_XP_PER_LEVEL.len() as i64;
    let end = if max_level < 0 { (len + max_level).max(0) } else { max_level.min(len) } as usize;
    let mut level = 0;
    let mut remaining = xp;
    for &need in &SKILL_XP_PER_LEVEL[..end] {
        if remaining < need {
            break;
        }
        remaining -= need;
        level += 1;
    }
    level
}

/// JS `cataLevelFromXp(xp)`.
pub fn cata_level_from_xp(xp: f64) -> i32 {
    let mut level = 0;
    let mut remaining = xp;
    for &need in &CATA_XP_PER_LEVEL {
        if remaining < need {
            break;
        }
        remaining -= need;
        level += 1;
    }
    level
}

/// JS `cataBoost(level)`: `CATA_STAT_BONUS[clamp(level)] / 100` (NaN for a non-integer or NaN level, like the JS).
#[inline]
pub fn cata_boost(level: f64) -> f64 {
    let capped = jsmath::max(0.0, jsmath::min(level, (CATA_STAT_BONUS.len() - 1) as f64));
    if capped.fract() != 0.0 || capped.is_nan() {
        return f64::NAN;
    }
    CATA_STAT_BONUS[capped as usize] / 100.0
}

/// JS `secretDigits(secrets)`: digits of the secret count, capped at 5 (General's Medallion).
#[inline]
pub fn secret_digits(secrets: f64) -> f64 {
    if !(secrets > 0.0) {
        return 0.0;
    }
    jsmath::min(crate::jsfmt::floor_digits(secrets) as f64, SECRET_DIGIT_CAP)
}

#[inline]
fn star_part(stars: f64, master_stars: f64, secret_digit: f64) -> f64 {
    STAR_STEP * jsmath::min(stars, MAX_STARS) + MASTER_STAR_STEP * master_stars + SECRET_STEP * jsmath::min(secret_digit, SECRET_DIGIT_CAP)
}

/// JS `dungeonMultiplier(cataLevel, stars = 5, masterStars = 0, secretDigit = 0)`.
#[inline]
pub fn dungeon_multiplier(cata_level: f64, stars: f64, master_stars: f64, secret_digit: f64) -> f64 {
    DUNGEON_BASE + cata_boost(cata_level) + star_part(stars, master_stars, secret_digit)
}

/// JS `dungeonCritMultiplier(stars = 5, masterStars = 0, secretDigit = 0)`.
#[inline]
pub fn dungeon_crit_multiplier(stars: f64, master_stars: f64, secret_digit: f64) -> f64 {
    DUNGEON_BASE + star_part(stars, master_stars, secret_digit)
}

/// JS `dungeonStat(baseStat, masterStars, cataLevel, secretDigit = 0, stars = 5)`.
#[inline]
pub fn dungeon_stat(base_stat: f64, master_stars: f64, cata_level: f64, secret_digit: f64, stars: f64) -> f64 {
    base_stat * dungeon_multiplier(cata_level, stars, master_stars, secret_digit)
}

/// JS `dungeonCritChance(baseCritChance, masterStars, secretDigit = 0, stars = 5)`.
#[inline]
pub fn dungeon_crit_chance(base_crit_chance: f64, master_stars: f64, secret_digit: f64, stars: f64) -> f64 {
    base_crit_chance * dungeon_crit_multiplier(stars, master_stars, secret_digit)
}

// ---------- Accessory Power ----------

/// JS `powerMultiplier(ap)`: `29.97 * ln(0.0019 * AP + 1) ^ 1.2` (V8's log and pow, bit-exact).
#[inline]
pub fn power_multiplier(accessory_power: f64) -> f64 {
    if !(accessory_power > 0.0) {
        return 0.0;
    }
    POWER_SCALE * jsmath::pow(jsmath::log(POWER_LOG_FACTOR * accessory_power + 1.0), POWER_EXPONENT)
}

// ---------- Damage formulas ----------

/// JS `defenseMultiplier(defense)`.
#[inline]
pub fn defense_multiplier(defense: f64) -> f64 {
    1.0 - defense / (defense + 100.0)
}

// Boss mobs (Withers, dragons) always take 90% less damage, at any hit size. Not applied: every target here is a boss,
// so it scales every score alike and never changes the ranking; the shown damage is before it.

/// Final stats as `finalStats()` returns them (the fields the damage formulas read; `intelligence` and
/// `ability_damage` count as 0 when the JS object lacks them).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FinalStats {
    pub damage: f64,
    pub strength: f64,
    pub crit_damage: f64,
    pub intelligence: f64,
    pub crit_chance: f64,
    pub additive_damage: f64,
    pub ability_damage: f64,
}

/// JS `expectedHit(s, defense, overload = true)`.
#[inline]
pub fn expected_hit(s: &FinalStats, defense: f64, overload: bool) -> f64 {
    let non_crit = s.damage * (1.0 + s.strength / 100.0) * (1.0 + s.additive_damage / 100.0) * defense_multiplier(defense);
    let crit = non_crit * (1.0 + s.crit_damage / 100.0);
    let cc = if overload { s.crit_chance / 100.0 } else { jsmath::min(s.crit_chance / 100.0, 1.0) };
    if cc <= 1.0 {
        return crit * cc + non_crit * (1.0 - cc);
    }
    if cc < 2.0 {
        return crit * (2.0 - cc) + OVERLOAD_MULTIPLIER * crit * (cc - 1.0);
    }
    OVERLOAD_MULTIPLIER * crit
}

/// JS `archerDamage(s, defense, classMultiplier = 1, overload = false)`.
#[inline]
pub fn archer_damage(s: &FinalStats, defense: f64, class_multiplier: f64, overload: bool) -> f64 {
    expected_hit(s, defense, overload) * class_multiplier
}

/// JS `berserkerDamage(s, defense, baseAdditiveDamage = 0, multiplier = 1, overload = false)`.
#[inline]
pub fn berserker_damage(s: &FinalStats, defense: f64, base_additive_damage: f64, multiplier: f64, overload: bool) -> f64 {
    let s2 = FinalStats { additive_damage: s.additive_damage + base_additive_damage, ..*s };
    expected_hit(&s2, defense, overload) * multiplier
}

/// JS `mageDamage(s, defense, multiplier = 1)`: one mage beam (no overload).
#[inline]
pub fn mage_damage(s: &FinalStats, defense: f64, multiplier: f64) -> f64 {
    expected_hit(s, defense, false) * multiplier * (MAGE_BEAM_BASE + MAGE_BEAM_PER_INTELLIGENCE * s.intelligence)
}

/// JS `rcmDamage(s, defense, cataStatBoost, magicAdditive = 0, multiplier = 1)`: one Wither Impact cast.
#[inline]
pub fn rcm_damage(s: &FinalStats, defense: f64, cata_stat_boost: f64, magic_additive: f64, multiplier: f64) -> f64 {
    let hit = WITHER_IMPACT_BASE
        * (1.0 + cata_stat_boost)
        * (1.0 + (s.intelligence / 100.0) * WITHER_IMPACT_SCALING)
        * (1.0 + s.ability_damage / 100.0)
        * (1.0 + magic_additive / 100.0)
        * multiplier;
    hit * (1.0 - defense / (defense + 100.0))
}
