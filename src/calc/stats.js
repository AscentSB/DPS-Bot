import {
  CLASSES, TARGET_DEFENSE, NECRON_BLADE_WITHER_MULTIPLIER, DUAL_WIELD_ADDITIVE, HOT_POTATO, FUMING_POTATO, ART_OF_WAR, RARITIES, SPECIAL_RARITIES,
  ARMOR_SLOTS, CLASS_WEAPON_CATEGORIES, GENERAL_MEDALLION_ID, RAGNAROCK_MULTIPLIER, CHIMERA_PER_LEVEL,
  LEGION_PER_LEVEL, LEGION_PLAYERS, MAGE_BEAM_TICKS, MAGE_INT_BASE, MAGE_INT_PER_LEVEL, SOLO_CLASS_MULTIPLIER, ARCHER_ARROW_DAMAGE_PER_LEVEL, BERSERK_DAMAGE_BASE, LUST_FOR_BLOOD_PER_LEVEL,
  BERSERK_DAMAGE_PER_LEVEL, WARRIOR_PER_LEVEL, WARRIOR_PER_LEVEL_AFTER_50, COMBAT_CRIT_CHANCE_PER_LEVEL, LEGION_GROUPED_PLAYERS, MAX_STARS, MAX_REFORGE_ITEMS, ATTACK_SPEED_CAP, TICKS_PER_SECOND,
  DOUBLESHOT_BASE, DOUBLESHOT_PER_LEVEL, BASE_SPEED, speedCap, sheetSpeedFor, CLEAR_BOOTS_CLASSES, EXTRA_SPEED, DUNGEON_POTION,
  WITHER_IMPACT_WEAPONS, WITHER_SCROLLS, ULTIMATE_WITHER_SCROLL, SA_CLOAK_IDS, SA_CLOAK_MULTIPLIER, DEFAULT_MAXED_COLLECTIONS, SA_CLOAK_COOLDOWN_S, MAGE_COOLDOWN_REDUCTION_BASE, MAGE_COOLDOWN_REDUCTION_PER_LEVEL, MAGE_COOLDOWN_REDUCTION_MAX, MAGE_ABILITY_DAMAGE_BASE, MAGE_ABILITY_DAMAGE_PER_5_LEVELS, ENCHANTING_ABILITY_DAMAGE_PER_LEVEL, ENCHANTING_MAX_LEVEL, LOVING_ABILITY_MULTIPLIER, IMPLOSION_BELT_ID, IMPLOSION_BELT_MULTIPLIER, FABLED_MULTIPLIER, FEROCITY_EFFECTIVENESS, ARROW_DAMAGE, BOSS_SWARM_ENEMIES, RCM_SWARM_ENEMIES, RCM_ENCHANTS, THUNDERLORD_EVERY, MAGE_FIRST_STRIKE_DPS,
} from '../config/constants.js';
import { ITEM_STATS, MISC_BUFFS, REFORGE_OVERRIDES } from '../config/itemStats.js';
import { ENCHANTS } from '../config/enchants.js';
import { GEMSTONES } from '../config/gemstones.js';
import {
  AP_BY_RARITY, DUNGEON_AP_MULTIPLIER, DOUBLE_AP_ITEMS, POWERS, DEFAULT_POWER, AP_PER_TUNING_POINT,
  TUNING_PER_POINT, DEFAULT_TUNING_SPLIT, ENRICHMENTS, NO_RECOMB, HATCESSORY, AP_OVERRIDES, tuningStats,
} from '../config/accessories.js';
import { SHARDS } from '../config/shards.js';
import { EXTRA_UPGRADES } from '../config/upgrades.js';
import { ESSENCE_PERKS } from '../config/essence.js';
import { ARMOR_SETS } from '../config/armorSets.js';
import { petStats } from '../lib/pets.js';
import { floorTierIndex, dropStatMultiplier, dropBaseRarity } from '../lib/drops.js';
import { EXCLUDED_PETS, TIER_BOOST_ID } from '../config/pets.js';
import { BASE_CRIT_DAMAGE, SKILL_BONUSES, SLAYER_BONUSES, SB_STRENGTH_EVERY, HARP_SONGS, BOOK_OF_PROGRESSION_ID, BOOK_DAMAGE_PER_LEVEL, BOOK_MAX_LEVEL, BOOK_DAMAGE_OPTION, BOOK_DAMAGE_UNLOCK_LEVEL } from '../config/profile.js';
import REFORGE_DATA from '../data/reforges.json' with { type: 'json' };
import ACCESSORY_DATA from '../data/accessories.json' with { type: 'json' };
import {
  calcDamage, calcStrength, calcCritDamage, calcIntelligence, calcCritChance,
  dungeonMultiplier, dungeonCritMultiplier, powerMultiplier, secretDigits, blessingEffect, cataBoost,
  archerDamage, berserkerDamage, mageDamage, rcmDamage,
} from './formulas.js';

// Mage RCM mode: the clear set, scored on Wither Impact damage against clear mobs (the "wither" target;
// boss-only bonuses do not apply). See rcmDamage.
// A Necron's Blade variant with Wither Impact (all 3 scrolls, or the Ultimate Wither Scroll).
export function hasWitherImpact(item) {
  if (!item || !WITHER_IMPACT_WEAPONS.includes(item.id)) return false;
  const s = item.scrolls ?? [];
  return s.includes(ULTIMATE_WITHER_SCROLL) || WITHER_SCROLLS.every((x) => s.includes(x));
}

// Flat stats from the profile: base crit damage, skills, slayers, SkyBlock level, Harp songs, cake buffs.
// Only counted when the profile was read (ctx.profileStats).
export function profileStatTotals(ctx) {
  const p = ctx.profileStats;
  if (!p) return {};
  return memo(profileStore, p, ctx.player ?? profileStore, () => computeProfileStats(ctx, p));
}

function computeProfileStats(ctx, p) {
  const out = { critDamage: BASE_CRIT_DAMAGE };
  for (const [skill, { max, steps }] of Object.entries(SKILL_BONUSES)) {
    const level = Math.min(max, ctx.player?.skills?.[skill] ?? max);
    const keys = Object.keys(steps).map(Number).sort((a, b) => a - b);
    for (let l = 1; l <= level; l++) addLoose(out, steps[keys.filter((k) => k <= l).at(-1)]);
  }
  for (const [boss, rewards] of Object.entries(SLAYER_BONUSES)) {
    const level = ctx.player?.slayers?.[boss] ?? 0;
    for (let l = 1; l <= level; l++) addLoose(out, rewards[l]);
  }
  addLoose(out, { strength: Math.floor((p.sbLevel ?? 0) / SB_STRENGTH_EVERY) });
  for (const song of p.harpSongs ?? []) addLoose(out, { intelligence: HARP_SONGS[song]?.intelligence ?? 0 });
  for (const cake of p.cakes ?? []) addLoose(out, { [cake.stat]: cake.amount });
  return out;
}

