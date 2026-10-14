import {
  CLASSES, TARGET_DEFENSE, HYPERION_ID, HOT_POTATO, FUMING_POTATO, ART_OF_WAR, RARITIES, SPECIAL_RARITIES,
  ARMOR_SLOTS, CLASS_WEAPON_CATEGORIES, GENERAL_MEDALLION_ID, RAGNAROCK_MULTIPLIER, CHIMERA_PER_LEVEL,
  LEGION_PER_LEVEL, LEGION_PLAYERS, MAX_STARS,
} from '../config/constants.js';
import { ITEM_STATS, MISC_BUFFS, REFORGE_OVERRIDES } from '../config/itemStats.js';
import { ENCHANTS } from '../config/enchants.js';
import { GEMSTONES } from '../config/gemstones.js';
import {
  AP_BY_RARITY, DUNGEON_AP_MULTIPLIER, DOUBLE_AP_ITEMS, POWERS, DEFAULT_POWER, AP_PER_TUNING_POINT,
  TUNING_PER_POINT, DEFAULT_TUNING_SPLIT, ENRICHMENTS,
} from '../config/accessories.js';
import { SHARDS } from '../config/shards.js';
import { EXTRA_UPGRADES } from '../config/upgrades.js';
import { ESSENCE_PERKS } from '../config/essence.js';
import REFORGE_DATA from '../data/reforges.json' with { type: 'json' };
import ACCESSORY_DATA from '../data/accessories.json' with { type: 'json' };
import {
  calcDamage, calcStrength, calcCritDamage, calcIntelligence, calcCritChance,
  dungeonMultiplier, dungeonCritMultiplier, powerMultiplier, secretDigits, blessingEffect,
  archerDamage, berserkerDamage, mageDamage,
} from './formulas.js';

export const TARGETS = ['wither', 'dragon'];

const STAT_ALIASES = {
  DAMAGE: 'damage', STRENGTH: 'strength', CRITICAL_DAMAGE: 'critDamage', CRIT_DAMAGE: 'critDamage',
  CRITICAL_CHANCE: 'critChance', CRIT_CHANCE: 'critChance', INTELLIGENCE: 'intelligence',
  WALK_SPEED: 'speed', SPEED: 'speed',
};

// Flat stats get dungeon scaling. Percent, fraction and multiplier stats do not.
const FLAT_STATS = new Set(['damage', 'strength', 'critDamage', 'critChance', 'intelligence']);
const MULT_STATS = new Set(['multStrength', 'multCritDamage', 'multCritChance', 'multIntelligence']);
const ALL_WEAPON_CATEGORIES = new Set(Object.values(CLASS_WEAPON_CATEGORIES).flat());
const EQUIPMENT_TYPES = new Set(['NECKLACE', 'CLOAK', 'BELT', 'GLOVES', 'BRACELET']);
const IGNORED_ACCESSORIES = new Set(ACCESSORY_DATA.ignored);
export const EXTRAS_BY_KEY = new Map(EXTRA_UPGRADES.map((e) => [e.key, e]));

function emptyBucket() {
  return {
    damage: 0, strength: 0, critDamage: 0, critChance: 0, intelligence: 0, additiveDamage: 0,
    additiveStrength: 0, additiveStrength2: 0, multStrength: 1,
    additiveCritDamage: 0, additiveCritDamage2: 0, multCritDamage: 1,
    additiveCritChance: 0, multCritChance: 1,
    additiveIntelligence: 0, additiveIntelligence2: 0, multIntelligence: 1,
    blessing: 0, blessingShard: 0, speed: 0,
  };
}

function addInto(bucket, stats, scale = 1) {
  if (!stats) return;
  for (const [key, value] of Object.entries(stats)) {
    if (!(key in bucket) || typeof value !== 'number') continue;
    if (MULT_STATS.has(key)) bucket[key] *= value;
    else bucket[key] += value * scale;
  }
}

function addLoose(target, stats, scale = 1) {
  for (const [key, value] of Object.entries(stats ?? {})) {
    if (typeof value !== 'number') continue;
    if (MULT_STATS.has(key)) target[key] = (target[key] ?? 1) * value;
    else target[key] = (target[key] ?? 0) + value * scale;
  }
}

// ---------- Item helpers ----------

export function isWeaponCategory(category) {
  return ALL_WEAPON_CATEGORIES.has(category);
}

export function itemRarity(item) {
  const base = item.baseRarity ?? 'COMMON';
  if (!item.recomb) return base;
  const special = SPECIAL_RARITIES.indexOf(base);
  if (special === 0) return SPECIAL_RARITIES[1];
  const i = RARITIES.indexOf(base);
  return i >= 0 && i < RARITIES.length - 1 ? RARITIES[i + 1] : base;
}

