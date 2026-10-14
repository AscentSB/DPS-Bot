import {
  BASE_DAMAGE, BASE_STRENGTH, BASE_CRIT_DAMAGE, BASE_INTELLIGENCE,
  STRENGTH_CONSTANT, CRIT_DAMAGE_CONSTANT, INTELLIGENCE_CONSTANT,
  CRIT_CHANCE_CAP, MAGE_INT_DIVISOR, MAGE_ABILITY_SCALING, MAGE_ABILITY_BASE,
  HYPERION_WITHER_MULTIPLIER, SOFT_CAP, SOFT_CAP_KEEP,
  DUNGEON_BASE_ADD, MASTER_STAR_STEP, CRIT_BASE, CRIT_MASTER_STAR_STEP,
  SECRET_STEP, SECRET_DIGIT_CAP, CATA_STAT_BONUS, CATA_XP_PER_LEVEL,
} from '../config/constants.js';

// ---------- Stat calculations ----------

export function calcDamage(weaponDamage) {
  return BASE_DAMAGE + weaponDamage;
}

// (Strength + 76)(1 + additive + 2nd additive)(multiplicative)(1.1 * 1.1 * 1.2 * .38)
export function calcStrength(strength, additive = 0, additive2 = 0, multiplicative = 1) {
  return (strength + BASE_STRENGTH) * (1 + additive + additive2) * multiplicative * STRENGTH_CONSTANT;
}

// (CritDamage + 76)(1 + additive + 2nd additive)(multiplicative)(1.1 * 1.1 * 1.2 * .38)
export function calcCritDamage(critDamage, additive = 0, additive2 = 0, multiplicative = 1) {
  return (critDamage + BASE_CRIT_DAMAGE) * (1 + additive + additive2) * multiplicative * CRIT_DAMAGE_CONSTANT;
}

// (Intelligence + 48)(1 + additive + 2nd additive)(multiplicative)(1.1 * 1.1 * 1.2 * .24)
export function calcIntelligence(intelligence, additive = 0, additive2 = 0, multiplicative = 1) {
  return (intelligence + BASE_INTELLIGENCE) * (1 + additive + additive2) * multiplicative * INTELLIGENCE_CONSTANT;
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

// basestat * (cataBoost + 1.5 + 5 * masterstars + .01 * secretdigit)
export function dungeonStat(baseStat, masterStars, cataLevel, secretDigit = 0) {
  return baseStat * (cataBoost(cataLevel) + DUNGEON_BASE_ADD + MASTER_STAR_STEP * masterStars + SECRET_STEP * secretDigit);
}

// critchance * (1.5 + .05 * masterstars + .01 * secretdigit), secret digit capped at 5
export function dungeonCritChance(baseCritChance, masterStars, secretDigit = 0) {
  const digit = Math.min(secretDigit, SECRET_DIGIT_CAP);
  return baseCritChance * (CRIT_BASE + CRIT_MASTER_STAR_STEP * masterStars + SECRET_STEP * digit);
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

function critFormula(s, defense) {
  const cc = Math.min(s.critChance, CRIT_CHANCE_CAP);
  const part = s.damage * (1 + s.critDamage / 100) * (1 + s.strength / 100) * (1 + s.additiveDamage / 100) * defenseMultiplier(defense);
  return part * (2 - cc / 400) + 1.5 * part * (cc / 400 - 1);
}

// Archer. stats: { damage, critDamage, strength, additiveDamage, critChance }
export function archerDamage(s, defense) {
  return applySoftCap(critFormula(s, defense));
}

// Berserker. Same formula as archer. The 1200 base additive damage is added here.
export function berserkerDamage(s, defense, baseAdditiveDamage = 1200) {
  return applySoftCap(critFormula({ ...s, additiveDamage: s.additiveDamage + baseAdditiveDamage }, defense));
}

// Mage. hyperion = true applies the 1.5x Hyperion multiplier (use only for withers).
export function mageDamage(s, defense, hyperion = false) {
  const melee = s.damage * (1 + s.critDamage / 100) * (1 + s.strength / 100) * (1 + s.additiveDamage / 100);
  const ability = MAGE_ABILITY_SCALING * (MAGE_ABILITY_BASE + s.intelligence / MAGE_INT_DIVISOR);
  const raw = (melee + ability) * defenseMultiplier(defense) * (hyperion ? HYPERION_WITHER_MULTIPLIER : 1);
  return applySoftCap(raw);
}