// ---------- Memo ----------
// Items, pets and profile parts are never mutated (every change makes a new object), so a result cached by object
// identity plus the context fields it reads stays valid. This keeps the planner from recomputing every unchanged
// item on each of its thousands of evaluations.
function memo(store, obj, key, compute) {
  let cache = store.get(obj);
  if (!cache) store.set(obj, (cache = new Map()));
  let value = cache.get(key);
  if (value === undefined) cache.set(key, (value = compute()));
  return value;
}
const rawStore = new WeakMap();
const contribStore = new WeakMap();
const accStore = new WeakMap();
const apStore = new WeakMap();
const profileStore = new WeakMap();
// The context fields item stats depend on, as one string, worked out once per context object.
const resourceIds = new WeakMap();
const ctxKeys = new WeakMap();
function itemKey(ctx) {
  let key = ctxKeys.get(ctx);
  if (key === undefined) {
    if (!resourceIds.has(ctx.resource)) resourceIds.set(ctx.resource, resourceIds.size + 1);
    key = `${resourceIds.get(ctx.resource)}|${ctx.cls}|${ctx.cataLevel}|${ctx.floorMode}|${ctx.mode}|${ctx.secretDigit ?? 0}|${maxedCollections(ctx)}`;
    ctxKeys.set(ctx, key);
  }
  return key;
}

// Content key of everything evaluate() reads from a loadout. Two loadouts with the same key score the same, even
// when they are different objects (the planner reaches the same gear by different paths).
const itemKeys = new WeakMap();
const contentKey = (item) => {
  if (!item) return '';
  let key = itemKeys.get(item);
  if (key === undefined) itemKeys.set(item, (key = JSON.stringify(item)));
  return key;
};
export function loadoutKey(l) {
  const slots = (group) => Object.keys(group ?? {}).sort().map((slot) => `${slot}=${contentKey(group[slot])}`).join(';');
  return [
    contentKey(l.weapon), contentKey(l.rag), contentKey(l.clearBoots), slots(l.armor), slots(l.equipment),
    [...l.accessories.values()].map(contentKey).sort().join(';'), JSON.stringify(l.pets ?? []), JSON.stringify(l.activePets ?? {}),
    JSON.stringify(l.shards ?? {}), JSON.stringify(l.perks ?? {}), [...(l.extras ?? [])].sort().join(','), l.power ?? '', l.bopBonus ?? '',
    Boolean(l.witherImpact), l.owned?.has(GENERAL_MEDALLION_ID),
    l.dragonGear ? [contentKey(l.dragonGear.weapon), slots(l.dragonGear.armor), slots(l.dragonGear.equipment)].join('|') : '', l.bank ?? '',
  ].join('\n');
}

// Mage LCM can swap gear for dragons: loadout.dragonGear { weapon, armor, equipment } holds only the slots that
// differ. The gear used against `target`, as a loadout (cached, so its item stats stay cached too).
const dragonViews = new WeakMap();
export function gearView(loadout, target) {
  const d = target === 'dragon' ? loadout.dragonGear : null;
  if (!d) return loadout;
  let view = dragonViews.get(loadout);
  if (!view) {
    view = { ...loadout, weapon: d.weapon ?? loadout.weapon, armor: { ...loadout.armor, ...d.armor }, equipment: { ...loadout.equipment, ...d.equipment }, dragonGear: null };
    dragonViews.set(loadout, view);
  }
  return view;
}

// The pet profile with this loadout's bank (Golden Dragon's Legendary Treasure), one object per bank amount so pet
// stats stay cached.
const bankProfiles = new WeakMap();
export function petProfileOf(loadout, ctx) {
  const base = ctx.petProfile ?? {};
  const bank = loadout?.bank ?? base.bank ?? 0;
  let byBank = bankProfiles.get(base);
  if (!byBank) bankProfiles.set(base, (byBank = new Map()));
  if (!byBank.has(bank)) byBank.set(bank, { ...base, bank });
  return byBank.get(bank);
}

// Shadow Assassin Cloak averaged over every hit: one hit in (cooldown x hits per second) gets 1.1x.
export function saCloakMultiplier(hitsPerSecond, ctx) {
  const reduction = ctx?.cls === 'mage' ? SOLO_CLASS_MULTIPLIER * MAGE_COOLDOWN_REDUCTION_BASE + Math.min(MAGE_COOLDOWN_REDUCTION_MAX, MAGE_COOLDOWN_REDUCTION_PER_LEVEL * (ctx.mageLevel ?? 0)) : 0;
  const hitsPerCooldown = Math.max(1, SA_CLOAK_COOLDOWN_S * (1 - reduction / 100) * hitsPerSecond);
  return 1 + (SA_CLOAK_MULTIPLIER - 1) / hitsPerCooldown;
}

// Maxed collections for The One. Unknown (collections API off or not loaded): all of them.
export function maxedCollections(ctx) {
  return ctx?.collections?.maxed ?? DEFAULT_MAXED_COLLECTIONS;
}

export function isRcm(ctx) {
  return ctx?.cls === 'mage' && ctx?.mode === 'rcm';
}

const RAGNAROCK_WEAPON_ID = 'RAGNAROCK_AXE';

export const TARGETS = ['wither', 'dragon'];