export function canRecomb(item) {
  return !item.recomb && itemRarity({ ...item, recomb: true }) !== (item.baseRarity ?? 'COMMON');
}

export function reforgeKey(name) {
  return String(name ?? '').toLowerCase().replace(/[^a-z]/g, '');
}

export function getReforge(name) {
  const key = reforgeKey(name);
  return REFORGE_OVERRIDES[key] ?? REFORGE_DATA[key] ?? null;
}

export function allReforges() {
  return { ...REFORGE_DATA, ...REFORGE_OVERRIDES };
}

export function reforgeAppliesTo(reforge, item) {
  if (reforge.items?.length) return reforge.items.includes(item.id);
  return (reforge.types ?? []).some((t) => {
    if (t === item.category) return true;
    if (t === 'ARMOR') return ARMOR_SLOTS.includes(item.category);
    if (t === 'EQUIPMENT') return EQUIPMENT_TYPES.has(item.category);
    if (t === 'SWORD') return item.category === 'SWORD' || item.category === 'LONGSWORD';
    return false;
  });
}

export function isDungeon(item, resource) {
  const override = ITEM_STATS[item.id]?.dungeon;
  if (override !== undefined) return override;
  if (item.dungeon !== undefined) return item.dungeon;
  return Boolean(resource.get(item.id)?.dungeon_item);
}

// Base stats for an item id: API stats (case-insensitive, highest tier of tiered_stats) plus itemStats.js.
export function baseItemStats(id, resource, cls) {
  const info = resource.get(id);
  const override = ITEM_STATS[id] ?? {};
  const classOk = !override.classes || !cls || override.classes.includes(cls);
  const stats = {};
  if (!override.replaceStats && info) {
    for (const [apiKey, value] of Object.entries(info.stats ?? {})) {
      const key = STAT_ALIASES[apiKey.toUpperCase()];
      if (key && typeof value === 'number') stats[key] = (stats[key] ?? 0) + value;
    }
    for (const [apiKey, values] of Object.entries(info.tiered_stats ?? {})) {
      const key = STAT_ALIASES[apiKey.toUpperCase()];
      if (key && Array.isArray(values) && values.length) stats[key] = (stats[key] ?? 0) + values[values.length - 1];
    }
  }
  if (classOk) for (const [key, value] of Object.entries(override.stats ?? {})) addLoose(stats, { [key]: value });
  return { global: stats, wither: classOk ? override.wither ?? {} : {}, dragon: classOk ? override.dragon ?? {} : {} };
}

export function gemStat(gem, quality, rarity) {
  const table = GEMSTONES[gem];
  if (!table) return null;
  const row = table.values[quality];
  if (!row) return null;
  const i = Math.min(Math.max(RARITIES.indexOf(rarity), 0), row.length - 1);
  return { [table.stat]: row[i] };
}

// ---------- One item's stats ----------

function scaleBucket(stats, item, ctx, dungeon) {
  if (!dungeon) return stats;
  const stars = item.stars ?? MAX_STARS;
  const flat = dungeonMultiplier(ctx.cataLevel, stars, item.masterStars ?? 0, ctx.secretDigit ?? 0);
  const crit = dungeonCritMultiplier(stars, item.masterStars ?? 0, ctx.secretDigit ?? 0);
  const out = {};
  for (const [key, value] of Object.entries(stats)) {
    if (key === 'critChance') out[key] = value * crit;
    else if (FLAT_STATS.has(key)) out[key] = value * flat;
    else out[key] = value;
  }
  return out;
}

