import { CLASSES, TARGET_DEFENSE, BASE_CRIT_CHANCE, HYPERION_ID, HOT_POTATO, FUMING_POTATO } from '../config/constants.js';
import { ITEM_STATS, MISC_BUFFS, REFORGES } from '../config/itemStats.js';
import { ENCHANTS } from '../config/enchants.js';
import {
  calcDamage, calcStrength, calcCritDamage, calcIntelligence,
  dungeonStat, dungeonCritChance, archerDamage, berserkerDamage, mageDamage,
} from './formulas.js';

export const TARGETS = ['wither', 'dragon'];

// Items resource stat names -> internal stat names.
const RESOURCE_STAT_MAP = {
  DAMAGE: 'damage',
  STRENGTH: 'strength',
  CRITICAL_DAMAGE: 'critDamage',
  CRITICAL_CHANCE: 'critChance',
  INTELLIGENCE: 'intelligence',
};

// Flat stats get dungeon scaling. Percent, fraction and multiplier stats do not.
const SCALED_STATS = new Set(['damage', 'strength', 'critDamage', 'intelligence']);
const MULT_STATS = new Set(['multStrength', 'multCritDamage', 'multIntelligence']);

function emptyBucket() {
  return {
    damage: 0, strength: 0, critDamage: 0, critChance: 0, intelligence: 0, additiveDamage: 0,
    additiveStrength: 0, additiveStrength2: 0, multStrength: 1,
    additiveCritDamage: 0, additiveCritDamage2: 0, multCritDamage: 1,
    additiveIntelligence: 0, additiveIntelligence2: 0, multIntelligence: 1,
  };
}

function addInto(bucket, stats) {
  if (!stats) return;
  for (const [key, value] of Object.entries(stats)) {
    if (!(key in bucket) || typeof value !== 'number') continue;
    if (MULT_STATS.has(key)) bucket[key] *= value;
    else bucket[key] += value;
  }
}

// Base stats for an item id: API stats plus anything in itemStats.js.
export function baseItemStats(id, resource) {
  const info = resource.get(id);
  const override = ITEM_STATS[id] ?? {};
  const stats = {};
  if (!override.replaceStats && info?.stats) {
    for (const [apiKey, key] of Object.entries(RESOURCE_STAT_MAP)) {
      if (typeof info.stats[apiKey] === 'number') stats[key] = info.stats[apiKey];
    }
  }
  for (const [key, value] of Object.entries(override.stats ?? {})) stats[key] = (stats[key] ?? 0) + value;
  return {
    global: stats,
    wither: override.wither ?? {},
    dragon: override.dragon ?? {},
    dungeon: override.dungeon ?? Boolean(info?.dungeon_item),
  };
}

function scaleStats(stats, item, ctx) {
  const out = {};
  for (const [key, value] of Object.entries(stats)) {
    if (key === 'critChance') out[key] = dungeonCritChance(value, item.masterStars, ctx.secretDigit);
    else if (SCALED_STATS.has(key)) out[key] = dungeonStat(value, item.masterStars, ctx.cataLevel, ctx.secretDigit);
    else out[key] = value;
  }
  return out;
}

// Stat contribution of one item, split into { global, wither, dragon }.
export function itemContribution(item, ctx) {
  const base = baseItemStats(item.id, ctx.resource);
  const scale = (s) => (base.dungeon ? scaleStats(s, item, ctx) : { ...s });
  const out = { global: scale(base.global), wither: scale(base.wither), dragon: scale(base.dragon) };

  const potatoes = Math.min(item.hpb ?? 0, HOT_POTATO.max + FUMING_POTATO.max);
  if (potatoes > 0 && ctx.isWeapon(item)) {
    out.global.damage = (out.global.damage ?? 0) + potatoes * HOT_POTATO.damage;
    out.global.strength = (out.global.strength ?? 0) + potatoes * HOT_POTATO.strength;
  }

  const reforge = item.reforge && REFORGES[item.reforge]?.[item.rarity];
  if (reforge) addLoose(out.global, reforge);

  for (const [name, level] of Object.entries(item.enchants ?? {})) {
    const ench = ENCHANTS[name];
    if (!ench || !ench.categories.includes(item.category) || level < 1) continue;
    const value = ench.values[Math.min(level, ench.values.length) - 1];
    addLoose(out[ench.target], { [ench.stat]: value });
  }
  return out;
}

function addLoose(target, stats) {
  for (const [key, value] of Object.entries(stats)) {
    if (MULT_STATS.has(key)) target[key] = (target[key] ?? 1) * value;
    else target[key] = (target[key] ?? 0) + value;
  }
}

export function loadoutItems(loadout) {
  const items = [];
  if (loadout.weapon) items.push(loadout.weapon);
  for (const item of Object.values(loadout.armor)) if (item) items.push(item);
  for (const item of Object.values(loadout.equipment)) if (item) items.push(item);
  for (const item of loadout.accessories.values()) items.push(item);
  return items;
}

// Raw stat buckets for each target.
export function rawStats(loadout, ctx) {
  const buckets = { wither: emptyBucket(), dragon: emptyBucket() };
  const misc = MISC_BUFFS[ctx.cls] ?? {};
  for (const target of TARGETS) {
    addInto(buckets[target], misc.stats);
    addInto(buckets[target], misc[target]);
  }
  for (const item of loadoutItems(loadout)) {
    const c = itemContribution(item, ctx);
    for (const target of TARGETS) {
      addInto(buckets[target], c.global);
      addInto(buckets[target], c[target]);
    }
  }
  return buckets;
}

// Final stats after the strength / crit damage / intelligence formulas.
export function finalStats(bucket) {
  return {
    damage: calcDamage(bucket.damage),
    strength: calcStrength(bucket.strength, bucket.additiveStrength, bucket.additiveStrength2, bucket.multStrength),
    critDamage: calcCritDamage(bucket.critDamage, bucket.additiveCritDamage, bucket.additiveCritDamage2, bucket.multCritDamage),
    intelligence: calcIntelligence(bucket.intelligence, bucket.additiveIntelligence, bucket.additiveIntelligence2, bucket.multIntelligence),
    critChance: BASE_CRIT_CHANCE + bucket.critChance,
    additiveDamage: bucket.additiveDamage,
  };
}

export function hasHyperion(loadout) {
  return loadout.weapon?.id === HYPERION_ID || loadout.owned.has(HYPERION_ID);
}

// Damage against each target for the loadout. Returns { wither, dragon, stats }.
export function evaluate(loadout, ctx) {
  const buckets = rawStats(loadout, ctx);
  const result = { stats: {} };
  for (const target of TARGETS) {
    const s = finalStats(buckets[target]);
    const defense = TARGET_DEFENSE[target];
    let dmg;
    if (ctx.cls === 'arch') dmg = archerDamage(s, defense);
    else if (ctx.cls === 'bers') dmg = berserkerDamage(s, defense, CLASSES.bers.baseAdditiveDamage);
    else dmg = mageDamage(s, defense, target === 'wither' && hasHyperion(loadout));
    result[target] = dmg;
    result.stats[target] = s;
  }
  return result;
}

// Weighted percent DPS gain from `before` to `after`.
// weights: { wither, dragon } that sum to 100 for a global upgrade.
export function weightedGain(before, after, weights) {
  let total = 0;
  for (const target of TARGETS) {
    if (!weights[target] || before[target] <= 0) continue;
    const pct = ((after[target] - before[target]) / before[target]) * 100;
    total += (pct * weights[target]) / 100;
  }
  return total;
}