const STAT_ALIASES = {
  DAMAGE: 'damage', STRENGTH: 'strength', CRITICAL_DAMAGE: 'critDamage', CRIT_DAMAGE: 'critDamage',
  CRITICAL_CHANCE: 'critChance', CRIT_CHANCE: 'critChance', INTELLIGENCE: 'intelligence',
  WALK_SPEED: 'speed', SPEED: 'speed', ATTACK_SPEED: 'attackSpeed', BONUS_ATTACK_SPEED: 'attackSpeed',
  ABILITY_DAMAGE_PERCENT: 'abilityDamage', ABILITY_DAMAGE: 'abilityDamage', FEROCITY: 'ferocity',
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
    blessing: 0, blessingShard: 0, speed: 0, bowDamage: 0, attackSpeed: 0, additiveAttackSpeed: 0, arrows: 0, abilityDamage: 0, magicAdditive: 0, ferocity: 0, additiveFerocity: 0,
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

// Shots per second for shortbows, 1 otherwise (regular bows, mage).
export function volleysPerSecond(weapon, attackSpeed, ctx) {
  const as = Math.min(Math.max(attackSpeed, 0), ATTACK_SPEED_CAP);
  // Mage beam (LCM): rounds to the nearest tick like melee.
  if (ctx?.cls === 'mage') return TICKS_PER_SECOND / Math.round(MAGE_BEAM_TICKS / (1 + as / 100));
  const shotTicks = ITEM_STATS[weapon?.id]?.shotTicks;
  if (shotTicks) return TICKS_PER_SECOND / Math.ceil(shotTicks / (1 + as / 100) - 1e-9);
  return 1;
}

// Arrows per shot: the bow's arrows + Duplex + Archer Doubleshot (bows only).
export function arrowsPerVolley(weapon, extraArrows, ctx) {
  if (weapon?.category !== 'BOW') return 1;
  const base = ITEM_STATS[weapon.id]?.arrows ?? 1;
  const doubleshot = ctx.cls === 'arch' ? DOUBLESHOT_BASE + DOUBLESHOT_PER_LEVEL * (ctx.archerLevel ?? 0) : 0;
  return base + extraArrows + doubleshot;
}

// Attack speed after percent boosts (Ender Dragon's Superior perk raises all of it, tuning included).
export function attackSpeedOf(bucket) {
  return bucket.attackSpeed * (1 + bucket.additiveAttackSpeed);
}

function isBow(item) {
  return item?.category === 'BOW';
}

export function isWeaponCategory(category) {
  return ALL_WEAPON_CATEGORIES.has(category);
}

export function itemRarity(item) {
  const base = item.baseRarity ?? 'COMMON';
  if (!item.recomb || MAX_REFORGE_ITEMS.test(item.id ?? '')) return base;
  const special = SPECIAL_RARITIES.indexOf(base);
  if (special === 0) return SPECIAL_RARITIES[1];
  const i = RARITIES.indexOf(base);
  return i >= 0 && i < RARITIES.length - 1 ? RARITIES[i + 1] : base;
}

// SPECIAL items use MYTHIC (max) reforge and gem stats, VERY_SPECIAL use DIVINE
// (the M7 archer sheet's Diamond Necron Head + Ancient numbers match MYTHIC Ancient).
export function statRarity(rarity) {
  return { SPECIAL: 'MYTHIC', VERY_SPECIAL: 'DIVINE' }[rarity] ?? rarity;
}

// The items resource marks items that cannot be recombobulated with can_recombobulate: false
// (Voter Badges, Runebook, Safety Badge, Beastslayer's Codex, Pandora's Box, Rift Prism, ...).
export function canRecomb(item, resource) {
  if (NO_RECOMB.includes(item.id) || MAX_REFORGE_ITEMS.test(item.id ?? '')) return false;
  if (resource?.get(item.id)?.can_recombobulate === false) return false;
  return !item.recomb && itemRarity({ ...item, recomb: true }) !== (item.baseRarity ?? 'COMMON');
}

export function reforgeKey(name) {
  return String(name ?? '').toLowerCase().replace(/[^a-z]/g, '');
}

export function getReforge(name) {
  const key = reforgeKey(name);
  return REFORGE_OVERRIDES[key] ?? REFORGE_DATA[key] ?? null;
}

const ALL_REFORGES = { ...REFORGE_DATA, ...REFORGE_OVERRIDES };
export function allReforges() {
  return ALL_REFORGES;
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
  // Dungeonizable items (e.g. Terminator) are assumed converted and starred.
  const info = resource.get(item.id);
  return Boolean(info?.dungeon_item || info?.dungeon_item_conversion_cost);
}

// Base stats for an item id: API stats (case-insensitive) plus itemStats.js. tiered_stats use the highest floor
// the player can enter (the top tier when cataLevel is unknown); floor-scaled dungeon armor gets its 50% boost.
// Pure in (id, class, Catacombs level, floor mode) for one items resource, so cached by those.
const baseStore = new WeakMap();
export function baseItemStats(id, resource, cls, cataLevel = Infinity, floorMode = 'master') {
  let cache = baseStore.get(resource);
  if (!cache) baseStore.set(resource, (cache = new Map()));
  const key = `${id}|${cls}|${cataLevel}|${floorMode}`;
  if (!cache.has(key)) cache.set(key, computeBaseStats(id, resource, cls, cataLevel, floorMode));
  return cache.get(key);
}

function computeBaseStats(id, resource, cls, cataLevel, floorMode) {
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
      if (key && Array.isArray(values) && values.length) stats[key] = (stats[key] ?? 0) + values[floorTierIndex(cataLevel, values.length)];
    }
    // Per-Catacombs-level base stats (Hyperion +1 damage and +2 intelligence per level).
    if (override.perCataLevel && Number.isFinite(cataLevel)) addLoose(stats, override.perCataLevel, cataLevel);
    const boost = dropStatMultiplier(id, resource);
    if (boost !== 1) for (const key of Object.keys(stats)) stats[key] *= boost;
  }
  // Master Mode only bonuses (Master Skulls) do nothing in Normal Mode.
  const modeOk = !(override.masterModeOnly && floorMode === 'normal');
  if (classOk && modeOk) for (const [key, value] of Object.entries(override.stats ?? {})) addLoose(stats, { [key]: value });
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

// Every dungeon item is assumed to be a regular 5-starred item.
// Crit chance, attack speed and ability damage get the star part only (1.5 + master stars + medallion), no Catacombs boost.
// Speed gets the full dungeon multiplier like strength.
const STAR_ONLY_STATS = new Set(['critChance', 'attackSpeed', 'abilityDamage', 'ferocity']);

function scaleBucket(stats, item, ctx, dungeon) {
  if (!dungeon) return stats;
  const stars = MAX_STARS;
  // Master stars only work in Master Mode.
  const masterStars = ctx.floorMode === 'normal' ? 0 : item.masterStars ?? 0;
  const flat = dungeonMultiplier(ctx.cataLevel, stars, masterStars, ctx.secretDigit ?? 0);
  const crit = dungeonCritMultiplier(stars, masterStars, ctx.secretDigit ?? 0);
  const out = {};
  for (const [key, value] of Object.entries(stats)) {
    if (STAR_ONLY_STATS.has(key)) out[key] = value * crit;
    else if (FLAT_STATS.has(key) || key === 'speed') out[key] = value * flat;
    else out[key] = value;
  }
  return out;
}

// Unscaled stats of one item, split by target.
export function itemRawStats(item, ctx, opts) {
  // Chimera on a Ragnarock depends on the pets, so only the plain call is cached.
  if (opts?.chimeraPetStrength) return computeItemRawStats(item, ctx, opts);
  return memo(rawStore, item, itemKey(ctx), () => computeItemRawStats(item, ctx, {}));
}

function computeItemRawStats(item, ctx, { chimeraPetStrength = 0 } = {}) {
  const base = baseItemStats(item.id, ctx.resource, ctx.cls, ctx.cataLevel, ctx.floorMode);
  const out = { global: { ...base.global }, wither: { ...base.wither }, dragon: { ...base.dragon } };
  const rarity = statRarity(itemRarity(item));
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
    if (ench.legion || ench.thunder) continue;
    if (ench.theOne) {
      addLoose(out.global, { strength: (ench.theOne[Math.min(level, ench.theOne.length) - 1] ?? 0) * maxedCollections(ctx) });
      continue;
    }
    // Master Mode / Normal Mode enchants (Titan Killer, Triple-Strike vs First Strike). The clear set has its own
    // list (RCM_ENCHANTS) that works on both floors.
    if (ench.floorMode && !isRcm(ctx) && ench.floorMode !== (ctx.floorMode ?? 'master')) continue;
    const enemies = ench.swarm ? (isRcm(ctx) ? ctx.swarmEnemies ?? RCM_SWARM_ENEMIES : BOSS_SWARM_ENEMIES) : 1;
    for (const [stat, values] of Object.entries(ench.stats ?? {})) {
      const value = values[Math.min(level, values.length) - 1] * enemies;
      addLoose(out[ench.target], { [stat]: value });
      // Mage RCM: only Giant Killer, Prosecute, Swarm and Smite raise Wither Impact damage (Master and Normal Mode).
      if (stat === 'additiveDamage' && weapon && isRcm(ctx) && RCM_ENCHANTS.includes(name)) {
        addLoose(out.global, { magicAdditive: value });
      }
    }
  }
  // Items like Necron heads multiply everything on them (base, reforge, gems, enchants).
  const m = ITEM_STATS[item.id]?.statMultiplier;
  if (m) for (const bucket of Object.values(out)) for (const k of Object.keys(bucket)) if (FLAT_STATS.has(k) || k === 'speed') bucket[k] *= m;
  return out;
}

// Stats of one equipped item after dungeon scaling.
export function itemContribution(item, ctx, opts) {
  if (opts?.chimeraPetStrength) return computeContribution(item, ctx, opts);
  return memo(contribStore, item, itemKey(ctx), () => computeContribution(item, ctx));
}