// Unscaled stats of one item, split by target.
export function itemRawStats(item, ctx, { chimeraPetStrength = 0 } = {}) {
  const base = baseItemStats(item.id, ctx.resource, ctx.cls);
  const out = { global: { ...base.global }, wither: { ...base.wither }, dragon: { ...base.dragon } };
  const rarity = itemRarity(item);
  const weapon = isWeaponCategory(item.category);

  const reforge = item.reforge && getReforge(item.reforge);
  if (reforge && reforgeAppliesTo(reforge, item)) {
    addLoose(out.global, reforge.stats?.[rarity]);
    if (reforge.cataStat) addLoose(out.global, { [reforge.cataStat]: ctx.cataLevel });
  }

  for (const { gem, quality } of Object.values(item.gems?.slots ?? {})) addLoose(out.global, gemStat(gem, quality, rarity));

  if (weapon) {
    const potatoes = Math.min(item.hpb ?? 0, HOT_POTATO.max + FUMING_POTATO.max);
    addLoose(out.global, { damage: potatoes * HOT_POTATO.damage, strength: potatoes * HOT_POTATO.strength });
    if (item.artOfWar) addLoose(out.global, { strength: ART_OF_WAR.strength });
  }

  for (const [name, level] of Object.entries(item.enchants ?? {})) {
    const ench = ENCHANTS[name];
    if (!ench || level < 1 || !ench.categories.includes(item.category)) continue;
    if (ench.items && !ench.items.includes(item.id)) continue;
    if (ench.classes && !ench.classes.includes(ctx.cls)) continue;
    if (ench.chimera) {
      addLoose(out.global, { strength: CHIMERA_PER_LEVEL * level * chimeraPetStrength });
      continue;
    }
    if (ench.legion) continue;
    for (const [stat, values] of Object.entries(ench.stats ?? {})) {
      addLoose(out[ench.target], { [stat]: values[Math.min(level, values.length) - 1] });
    }
  }
  return out;
}

// Stats of one equipped item after dungeon scaling.
export function itemContribution(item, ctx, opts) {
  const raw = itemRawStats(item, ctx, opts);
  const dungeon = isDungeon(item, ctx.resource);
  return { global: scaleBucket(raw.global, item, ctx, dungeon), wither: scaleBucket(raw.wither, item, ctx, dungeon), dragon: scaleBucket(raw.dragon, item, ctx, dungeon) };
}

// ---------- Accessories ----------

export function accessorySuperseded(id, accessoryIds) {
  return (ACCESSORY_DATA.upgrades[id] ?? []).some((up) => accessoryIds.has(up));
}

export function countedAccessories(loadout) {
  const ids = new Set(loadout.accessories.keys());
  const out = [];
  for (const item of loadout.accessories.values()) {
    if (IGNORED_ACCESSORIES.has(item.id) || accessorySuperseded(item.id, ids)) continue;
    out.push(item);
  }
  return out;
}

export function accessoryPower(item, resource) {
  let ap = AP_BY_RARITY[itemRarity(item)] ?? 0;
  if (resource.get(item.id)?.dungeon_item) ap *= DUNGEON_AP_MULTIPLIER;
  if (DOUBLE_AP_ITEMS.includes(item.id)) ap *= 2;
  return ap;
}

export function totalAccessoryPower(loadout, resource) {
  return countedAccessories(loadout).reduce((sum, item) => sum + accessoryPower(item, resource), 0);
}

function tuningStats(points, ctx) {
  const stats = {};
  const base = ctx.tuning;
  const basePoints = base?.basePoints ?? 0;
  if (base?.allocation && Object.keys(base.allocation).length) {
    for (const [stat, n] of Object.entries(base.allocation)) addLoose(stats, { [stat]: n * (TUNING_PER_POINT[stat] ?? 0) });
  } else {
    distribute(stats, Math.min(points, basePoints), ctx);
  }
  distribute(stats, points - basePoints, ctx);
  return stats;
}

// New tuning points follow the profile's split over DPS stats (never speed), or the class default.
function distribute(stats, points, ctx) {
  if (!points) return;
  const entries = Object.entries(ctx.tuning?.split ?? {}).filter(([stat]) => stat !== 'speed' && TUNING_PER_POINT[stat]);
  const total = entries.reduce((sum, [, share]) => sum + share, 0);
  const split = total ? entries.map(([stat, share]) => [stat, share / total]) : Object.entries(DEFAULT_TUNING_SPLIT[ctx.cls]);
  for (const [stat, share] of split) addLoose(stats, { [stat]: points * share * (TUNING_PER_POINT[stat] ?? 0) });
}

// Speed above (or below) the starting loadout moves tuning points off (or onto) speed.
function speedTuning(speed, ctx) {
  if (ctx.baseSpeed === undefined) return {};
  let points = (speed - ctx.baseSpeed) / TUNING_PER_POINT.speed;
  const inSpeed = ctx.tuning?.allocation ? ctx.tuning.allocation.speed ?? 0 : Infinity;
  if (points > inSpeed) points = inSpeed;
  const stats = {};
  distribute(stats, points, ctx);
  return stats;
}

