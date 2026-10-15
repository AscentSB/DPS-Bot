import {
  BASE_DAMAGE, BLESSINGS, BLESSING_FLAT_PER_LEVEL, BLESSING_PERCENT_PER_LEVEL, BLESSING_FLOOR_BUFF,
  BASE_CRIT_CHANCE, SKILL_XP_PER_LEVEL, OVERLOAD_MULTIPLIER, MAGE_BEAM_BASE, MAGE_BEAM_PER_INTELLIGENCE,
  SOFT_CAP, SOFT_CAP_KEEP, WITHER_IMPACT_BASE, WITHER_IMPACT_SCALING,
  DUNGEON_BASE, STAR_STEP, MAX_STARS, MASTER_STAR_STEP, SECRET_STEP, SECRET_DIGIT_CAP,
  CATA_STAT_BONUS, CATA_XP_PER_LEVEL,
} from '../config/constants.js';
import { POWER_SCALE, POWER_LOG_FACTOR, POWER_EXPONENT } from '../config/accessories.js';

// ---------- Stat calculations ----------

export function calcDamage(weaponDamage) {
  return BASE_DAMAGE + weaponDamage;
}

// Blessing effect E = (1 + floor buff)(1 + Forbidden Blessing)(1 + Mimic shard). Mayor perks are not counted.
export function blessingEffect(forbiddenBlessing = 0, mimic = 0) {
  return (1 + BLESSING_FLOOR_BUFF) * (1 + forbiddenBlessing) * (1 + mimic);
}

// Apply every blessing that boosts `stat`: flat first, then each blessing's percent multiplies.
export function applyBlessings(value, stat, effect, blessings = BLESSINGS) {
  let flat = value;
  let pct = 1;
  for (const b of Object.values(blessings)) {
    if (!b.level || !b.stats.includes(stat)) continue;
    flat += BLESSING_FLAT_PER_LEVEL * b.level * effect;
    pct *= 1 + BLESSING_PERCENT_PER_LEVEL * b.level * effect;
  }
  return { flat, pct };
}

function blessed(stat, flat, additive, additive2, multiplicative, effect) {
  const b = applyBlessings(flat, stat, effect);
  return b.flat * (1 + additive + additive2) * multiplicative * b.pct;
}

// (Strength + Power + Time flat)(1 + additive + 2nd additive)(multiplicative)(1 + Power %)(1 + Time %)
export function calcStrength(strength, additive = 0, additive2 = 0, multiplicative = 1, effect = blessingEffect()) {
  return blessed('strength', strength, additive, additive2, multiplicative, effect);
}

// (CritDamage + Power flat)(1 + additive + 2nd additive)(multiplicative)(1 + Power %)
export function calcCritDamage(critDamage, additive = 0, additive2 = 0, multiplicative = 1, effect = blessingEffect()) {
  return blessed('critDamage', critDamage, additive, additive2, multiplicative, effect);
}

// (Intelligence + Wisdom + Time flat)(1 + additive + 2nd additive)(multiplicative)(1 + Wisdom %)(1 + Time %)
export function calcIntelligence(intelligence, additive = 0, additive2 = 0, multiplicative = 1, effect = blessingEffect()) {
  return blessed('intelligence', intelligence, additive, additive2, multiplicative, effect);
}

// (CritChance + 30)(1 + additive)(multiplicative)
export function calcCritChance(critChance, additive = 0, multiplicative = 1) {
  return (critChance + BASE_CRIT_CHANCE) * (1 + additive) * multiplicative;
}

// ---------- Catacombs and dungeon item scaling ----------

// Skill level from total skill XP, capped at maxLevel.
export function skillLevelFromXp(xp, maxLevel) {
  let level = 0;
  let remaining = xp ?? 0;
  for (const need of SKILL_XP_PER_LEVEL.slice(0, maxLevel)) {
    if (remaining < need) break;
    remaining -= need;
    level++;
  }
  return level;
}

export function cataLevelFromXp(xp) {
  let level = 0;
  let remaining = xp ?? 0;
  for (const need of CATA_XP_PER_LEVEL) {
    if (remaining < need) break;
    remaining -= need;
    level++;
  }
  return level;
}

export function cataBoost(level) {
  const capped = Math.max(0, Math.min(level, CATA_STAT_BONUS.length - 1));
  return CATA_STAT_BONUS[capped] / 100;
}

// Number of digits in the secret count, capped at 5 (General's Medallion).
export function secretDigits(secrets) {
  if (!(secrets > 0)) return 0;
  return Math.min(String(Math.floor(secrets)).length, SECRET_DIGIT_CAP);
}

function starPart(stars, masterStars, secretDigit) {
  return STAR_STEP * Math.min(stars, MAX_STARS) + MASTER_STAR_STEP * masterStars + SECRET_STEP * Math.min(secretDigit, SECRET_DIGIT_CAP);
}