function computeContribution(item, ctx, opts) {
  const raw = itemRawStats(item, ctx, opts);
  const dungeon = isDungeon(item, ctx.resource);
  return { global: scaleBucket(raw.global, item, ctx, dungeon), wither: scaleBucket(raw.wither, item, ctx, dungeon), dragon: scaleBucket(raw.dragon, item, ctx, dungeon) };
}

// ---------- Accessories ----------

// Accessory families. NEU lists each family as upgrade chains (BADGE -> [RING, ARTIFACT, RELIC]); some links are
// missing (no ARTIFACT -> [RELIC]) and some groups are mutual (Abiphone cases), so every item in any chain
// together is one family. Hatcessories are one family too. Only the best owned member of a family counts.
const ACCESSORY_FAMILY = new Map();
const CHAIN_POSITION = new Map();
(() => {
  const parent = new Map();
  const find = (x) => {
    if (!parent.has(x)) parent.set(x, x);
    while (parent.get(x) !== x) x = parent.get(x);
    return x;
  };
  for (const [id, ups] of Object.entries(ACCESSORY_DATA.upgrades)) {
    const chain = [id, ...ups];
    chain.forEach((member, i) => {
      CHAIN_POSITION.set(member, Math.max(CHAIN_POSITION.get(member) ?? 0, i));
      parent.set(find(member), find(id));
    });
  }
  for (const id of parent.keys()) ACCESSORY_FAMILY.set(id, find(id));
})();

export function isHatcessory(id) {
  return HATCESSORY.test(id);
}

// Position in its accessory upgrade chain (0 for the first tier or no chain).
export function accessoryChainPosition(id) {
  return CHAIN_POSITION.get(id) ?? 0;
}

export function accessoryFamily(id) {
  return isHatcessory(id) ? 'HATCESSORY' : ACCESSORY_FAMILY.get(id) ?? null;
}

// Higher is better: rarity first, then position in the upgrade chain.
function familyRank(rarity, id) {
  return statRarityIndex(rarity) * 100 + (CHAIN_POSITION.get(id) ?? 0);
}

// Higher is better within a family (by base rarity, then chain position).
export function accessoryFamilyRank(id, resource) {
  return familyRank(dropBaseRarity(id, resource), id);
}

// True when an owned accessory of the same family is at least as good (by base rarity when the resource is
// given, otherwise by chain position), so this one would add nothing.
export function accessorySuperseded(id, accessoryIds, resource) {
  const family = accessoryFamily(id);
  if (!family) return false;
  const rank = (x) => (resource ? familyRank(dropBaseRarity(x, resource), x) : CHAIN_POSITION.get(x) ?? 0);
  return [...accessoryIds].some((other) => other !== id && accessoryFamily(other) === family && rank(other) >= rank(id));
}

export function countedAccessories(loadout) {
  const out = [];
  const best = new Map();
  for (const item of loadout.accessories.values()) {
    if (IGNORED_ACCESSORIES.has(item.id)) continue;
    const family = accessoryFamily(item.id);
    if (!family) {
      out.push(item);
      continue;
    }
    const prev = best.get(family);
    if (!prev || itemFamilyRank(item) > itemFamilyRank(prev)) best.set(family, item);
  }
  return [...out, ...best.values()];
}

// Equal members (all hatcessories are equal) go to the enriched one, so an enriched Crab Hat in the bag beats a bare
// Balloon Hat in the ender chest.
function itemFamilyRank(item) {
  return (familyRank(itemRarity(item), item.id) * 2 + (item.recomb ? 1 : 0)) * 2 + (item.enrichment ? 1 : 0);
}

function statRarityIndex(rarity) {
  return RARITIES.indexOf(statRarity(rarity));
}

export function accessoryPower(item, resource) {
  return memo(apStore, item, resource, () => computeAccessoryPower(item, resource));
}

function computeAccessoryPower(item, resource) {
  if (AP_OVERRIDES[item.id] !== undefined) return AP_OVERRIDES[item.id];
  let ap = AP_BY_RARITY[itemRarity(item)] ?? 0;
  if (resource.get(item.id)?.dungeon_item) ap *= DUNGEON_AP_MULTIPLIER;
  if (DOUBLE_AP_ITEMS.includes(item.id)) ap *= 2;
  return ap;
}

export function totalAccessoryPower(loadout, resource) {
  return countedAccessories(loadout).reduce((sum, item) => sum + accessoryPower(item, resource), 0);
}

// Leftover tuning points go to the DPS stats by the profile's split, or the class default.
// Never speed or attack speed: both are already at their cap when points are left over.
function distribute(stats, points, ctx) {
  if (!(points > 0)) return;
  // Only the profile's tuning stats that matter for this class (an Archer split doesn't carry over to Mage).
  const useful = tuningStats(ctx);
  const entries = Object.entries(ctx.tuning?.split ?? {}).filter(([stat]) => useful.includes(stat) && TUNING_PER_POINT[stat]);
  const total = entries.reduce((sum, [, share]) => sum + share, 0);
  const fallback = isRcm(ctx) ? { intelligence: 1 } : DEFAULT_TUNING_SPLIT[ctx.cls];
  const split = total ? entries.map(([stat, share]) => [stat, share / total]) : Object.entries(fallback);
  for (const [stat, share] of split) addLoose(stats, { [stat]: points * share * (TUNING_PER_POINT[stat] ?? 0) });
}

// Tuning order: attack speed up to the cap, then speed up to the cap, then DPS stats.
// speed / attackSpeed: what the player has from everything except tuning (speed as on the stat sheet).
// asMult: percent boost on attack speed, which also raises what each tuning point gives.
export function tuningAllocation(points, speed, ctx, attackSpeed = ATTACK_SPEED_CAP, asMult = 1) {
  const asPoints = Math.min(points, Math.max(0, Math.ceil((ATTACK_SPEED_CAP - attackSpeed) / (TUNING_PER_POINT.attackSpeed * asMult) - 1e-9)));
  const left = points - asPoints;
  // The cap is dungeon speed (over 100 counts for 2/3), so points fill the stat sheet up to sheetSpeedFor(cap).
  const target = sheetSpeedFor(speedCap(ctx?.cls, ctx?.mode) + (ctx?.speedCapBonus ?? 0));
  const speedPoints = Math.min(left, Math.max(0, Math.ceil((target - speed) / TUNING_PER_POINT.speed - 1e-9)));
  const stats = { attackSpeed: asPoints * TUNING_PER_POINT.attackSpeed, speed: speedPoints * TUNING_PER_POINT.speed };
  distribute(stats, left - speedPoints, ctx);
  return { stats, attackSpeedPoints: asPoints, speedPoints, dpsPoints: left - speedPoints };
}

// Accessory bag: own stats, Accessory Power, power stats, enrichments, and how many tuning points it gives.
// One accessory's stats with its enrichment. Dungeon accessories get x (1 + cata boost) on every stat except
// crit chance and attack speed (no stars or medallion on accessories).
export function accessoryItemStats(item, ctx) {
  return memo(accStore, item, itemKey(ctx), () => computeAccessoryStats(item, ctx));
}