// Accessory bag: own stats, Accessory Power, power stats, tuning, enrichments.
export function accessoryStats(loadout, ctx, extraTuningPoints = 0) {
  const counted = countedAccessories(loadout);
  const stats = {};
  let ap = 0;
  for (const item of counted) {
    ap += accessoryPower(item, ctx.resource);
    addLoose(stats, itemRawStats(item, ctx).global);
    const enrich = item.enrichment && ENRICHMENTS[item.enrichment];
    if (enrich) addLoose(stats, enrich.stats);
  }
  const power = POWERS[ctx.power] ?? POWERS[DEFAULT_POWER[ctx.cls]];
  const mult = powerMultiplier(ap);
  addLoose(stats, power.stats, mult);
  addLoose(stats, power.bonus);
  const points = Math.floor(ap / AP_PER_TUNING_POINT) + extraTuningPoints;
  addLoose(stats, tuningStats(points, ctx));
  return { stats, ap, tuningPoints: points };
}

// ---------- Shards ----------

export function shardLevel(loadout, id) {
  return Math.max(0, Math.min(10, loadout.shards?.[id] ?? 0));
}

export function shardStats(loadout, ctx) {
  const out = { global: {}, wither: {}, dragon: {}, tuningPoints: 0 };
  const level = (id) => shardLevel(loadout, id);
  const tiamat = 1 + (SHARDS.TIAMAT.perLevel.echo * level('TIAMAT'));
  const echo = {};
  for (const [id, s] of Object.entries(SHARDS)) {
    if (s.echo && s.echo !== 'echo') echo[s.echo] = 1 + s.perLevel.echo * level(id) * tiamat;
  }
  for (const [id, s] of Object.entries(SHARDS)) {
    if (s.echo || !level(id)) continue;
    if (s.classes && !s.classes.includes(ctx.cls)) continue;
    const mult = (s.family && echo[s.family]) || 1;
    const { tuningPoints, ...stats } = s.perLevel;
    if (tuningPoints) out.tuningPoints += tuningPoints * level(id) * mult;
    addLoose(out[s.target ?? 'global'], stats, level(id) * mult);
  }
  out.tuningPoints = Math.floor(out.tuningPoints);
  return out;
}

// ---------- Extras (pets, ability items, perks) ----------

function extraAllowed(entry, loadout, cls) {
  if (entry.classes && !entry.classes.includes(cls)) return false;
  if (entry.requires && !loadout.extras.has(entry.requires)) return false;
  return true;
}

function extraStats(entries, active, bucket) {
  for (const e of entries) {
    if (e.activeWith && !active.has(e.activeWith)) continue;
    addLoose(bucket.global, e.stats);
    addLoose(bucket.wither, e.wither);
    addLoose(bucket.dragon, e.dragon);
  }
}

function petStrength(entries, active) {
  return entries.filter((e) => !e.activeWith || active.has(e.activeWith)).reduce((sum, e) => sum + (e.petStrength ?? 0), 0);
}

// ---------- Whole loadout ----------

export function loadoutGear(loadout) {
  return [loadout.weapon, ...Object.values(loadout.armor), ...Object.values(loadout.equipment)].filter(Boolean);
}

function secretDigitFor(loadout, ctx) {
  return loadout.accessories.has(GENERAL_MEDALLION_ID) || loadout.owned.has(GENERAL_MEDALLION_ID) ? secretDigits(ctx.secrets) : 0;
}

// Raw stat buckets for each target with a fixed set of active extras.
function rawStats(loadout, ctx, activeExtras) {
  const c = { ...ctx, secretDigit: secretDigitFor(loadout, ctx) };
  const buckets = { wither: emptyBucket(), dragon: emptyBucket() };
  const add = (stats, target) => {
    if (!target || target === 'global') for (const t of TARGETS) addInto(buckets[t], stats);
    else addInto(buckets[target], stats);
  };
  const misc = MISC_BUFFS[c.cls] ?? {};
  add(misc.stats);
  add(misc.wither, 'wither');
  add(misc.dragon, 'dragon');

  for (const item of loadoutGear(loadout)) {
    const contrib = itemContribution(item, c);
    add(contrib.global);
    add(contrib.wither, 'wither');
    add(contrib.dragon, 'dragon');
  }

  // Legion: percent of all combat stats per piece of armor.
  let legion = 0;
  for (const item of Object.values(loadout.armor)) legion += (item?.enchants?.ultimate_legion ?? 0) * LEGION_PER_LEVEL * LEGION_PLAYERS;
  if (legion) add({ additiveStrength: legion, additiveCritDamage: legion, additiveCritChance: legion, additiveIntelligence: legion });

  const extrasBucket = { global: {}, wither: {}, dragon: {} };
  const activeKeys = new Set(activeExtras.map((e) => e.key));
  extraStats(activeExtras, activeKeys, extrasBucket);
  add(extrasBucket.global);
  add(extrasBucket.wither, 'wither');
  add(extrasBucket.dragon, 'dragon');

  if (loadout.rag) {
    const rag = itemRawStats(loadout.rag, c, { chimeraPetStrength: petStrength(activeExtras, activeKeys) });
    add({ strength: RAGNAROCK_MULTIPLIER * (rag.global.strength ?? 0) });
  }

  // Essence shop perks.
  for (const [key, perk] of Object.entries(ESSENCE_PERKS)) {
    const level = loadout.perks?.[key] ?? 0;
    if (!level) continue;
    const { petCritDamage, ...stats } = perk.perLevel;
    add(Object.fromEntries(Object.entries(stats).map(([k, v]) => [k, v * level])));
    const pet = petCritDamage && activeExtras.find((e) => e.key === perk.pet);
    if (pet) add({ critDamage: (pet.stats?.critDamage ?? 0) * petCritDamage * level });
  }

  const shards = shardStats(loadout, c);
  add(shards.global);
  add(shards.wither, 'wither');
  add(shards.dragon, 'dragon');

  const acc = accessoryStats(loadout, c, shards.tuningPoints);
  add(acc.stats);
  const speed = buckets.wither.speed;
  add(speedTuning(speed, c));
  return { buckets, ap: acc.ap, speed };
}

