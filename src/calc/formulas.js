import {
  BASE_DAMAGE, POWER_BLESSING, WISDOM_BLESSING, BLESSING_FLAT_PER_LEVEL, BLESSING_PERCENT_PER_LEVEL, BLESSING_MULT,
  BASE_CRIT_CHANCE, OVERLOAD_MULTIPLIER, MAGE_INT_DIVISOR, MAGE_ABILITY_SCALING, MAGE_ABILITY_BASE,
  HYPERION_WITHER_MULTIPLIER, SOFT_CAP, SOFT_CAP_KEEP,
  DUNGEON_BASE, STAR_STEP, MAX_STARS, MASTER_STAR_STEP, SECRET_STEP, SECRET_DIGIT_CAP,
  CATA_STAT_BONUS, CATA_XP_PER_LEVEL,
} from '../config/constants.js';
import { POWER_SCALE, POWER_LOG_FACTOR, POWER_EXPONENT } from '../config/accessories.js';

// ---------- Stat calculations ----------

export function calcDamage(weaponDamage) {
  return BASE_DAMAGE + weaponDamage;
}

function blessed(flat, blessing, additive, additive2, multiplicative) {
  return (flat + BLESSING_FLAT_PER_LEVEL * blessing)
    * (1 + additive + additive2)
    * multiplicative
    * (1 + BLESSING_MULT * BLESSING_PERCENT_PER_LEVEL * blessing);
}

// (Strength + 76)(1 + additive + 2nd additive)(multiplicative)(1 + 1.1 * 1.1 * 1.2 * .38)
export function calcStrength(strength, additive = 0, additive2 = 0, multiplicative = 1) {
  return blessed(strength, POWER_BLESSING, additive, additive2, multiplicative);
}

// (CritDamage + 76)(1 + additive + 2nd additive)(multiplicative)(1 + 1.1 * 1.1 * 1.2 * .38)
export function calcCritDamage(critDamage, additive = 0, additive2 = 0, multiplicative = 1) {
  return blessed(critDamage, POWER_BLESSING, additive, additive2, multiplicative);
}

// (Intelligence + 48)(1 + additive + 2nd additive)(multiplicative)(1 + 1.1 * 1.1 * 1.2 * .24)
export function calcIntelligence(intelligence, additive = 0, additive2 = 0, multiplicative = 1) {
  return blessed(intelligence, WISDOM_BLESSING, additive, additive2, multiplicative);
}

// (CritChance + 30)(1 + additive)(multiplicative)
export function calcCritChance(critChance, additive = 0, multiplicative = 1) {
  return (critChance + BASE_CRIT_CHANCE) * (1 + additive) * multiplicative;
}

// ---------- Catacombs and dungeon item scaling ----------

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

// Archer. stats: { damage, critDamage, strength, additiveDamage, critChance }
export function archerDamage(s, defense) {
  return applySoftCap(expectedHit(s, defense));
}

// Berserker. Same formula as archer. The 1200 base additive damage is added here.
export function berserkerDamage(s, defense, baseAdditiveDamage = 1200) {
  return applySoftCap(expectedHit({ ...s, additiveDamage: s.additiveDamage + baseAdditiveDamage }, defense));
}

// Mage. hyperion = true applies the 1.5x Hyperion multiplier (use only for withers).
export function mageDamage(s, defense, hyperion = false) {
  const melee = s.damage * (1 + s.critDamage / 100) * (1 + s.strength / 100) * (1 + s.additiveDamage / 100);
  const ability = MAGE_ABILITY_SCALING * (MAGE_ABILITY_BASE + s.intelligence / MAGE_INT_DIVISOR);
  const raw = (melee + ability) * defenseMultiplier(defense) * (hyperion ? HYPERION_WITHER_MULTIPLIER : 1);
  return applySoftCap(raw);
}