function computeAccessoryStats(item, ctx) {
  const stats = { ...itemRawStats(item, ctx).global };
  const enrich = item.enrichment && ENRICHMENTS[item.enrichment];
  if (enrich) addLoose(stats, enrich.stats);
  if (!ctx.resource?.get(item.id)?.dungeon_item) return stats;
  const boost = 1 + cataBoost(ctx.cataLevel ?? 0);
  for (const key of Object.keys(stats)) if ((FLAT_STATS.has(key) || key === 'speed') && !STAR_ONLY_STATS.has(key)) stats[key] *= boost;
  return stats;
}

// Bags are shared between loadouts until a step changes them, so the totals are cached per bag.
const bagStore = new WeakMap();
export function accessoryStats(loadout, ctx, extraTuningPoints = 0) {
  return memo(bagStore, loadout.accessories, `${itemKey(ctx)}|${loadout.power ?? ctx.power}|${extraTuningPoints}`, () => computeBagStats(loadout, ctx, extraTuningPoints));
}

function computeBagStats(loadout, ctx, extraTuningPoints) {
  const counted = countedAccessories(loadout);
  const stats = {};
  let ap = 0;
  for (const item of counted) {
    ap += accessoryPower(item, ctx.resource);
    addLoose(stats, accessoryItemStats(item, ctx));
  }
  const power = POWERS[loadout.power ?? ctx.power] ?? POWERS[DEFAULT_POWER[ctx.cls]];
  const mult = powerMultiplier(ap);
  addLoose(stats, power.stats, mult);
  addLoose(stats, power.bonus);
  const points = Math.floor(ap / AP_PER_TUNING_POINT) + extraTuningPoints;
  return { stats, ap, tuningPoints: points };
}

// ---------- Shards ----------

export function shardLevel(loadout, id) {
  return Math.max(0, Math.min(10, loadout.shards?.[id] ?? 0));
}

// Shard levels are shared between loadouts until a step changes them, so totals are cached per shards object.
const shardStore = new WeakMap();
export function shardStats(loadout, ctx) {
  return memo(shardStore, loadout.shards, ctx.cls, () => computeShardStats(loadout, ctx));
}