// Final stats after the strength / crit damage / intelligence / crit chance formulas.
export function finalStats(bucket, cls) {
  const effect = blessingEffect(bucket.blessing, bucket.blessingShard);
  return {
    damage: calcDamage(bucket.damage),
    strength: calcStrength(bucket.strength, bucket.additiveStrength, bucket.additiveStrength2, bucket.multStrength, effect),
    critDamage: calcCritDamage(bucket.critDamage, bucket.additiveCritDamage, bucket.additiveCritDamage2, bucket.multCritDamage, effect),
    intelligence: calcIntelligence(bucket.intelligence, bucket.additiveIntelligence, bucket.additiveIntelligence2, bucket.multIntelligence, effect),
    critChance: calcCritChance(bucket.critChance, bucket.additiveCritChance, bucket.multCritChance),
    // Berserker's 1200 is added inside berserkerDamage.
    additiveDamage: bucket.additiveDamage + (cls === 'bers' ? 0 : CLASSES[cls]?.baseAdditiveDamage ?? 0),
  };
}

export function hasHyperion(loadout) {
  return loadout.weapon?.id === HYPERION_ID || loadout.owned.has(HYPERION_ID);
}

function evaluateWith(loadout, ctx, activeExtras) {
  const { buckets, ap, speed } = rawStats(loadout, ctx, activeExtras);
  const result = { stats: {}, ap, speed };
  for (const target of TARGETS) {
    const s = finalStats(buckets[target], ctx.cls);
    const defense = TARGET_DEFENSE[target];
    let dmg;
    if (ctx.cls === 'arch') dmg = archerDamage(s, defense);
    else if (ctx.cls === 'bers') dmg = berserkerDamage(s, defense, CLASSES.bers.baseAdditiveDamage);
    else dmg = mageDamage(s, defense, target === 'wither' && hasHyperion(loadout));
    result[target] = dmg * (ITEM_STATS[loadout.weapon?.id]?.dpsMultiplier ?? 1);
    result.stats[target] = s;
  }
  return result;
}

// Owned extras that count right now. In each group, the member that gives the most damage wins.
export function activeExtras(loadout, ctx, weights) {
  const owned = EXTRA_UPGRADES.filter((e) => loadout.extras.has(e.key) && extraAllowed(e, loadout, ctx.cls));
  const fixed = owned.filter((e) => !e.group);
  const groups = new Map();
  for (const e of owned.filter((x) => x.group)) {
    if (!groups.has(e.group)) groups.set(e.group, []);
    groups.get(e.group).push(e);
  }
  let active = [...fixed];
  for (const members of groups.values()) {
    let best = members[0];
    if (members.length > 1) {
      let bestScore = -Infinity;
      for (const m of members) {
        const r = evaluateWith(loadout, ctx, [...active, m]);
        const score = r.wither * (weights?.wither ?? 50) + r.dragon * (weights?.dragon ?? 50);
        if (score > bestScore) { bestScore = score; best = m; }
      }
    }
    active.push(best);
  }
  // Extras tied to a group member (pet items) only count with that member.
  const keys = new Set(active.map((e) => e.key));
  active = active.filter((e) => !e.activeWith || keys.has(e.activeWith));
  return active;
}

// Damage against each target for the loadout. Returns { wither, dragon, stats, ap, extras }.
export function evaluate(loadout, ctx, weights) {
  const extras = activeExtras(loadout, ctx, weights);
  return { ...evaluateWith(loadout, ctx, extras), extras };
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