// basestat * (1 + cataBoost + .1 * stars + .05 * masterstars + .01 * secretdigit)
// At 5 stars: basestat * (cataBoost + 1.5 + .05 * masterstars + .01 * secretdigit)
export function dungeonMultiplier(cataLevel, stars = MAX_STARS, masterStars = 0, secretDigit = 0) {
  return DUNGEON_BASE + cataBoost(cataLevel) + starPart(stars, masterStars, secretDigit);
}

// critchance * (1 + .1 * stars + .05 * masterstars + .01 * secretdigit). At 5 stars: 1.5 + ...
export function dungeonCritMultiplier(stars = MAX_STARS, masterStars = 0, secretDigit = 0) {
  return DUNGEON_BASE + starPart(stars, masterStars, secretDigit);
}

export function dungeonStat(baseStat, masterStars, cataLevel, secretDigit = 0, stars = MAX_STARS) {
  return baseStat * dungeonMultiplier(cataLevel, stars, masterStars, secretDigit);
}

export function dungeonCritChance(baseCritChance, masterStars, secretDigit = 0, stars = MAX_STARS) {
  return baseCritChance * dungeonCritMultiplier(stars, masterStars, secretDigit);
}

// ---------- Accessory Power ----------

// 29.97 * ln(0.0019 * AP + 1) ^ 1.2
export function powerMultiplier(accessoryPower) {
  if (!(accessoryPower > 0)) return 0;
  return POWER_SCALE * Math.log(POWER_LOG_FACTOR * accessoryPower + 1) ** POWER_EXPONENT;
}

// ---------- Damage formulas ----------

export function defenseMultiplier(defense) {
  return 1 - defense / (defense + 100);
}

// All damage above 1,000,000 has a 90% reduction.
export function applySoftCap(damage) {
  if (damage <= SOFT_CAP) return damage;
  return SOFT_CAP + (damage - SOFT_CAP) * SOFT_CAP_KEEP;
}

// Expected hit for a given crit chance:
//   cc <= 100:       crit * cc/100 + noncrit * (1 - cc/100)
//   100 < cc < 200:  crit * (2 - cc/100) + 1.5 * crit * (cc/100 - 1)     (overload)
//   cc >= 200:       1.5 * crit
export function expectedHit(s, defense) {
  const nonCrit = s.damage * (1 + s.strength / 100) * (1 + s.additiveDamage / 100) * defenseMultiplier(defense);
  const crit = nonCrit * (1 + s.critDamage / 100);
  const cc = s.critChance / 100;
  if (cc <= 1) return crit * cc + nonCrit * (1 - cc);
  if (cc < 2) return crit * (2 - cc) + OVERLOAD_MULTIPLIER * crit * (cc - 1);
  return OVERLOAD_MULTIPLIER * crit;
}

// Archer. stats: { damage, critDamage, strength, additiveDamage, critChance }. classMultiplier: the Archer class
// arrow damage buff (multiplicative), applied before the soft cap.
export function archerDamage(s, defense, classMultiplier = 1) {
  return applySoftCap(expectedHit(s, defense) * classMultiplier);
}

// Berserker. Same formula as archer, with extra additive damage (class bonuses are added by the caller).
export function berserkerDamage(s, defense, baseAdditiveDamage = 0, multiplier = 1) {
  return applySoftCap(expectedHit({ ...s, additiveDamage: s.additiveDamage + baseAdditiveDamage }, defense) * multiplier);
}

// Mage, scored as LCM: one mage beam deals (0.3 + 0.0009 x intelligence) of the melee hit (crits and overload
// included). Ability damage does not apply.
// multiplier: multiplicative melee buffs (Fabled 1.075x, Necron's Blade 1.5x vs withers).
export function mageDamage(s, defense, multiplier = 1) {
  return applySoftCap(expectedHit(s, defense) * multiplier * (MAGE_BEAM_BASE + MAGE_BEAM_PER_INTELLIGENCE * (s.intelligence ?? 0)));
}

// Mage RCM: one Wither Impact cast. Base 10,000 x (1 + Catacombs stat boost), intelligence at 0.3 scaling,
// ability damage, magic additive damage (pet perks, Combat Warrior; weapon enchants do not count) and
// multiplicative buffs (Loving chestplate 1.05x).
export function rcmDamage(s, defense, cataStatBoost, magicAdditive = 0, multiplier = 1) {
  const hit = WITHER_IMPACT_BASE * (1 + cataStatBoost)
    * (1 + ((s.intelligence ?? 0) / 100) * WITHER_IMPACT_SCALING)
    * (1 + (s.abilityDamage ?? 0) / 100)
    * (1 + magicAdditive / 100) * multiplier;
  return applySoftCap(hit * (1 - defense / (defense + 100)));
}