function computeShardStats(loadout, ctx) {
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

// Best Jellyfish buff among owned pets (it stays on the Dungeon potion after swapping pets).
function potionBoost(loadout, ctx) {
  let best = 0;
  for (const pet of loadout.pets ?? []) {
    if (pet.type !== 'JELLYFISH') continue;
    best = Math.max(best, petStats(pet, petProfileOf(loadout, ctx)).potionBoost);
  }
  return best;
}

function setBonuses(loadout, cls) {
  const ids = Object.values(loadout.armor).filter(Boolean).map((i) => i.id);
  const out = {};
  for (const s of ARMOR_SETS) {
    if (s.classes && !s.classes.includes(cls)) continue;
    const n = ids.filter((id) => s.pieces.includes(id)).length;
    if (!n) continue;
    addLoose(out, s.perPiece, n);
    if (n >= 4) addLoose(out, s.full);
  }
  return out;
}

// ---------- Whole loadout ----------

export function loadoutGear(loadout) {
  return [loadout.weapon, ...Object.values(loadout.armor), ...Object.values(loadout.equipment)].filter(Boolean);
}

function secretDigitFor(loadout, ctx) {
  return loadout.accessories.has(GENERAL_MEDALLION_ID) || loadout.owned.has(GENERAL_MEDALLION_ID) ? secretDigits(ctx.secrets) : 0;
}

// Raw stat buckets for each target with a fixed set of active extras. With a dragon set, each target gets the
// buckets of its own gear, with its own tuning (points are re-spent when swapping sets).
function rawStats(loadout, ctx, activeExtras) {
  const c = { ...ctx, secretDigit: secretDigitFor(loadout, ctx) };
  const pre = bucketsBeforeTuning(loadout, c, activeExtras);
  const view = gearView(loadout, 'dragon');
  if (view === loadout) return tuned(pre, loadout, c, TARGETS);
  const w = tuned(pre, loadout, c, ['wither']);
  tuned(pre, view, c, ['dragon']);
  return w;
}

// Spend one set's tuning points (attack speed for whichever of `targets` needs more, then speed, then DPS stats)
// and add them to its buckets.
function tuned({ buckets, acc, pets, speeds }, loadout, c, targets) {
  // The whole run's tuning is set for the withers' pet; a dragon set's for the dragon pet.
  const speedTarget = targets.length === 1 ? targets[0] : 'wither';
  const speedBeforeTuning = speeds[speedTarget];
  const asNeed = (b) => (ATTACK_SPEED_CAP - attackSpeedOf(b)) / (TUNING_PER_POINT.attackSpeed * (1 + b.additiveAttackSpeed));
  // F7 has no dragons, so the dragon pet's attack speed never sets the points.
  const fought = c.floorMode === 'normal' && targets.length > 1 ? targets.filter((t) => t !== 'dragon') : targets;
  const binding = fought.reduce((a, t) => (asNeed(buckets[t]) > asNeed(buckets[a]) ? t : a), fought[0]);
  // Attack speed only matters for shortbows (shots per second) and the mage beam; otherwise no points go to it.
  const needsAttackSpeed = (c.cls === 'mage' && !isRcm(c)) || Boolean(ITEM_STATS[loadout.weapon?.id]?.shotTicks);
  // Black Cat raises the speed cap, so the speed target goes up with it.
  const speedCapBonus = pets[speedTarget]?.special?.speedCapBonus ?? 0;
  const tuning = tuningAllocation(acc.tuningPoints, speedBeforeTuning, { ...c, speedCapBonus },
    needsAttackSpeed ? attackSpeedOf(buckets[binding]) : ATTACK_SPEED_CAP, 1 + buckets[binding].additiveAttackSpeed);
  for (const t of targets) addInto(buckets[t], tuning.stats);
  return { buckets, ap: acc.ap, speed: speedBeforeTuning + tuning.stats.speed, tuning };
}

// Stat buckets for each target before tuning points, each with the gear used against it (gearView). c: the
// context with its secret digit.
function bucketsBeforeTuning(loadout, c, activeExtras) {
  const gear = { wither: gearView(loadout, 'wither'), dragon: gearView(loadout, 'dragon') };
  const buckets = { wither: emptyBucket(), dragon: emptyBucket() };
  const add = (stats, target, scale = 1) => {
    if (!target || target === 'global') for (const t of TARGETS) addInto(buckets[t], stats, scale);
    else addInto(buckets[target], stats, scale);
  };
  // Bonuses against a boss only (wither or dragon). RCM is scored on clear mobs, so they do not count there.
  const rcm = isRcm(c);
  const bossOnly = (stats, target) => { if (!rcm) add(stats, target); };
  const misc = MISC_BUFFS[c.cls] ?? {};
  add(misc.stats);
  // Mage class intelligence (solo): 2 x 250 + 5 per Mage level.
  if (c.cls === 'mage') add({ intelligence: SOLO_CLASS_MULTIPLIER * MAGE_INT_BASE + MAGE_INT_PER_LEVEL * (c.mageLevel ?? 0) });
  add(profileStatTotals(c));
  // Arrow base damage, added to the bow's (Nansorb for Archer, Bouncy for Berserker).
  for (const t of TARGETS) if (isBow(gear[t].weapon)) add({ damage: ARROW_DAMAGE[c.cls] ?? 0 }, t);
  // Combat skill: +0.5 base crit chance per level.
  add({ critChance: (c.combatLevel ?? 0) * COMBAT_CRIT_CHANCE_PER_LEVEL });
  // Mage class ability damage (+5 doubled solo, +1 per 5 levels) and Enchanting (+0.5% per level). RCM only uses it.
  if (c.cls === 'mage') add({ abilityDamage: SOLO_CLASS_MULTIPLIER * MAGE_ABILITY_DAMAGE_BASE + MAGE_ABILITY_DAMAGE_PER_5_LEVELS * Math.floor((c.mageLevel ?? 0) / 5) });
  if (c.cls === 'mage') add({ abilityDamage: ENCHANTING_ABILITY_DAMAGE_PER_LEVEL * (c.player?.skills?.ENCHANTING ?? ENCHANTING_MAX_LEVEL) });
  bossOnly(misc.wither, 'wither');
  bossOnly(misc.dragon, 'dragon');

  for (const t of TARGETS) {
    const g = gear[t];
    for (const item of loadoutGear(g)) {
      const contrib = itemContribution(item, c);
      add(contrib.global, t);
      bossOnly(contrib[t], t);
    }
    // Dual wield: Prosecute copy early, Execute copy late.
    if (g.weapon?.dualWield && !rcm) add({ additiveDamage: DUAL_WIELD_ADDITIVE }, t);
    // Legion: percent of combat stats per piece of armor, per nearby player (4 against withers, and against dragons
    // for Mage; 1 otherwise).
    let legion = 0;
    for (const item of Object.values(g.armor)) legion += (item?.enchants?.ultimate_legion ?? 0) * LEGION_PER_LEVEL;
    if (legion) {
      const grouped = !rcm && (t === 'wither' || c.cls === 'mage');
      const l = legion * (grouped ? LEGION_GROUPED_PLAYERS : LEGION_PLAYERS);
      add({ additiveStrength: l, additiveCritDamage: l, additiveIntelligence: l, additiveAttackSpeed: l, additiveCritChance: l }, t);
    }
    add(setBonuses(g, c.cls), t);
  }

  const extrasBucket = { global: {}, wither: {}, dragon: {} };
  const activeKeys = new Set(activeExtras.map((e) => e.key));
  extraStats(activeExtras, activeKeys, extrasBucket);
  add(extrasBucket.global);
  bossOnly(extrasBucket.wither, 'wither');
  bossOnly(extrasBucket.dragon, 'dragon');

  // Active pet for each boss (swapped between phases) and the Dungeon potion, buffed by your best Jellyfish.
  const pets = {};
  for (const t of TARGETS) {
    const active = loadout.activePets?.[t];
    pets[t] = active ? petStats(active, petProfileOf(loadout, c)) : null;
    if (!pets[t]) continue;
    add(pets[t].stats, t);
    bossOnly(pets[t][t], t);
    // Pet damage perks (Golden Dragon's Legendary Treasure) also raise ability damage.
    add({ magicAdditive: pets[t].stats.additiveDamage ?? 0 }, t);
    const sp = pets[t].special ?? {};
    const weapon = gear[t].weapon;
    // Skeleton Combo strength only builds with a bow.
    if (sp.comboStrength && isBow(weapon)) add({ strength: sp.comboStrength }, t);
    if (sp.perkStrength) add({ strength: sp.perkStrength }, t);
    if (sp.perkSpeed) add({ speed: sp.perkSpeed }, t);
    // Lion Primal Force: stats on the weapon, scaled like the weapon's own.
    if (sp.weaponStats && weapon) add(scaleBucket(sp.weaponStats, weapon, c, isDungeon(weapon, c.resource)), t);
    // Lion First Pounce: First Strike / Triple-Strike stronger (melee, so not RCM).
    if (sp.firstStrikeBoost && weapon && !rcm) {
      for (const key of ['first_strike', 'triple_strike']) {
        const ench = ENCHANTS[key];
        const level = weapon.enchants?.[key] ?? 0;
        if (!level || ench.floorMode !== (c.floorMode ?? 'master')) continue;
        add({ additiveDamage: ench.stats.additiveDamage[Math.min(level, 5) - 1] * sp.firstStrikeBoost }, t);
      }
    }
    // Blaze: potato books on the weapon count twice (the extra copy, dungeon scaled like the weapon).
    const potatoBoost = (sp.potatoMultiplier ?? 1) - 1;
    if (potatoBoost > 0 && weapon) {
      const books = Math.min(weapon.hpb ?? 0, HOT_POTATO.max + FUMING_POTATO.max) * potatoBoost;
      add(scaleBucket({ damage: books * HOT_POTATO.damage, strength: books * HOT_POTATO.strength }, weapon, c, isDungeon(weapon, c.resource)), t);
    }
  }
  // Chimera on the weapon (not a Ragnarock): copies the active pet's stats, which then get the weapon's
  // dungeon boost (stars and cata; crit chance and attack speed stars only). Big for Mage: Golden Dragon's
  // attack speed and strength.
  for (const t of TARGETS) {
    const weapon = gear[t].weapon;
    const chimera = weapon?.enchants?.ultimate_chimera ?? 0;
    if (chimera && weapon.id !== RAGNAROCK_WEAPON_ID && pets[t]) {
      const dungeon = isDungeon(weapon, c.resource);
      const copy = {};
      for (const [k, v] of Object.entries(pets[t].stats)) {
        if (typeof v === 'number' && (FLAT_STATS.has(k) || k === 'speed' || k === 'attackSpeed' || k === 'abilityDamage')) copy[k] = v * CHIMERA_PER_LEVEL * chimera;
      }
      add(scaleBucket(copy, weapon, c, dungeon), t);
    }
  }
  add(DUNGEON_POTION, null, 1 + potionBoost(loadout, c));

  // Ragnarock: the ability is cast with whichever owned pet has the most strength (swap, cast, swap back),
  // so Chimera copies that pet's strength in every phase.
  if (loadout.rag) {
    const ragPetStrength = Math.max(0, ...(loadout.pets ?? []).map((p) => petStats(p, petProfileOf(loadout, c)).petStrength ?? 0));
    const rag = itemRawStats(loadout.rag, c, { chimeraPetStrength: ragPetStrength });
    add({ strength: RAGNAROCK_MULTIPLIER * (rag.global.strength ?? 0) });
  }

  // Essence shop perks.
  for (const [key, perk] of Object.entries(ESSENCE_PERKS)) {
    const level = loadout.perks?.[key] ?? 0;
    if (!level) continue;
    const { petCritDamage, ...stats } = perk.perLevel;
    add(Object.fromEntries(Object.entries(stats).map(([k, v]) => [k, v * level])));
    if (petCritDamage) {
      for (const t of TARGETS) {
        if (loadout.activePets?.[t]?.type === perk.pet) add({ critDamage: (pets[t].stats.critDamage ?? 0) * petCritDamage * level }, t);
      }
    }
  }

  const shards = shardStats(loadout, c);
  add(shards.global);
  bossOnly(shards.wither, 'wither');
  bossOnly(shards.dragon, 'dragon');

  const acc = accessoryStats(loadout, c, shards.tuningPoints);
  add(acc.stats);
  for (const t of TARGETS) if (isBow(gear[t].weapon)) buckets[t].additiveDamage += buckets[t].bowDamage;
  // Speed only has to hit the cap during the clear, so faster clear boots count for speed.
  const bootsSpeed = (item) => (item ? itemContribution(item, c).global.speed ?? 0 : 0);
  // Mage is scored in the boss only (RCM armor for the clear), so its boss boots carry the speed.
  const clearSwap = CLEAR_BOOTS_CLASSES.includes(c.cls) && !rcm ? Math.max(0, bootsSpeed(loadout.clearBoots) - bootsSpeed(loadout.armor.BOOTS)) : 0;
  // Speed per target (a pet's speed perk only counts while it is out).
  const speeds = Object.fromEntries(TARGETS.map((t) => [t, BASE_SPEED + EXTRA_SPEED + (c.profileSpeed ?? 0) + buckets[t].speed + clearSwap]));
  return { buckets, acc, pets, speeds };
}

// Final stats after the strength / crit damage / intelligence / crit chance formulas.
// Class and Combat additive damage: Warrior (Combat level) for Archer and Berserker, plus the Berserk class
// bonus and Lust for Blood. Classes are assumed solo.
export function classAdditiveDamage(ctx) {
  const combat = ctx.combatLevel ?? 0;
  const warrior = WARRIOR_PER_LEVEL * Math.min(combat, 50) + WARRIOR_PER_LEVEL_AFTER_50 * Math.max(0, combat - 50);
  const bersLevel = ctx.berserkLevel ?? 0;
  const berserk = ctx.cls === 'bers' ? SOLO_CLASS_MULTIPLIER * BERSERK_DAMAGE_BASE + (BERSERK_DAMAGE_PER_LEVEL + LUST_FOR_BLOOD_PER_LEVEL) * bersLevel : 0;
  return warrior + berserk;
}

// Berserk melee bonus as a multiplier (40 doubled solo + 0.75 per level): ferocity hits only.
export function berserkMeleeMultiplier(ctx) {
  return 1 + (SOLO_CLASS_MULTIPLIER * BERSERK_DAMAGE_BASE + BERSERK_DAMAGE_PER_LEVEL * (ctx.berserkLevel ?? 0)) / 100;
}

// Archer class arrow damage: a multiplicative buff from Archer level (solo).
export function archerMultiplier(ctx) {
  return ctx.cls === 'arch' ? 1 + (SOLO_CLASS_MULTIPLIER * ARCHER_ARROW_DAMAGE_PER_LEVEL * (ctx.archerLevel ?? 0)) / 100 : 1;
}

export function finalStats(bucket, ctx) {
  const effect = blessingEffect(bucket.blessing, bucket.blessingShard);
  return {
    damage: calcDamage(bucket.damage),
    strength: calcStrength(bucket.strength, bucket.additiveStrength, bucket.additiveStrength2, bucket.multStrength, effect, ctx.blessings),
    critDamage: calcCritDamage(bucket.critDamage, bucket.additiveCritDamage, bucket.additiveCritDamage2, bucket.multCritDamage, effect, ctx.blessings),
    intelligence: calcIntelligence(bucket.intelligence, bucket.additiveIntelligence, bucket.additiveIntelligence2, bucket.multIntelligence, effect, ctx.blessings),
    critChance: calcCritChance(bucket.critChance, bucket.additiveCritChance, bucket.multCritChance),
    additiveDamage: bucket.additiveDamage + classAdditiveDamage(ctx),
    abilityDamage: bucket.abilityDamage,
  };
}

// Book of Progression set to damage (Extra Infliction, SkyBlock level 75+): multiplicative, +0.01% per SkyBlock
// level up to 500. The option comes from the profile (loadout.bopBonus) and can be switched as an upgrade step.
export function bookMultiplier(loadout, ctx) {
  if (!loadout.accessories?.has(BOOK_OF_PROGRESSION_ID) || !ctx.profileStats) return 1;
  if (loadout.bopBonus !== BOOK_DAMAGE_OPTION || (ctx.profileStats.sbLevel ?? 0) < BOOK_DAMAGE_UNLOCK_LEVEL) return 1;
  return 1 + BOOK_DAMAGE_PER_LEVEL * Math.min(ctx.profileStats.sbLevel ?? 0, BOOK_MAX_LEVEL);
}

function fireAspectPercent(weapon) {
  const level = weapon?.enchants?.fire_aspect ?? 0;
  const ench = ENCHANTS.fire_aspect;
  if (!level || !ench.categories.includes(weapon.category)) return 0;
  return ench.fire[Math.min(level, ench.fire.length) - 1];
}

function firstStrikeAdditive(weapon) {
  const level = weapon?.enchants?.first_strike ?? 0;
  const ench = ENCHANTS.first_strike;
  if (!level || !ench.categories.includes(weapon.category)) return 0;
  return ench.stats.additiveDamage[Math.min(level, ench.stats.additiveDamage.length) - 1];
}

function thunderlordPercent(weapon) {
  const level = weapon?.enchants?.thunderlord ?? 0;
  const ench = ENCHANTS.thunderlord;
  if (!level || !ench.categories.includes(weapon.category)) return 0;
  return ench.thunder[Math.min(level, ench.thunder.length) - 1];
}

// Necron's Blade family: +50% vs withers, only while held.
function witherBladeMultiplier(loadout, target) {
  return target === 'wither' && WITHER_IMPACT_WEAPONS.includes(loadout.weapon?.id) ? NECRON_BLADE_WITHER_MULTIPLIER : 1;
}

function evaluateWith(loadout, ctx, activeExtras) {
  const { buckets, ap, speed, tuning } = rawStats(loadout, ctx, activeExtras);
  const arrows = arrowsPerVolley(loadout.weapon, buckets.wither.arrows, ctx);
  // Attack speed and volleys per target: dragon-only buffs (Weirder Tuba) raise them in P5 only.
  const attackSpeeds = Object.fromEntries(TARGETS.map((t) => [t, attackSpeedOf(buckets[t])]));
  const volleys = Object.fromEntries(TARGETS.map((t) => [t, volleysPerSecond(gearView(loadout, t).weapon, attackSpeeds[t], ctx)]));
  const result = {
    stats: {}, ap, speed, attackSpeed: attackSpeeds.wither, attackSpeeds, volleysPerSecond: volleys.wither, volleys, arrowsPerVolley: arrows,
    tuning: { attackSpeed: tuning.attackSpeedPoints, speed: tuning.speedPoints, dps: tuning.dpsPoints },
  };
  const main = loadout;
  for (const target of TARGETS) {
    // The gear used against this target (Mage LCM can swap to a dragon set).
    const loadout = gearView(main, target);
    const shots = volleys[target];
    const base = finalStats(buckets[target], ctx);
    const defense = TARGET_DEFENSE[target];
    // Fabled: every hit deals 0-15% more (multiplicative), 7.5% on average. Melee hits only, so the beam
    // gets it and RCM does not.
    // Shadow Assassin Cloak while holding a Wither Impact blade (all 3 scrolls): one 1.1x hit per cooldown,
    // averaged over every hit. Any other weapon gets nothing.
    // Necron's Blade family held: +50% vs withers.
    const saCloak = SA_CLOAK_IDS.includes(loadout.equipment?.CLOAK?.id) && hasWitherImpact(loadout.weapon) ? saCloakMultiplier(shots, ctx) : 1;
    const fabled = (loadout.weapon?.reforge === 'fabled' ? FABLED_MULTIPLIER : 1) * saCloak * witherBladeMultiplier(loadout, target) * bookMultiplier(loadout, ctx);
    if (isRcm(ctx)) {
      // One Wither Impact cast against a clear mob (casts per second do not change with gear).
      const blade = hasWitherImpact(loadout.weapon);
      const loving = loadout.armor.CHESTPLATE?.reforge === 'loving' ? LOVING_ABILITY_MULTIPLIER : 1;
      const implosion = loadout.equipment.BELT?.id === IMPLOSION_BELT_ID ? IMPLOSION_BELT_MULTIPLIER : 1;
      result[target] = blade ? rcmDamage(base, 0, cataBoost(ctx.cataLevel), buckets[target].magicAdditive + classAdditiveDamage(ctx), loving * implosion * bookMultiplier(loadout, ctx)) : 0;
      result.stats[target] = base;
      continue;
    }
    // Boss DPS for a set of final stats.
    const bossDps = (s) => {
      // Crit chance over 100% (overload) only counts with the Overload enchant on the bow.
      const overload = isBow(loadout.weapon) && (loadout.weapon.enchants?.overload ?? 0) > 0;
      const hitWith = (m) => (ctx.cls === 'arch' ? archerDamage(s, defense, archerMultiplier(ctx) * m, overload)
        : ctx.cls === 'bers' ? berserkerDamage(s, defense, 0, m, overload)
          : mageDamage(s, defense, m));
      let dmg = hitWith(fabled);
      // Ferocity (Berserker vs withers only): each 100 ferocity is one extra hit per arrow, a melee hit, so the
      // Berserk melee bonus multiplies it again on top of the additive. Counted at
      // FEROCITY_EFFECTIVENESS (it lands late on a boss that dies fast).
      if (ctx.cls === 'bers' && target === 'wither') {
        const ferocity = buckets[target].ferocity * (1 + buckets[target].additiveFerocity);
        if (ferocity > 0) dmg += FEROCITY_EFFECTIVENESS * (ferocity / 100) * hitWith(fabled * berserkMeleeMultiplier(ctx));
      }
      // Thunderlord: every 3rd hit adds lightning for a % of the hit.
      const thunder = thunderlordPercent(loadout.weapon);
      if (thunder) dmg += hitWith(fabled * thunder / 100) / THUNDERLORD_EVERY;
      // DPS = damage per arrow x arrows per volley x volleys per second.
      let dps = dmg * arrows * shots;
      // Wither Skeleton Death's Touch: one extra hit of 67% every second.
      const pet = loadout.activePets?.[target];
      const deathsTouch = pet ? petStats(pet, petProfileOf(loadout, ctx)).special?.deathsTouch ?? 0 : 0;
      if (deathsTouch) dps += hitWith(fabled * deathsTouch);
      // Fire Aspect (withers only; dragons do not burn): a % of the hit as fire damage every second.
      const fire = target === 'wither' ? fireAspectPercent(loadout.weapon) : 0;
      if (fire) dps += hitWith(fabled * fire / 100);
      return dps;
    };
    let s = base;
    result[target] = bossDps(s);
    // Mage at 1B+ DPS kills things in so few hits that First Strike counts in Master Mode too.
    const firstStrike = ctx.cls === 'mage' && ctx.floorMode !== 'normal' ? firstStrikeAdditive(loadout.weapon) : 0;
    if (firstStrike && result[target] >= MAGE_FIRST_STRIKE_DPS) {
      s = { ...base, additiveDamage: base.additiveDamage + firstStrike };
      result[target] = bossDps(s);
    }
    result.stats[target] = s;
  }
  return result;
}

function dominates(a, b) {
  const keys = (e) => ['stats', 'wither', 'dragon'].flatMap((t) => Object.keys(e[t] ?? {}).map((k) => [t, k]));
  return keys(b).every(([t, k]) => (a[t]?.[k] ?? 0) >= b[t][k]) && keys(a).some(([t, k]) => (a[t][k] ?? 0) > (b[t]?.[k] ?? 0));
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
  for (const all of groups.values()) {
    // A member at least as strong as another on every stat wins without scoring both (Weirder Tuba over Weird).
    const members = all.filter((m) => !all.some((o) => o !== m && dominates(o, m)));
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

// Owned pets that can matter for damage, one per type (highest rarity, keeping a held item).
export function dpsPets(pets, ctx) {
  const byType = new Map();
  const order = ['COMMON', 'UNCOMMON', 'RARE', 'EPIC', 'LEGENDARY', 'MYTHIC'];
  for (const pet of pets ?? []) {
    if (EXCLUDED_PETS.includes(pet.type)) continue;
    const s = petStats(pet, ctx.petProfile);
    const useful = ['strength', 'critDamage', 'critChance', 'intelligence', 'additiveDamage', 'additiveStrength', 'attackSpeed', 'abilityDamage'].some((k) => s.stats[k])
      || Object.keys(s.dragon).length || Object.keys(s.wither).length;
    if (!useful) continue;
    const key = `${pet.type}:${pet.heldItem ?? ''}`;
    const prev = byType.get(key);
    if (!prev || order.indexOf(pet.tier) > order.indexOf(prev.tier)) byType.set(key, pet);
  }
  return [...byType.values()];
}

// Pick the active pet for each boss: the owned pet that does the most damage against it (no fixed pet per boss,
// e.g. Ender Dragon's Superior can beat a Golden Dragon with a small bank against withers).
export function selectPet(loadout, ctx, weights) {
  const candidates = [null, ...dpsPets(loadout.pets, ctx)];
  const activePets = { ...(loadout.activePets ?? {}) };
  for (const target of TARGETS) {
    // RCM scores the clear only, so the dragon pet changes nothing (every pet ties, and ties keep none).
    if (isRcm(ctx) && target === 'dragon') { activePets.dragon = null; continue; }
    // RCM: an owned Golden Dragon is always the clear pet (its bank bonus grows with deposits). Without one, the
    // best clear pet wins.
    const pool = isRcm(ctx) ? candidates.filter((p) => p && p.type === 'GOLDEN_DRAGON') : [];
    let best = null;
    let bestScore = -Infinity;
    for (const pet of pool.length ? pool : candidates) {
      // Weighted over both bosses: tuning is shared, so a pet for one boss can change the other.
      const r = evaluate({ ...loadout, activePets: { ...activePets, [target]: pet } }, ctx, weights);
      // Mage can only use a Tier Boosted Ender Dragon if attack speed still reaches the cap some other way.
      if (ctx.cls === 'mage' && pet?.type === 'ENDER_DRAGON' && pet.heldItem === TIER_BOOST_ID && r.attackSpeeds[target] < ATTACK_SPEED_CAP) continue;
      const score = r.wither * weights.wither + r.dragon * weights.dragon;
      if (score > bestScore) { bestScore = score; best = pet; }
    }
    activePets[target] = best;
  }
  return { ...loadout, activePets };
}

// Strength against each target before tuning and % boosts, after pet items, Chimera and Ragnarock have counted
// the pet's strength (profile row labels).
export function flatStrength(loadout, ctx, weights) {
  const c = { ...ctx, secretDigit: secretDigitFor(loadout, ctx) };
  const { buckets } = bucketsBeforeTuning(loadout, c, activeExtras(loadout, ctx, weights));
  return { wither: buckets.wither.strength, dragon: buckets.dragon.strength };
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
    if (!weights[target]) continue;
    // Going from no damage (e.g. a blade without Wither Impact) to some counts as +100%.
    if (before[target] <= 0) { if (after[target] > 0) total += weights[target]; continue; }
    const pct = ((after[target] - before[target]) / before[target]) * 100;
    total += (pct * weights[target]) / 100;
  }
  return total;
}
