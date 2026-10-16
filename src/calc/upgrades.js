import {
  CLASSES, CLASS_WEAPON_CATEGORIES, ARMOR_SLOTS, EQUIPMENT_SLOTS, ACCESSORY_CATEGORY, MASTER_STAR_IDS,
  MAX_MASTER_STARS, MAX_STARS, DUAL_WIELD_WEAPONS, IMPLOSION_BELT_ID, HOT_POTATO, FUMING_POTATO, ART_OF_WAR, RECOMBOBULATOR_ID, RAGNAROCK_ID,
  MAX_UPGRADES_LISTED, BLACKSMITH_REFORGE_COST, ARMOR_REFORGE, equipmentSlot, MAXOR_BOOTS_ID, NECRON_BOOTS_ID, CLEAR_BOOTS_CLASSES, CLASS_EXCLUDED_ITEMS, WITHER_IMPACT_WEAPONS, WITHER_SCROLLS, SA_CLOAK_IDS, blessingsWith, SKELETON_MASTER_CHEST_CLASSES, SKELETON_MASTER_CHESTS, SKELETON_MASTER_CHEST_CATA,
} from '../config/constants.js';
import { ENCHANTS, enchantMaxLevel, TABLE_ENCHANT_XP, ENCHANT_TABLE_TWO_BOTTLES, GRAND_EXP_BOTTLE_ID, ENCHANT_UPGRADE_ITEMS } from '../config/enchants.js';
import { GEMSTONES, GEM_QUALITIES, UPGRADE_QUALITIES, SLOT_GEMS } from '../config/gemstones.js';
import { ENRICHMENTS, ENRICHABLE_RARITIES, enrichmentKinds, tuningStats, AP_OVERRIDES, RIFT_PRISM_ID, POWERS, POWER_STONES, FREE_POWERS, ENRICHMENT_SWAPPER_ID, DEFAULT_POWER } from '../config/accessories.js';
import { SHARDS, SHARD_MAX_LEVEL, SHARDS_CUMULATIVE, SHARD_HUNTING_REQ } from '../config/shards.js';
import { EXTRA_UPGRADES } from '../config/upgrades.js';
import { ESSENCE_PERKS, ESSENCE_MIN_CATA } from '../config/essence.js';
import FRAGGED from '../data/fragged.json' with { type: 'json' };
import { ITEM_UPGRADES } from '../config/itemUpgrades.js';
import { ITEM_STATS } from '../config/itemStats.js';
import {
  evaluate, weightedGain, loadoutKey, baseItemStats, isDungeon, isWeaponCategory, itemRarity, canRecomb, allReforges,
  getReforge, reforgeAppliesTo, gemStat, accessorySuperseded, accessoryFamily, accessoryChainPosition, hasWitherImpact, statRarity, itemContribution, shardLevel, totalAccessoryPower, selectPet, dpsPets, gearView, isRcm,
} from './stats.js';
import { PET_ITEMS, BUY_RARITIES, TIER_BOOST_ID, KAT_UPGRADES, KAT_LEVEL_DISCOUNT, KAT_FLOWER_ID, GOLDEN_DRAGON_BANK_CAP, GOLD_COLLECTION_DIGITS } from '../config/pets.js';
import { allPetTypes, petTiers, petStats, maxPetLevel, effectiveTier } from '../lib/pets.js';
import { cleanName } from '../lib/nbt.js';
import { dropBaseRarity, isFloorDrop, highestFloor, buyPriceId } from '../lib/drops.js';
import { meetsRequirements } from '../lib/requirements.js';
import { SKILL_BONUSES, SLAYER_BONUSES, SLAYER_MAX_LEVEL, SB_STRENGTH_EVERY, HARP_SONGS, BOOK_OF_PROGRESSION_ID, BOOK_DAMAGE_OPTION, BOOK_DAMAGE_UNLOCK_LEVEL, BOOK_SWAP_COINS_PER_LEVEL } from '../config/profile.js';
import { CATA_STAT_BONUS, COMBAT_MAX_LEVEL, WOLF_SLAYER_SPEED_LEVELS } from '../config/constants.js';

// Items nobody can buy (admin and unobtainable items like the Raygun).
const UNBUYABLE_TIERS = new Set(['UNOBTAINABLE', 'ADMIN']);
// Most coins spent on an armor or equipment piece the plan later replaces (buy + upgrades, kept below this).
export const REPLACED_PIECE_BUDGET = 25_000_000;
// Armor bundles: the best this many single buys per armor slot are mixed into multi-piece steps.
const ARMOR_BUNDLE_SHORTLIST = 3;
// Free items (no coins): offered as a "Claim" step priced at 1 coin, so it ranks first when it helps and can join
// armor bundles.
// Golden Necron Head: free with 50 completions; it then upgrades into the Diamond one (8 Enchanted Diamond Blocks),
// keeping its upgrades.
export const FREE_ITEMS = { GOLD_NECRON_HEAD: 'free with 50 completions' };
// What a free item upgrades into: target id -> { from, parts }. Offered as one step (claim, then upgrade), so the
// Diamond head is never lost once the free Golden one has been swapped out.
const FREE_UPGRADES = Object.fromEntries(Object.keys(FREE_ITEMS).flatMap((from) => (ITEM_UPGRADES[from] ?? []).map((up) => [up.id, { from, parts: up.parts }])));
const REPLAN_ROUNDS = 10;
// A step the plan overwrites soon after (a reforge, enchant level, gem, enrichment, pet item, piece or pet that a
// later step replaces with at most this many coins spent in between, the replacing step included) is skipped,
// unless it alone raises damage by more than REPLACED_SOON_MIN_GAIN percent.
export const REPLACED_SOON_COINS = 10_000_000;
export const REPLACED_SOON_MIN_GAIN = 5;
// Floor-scaled dungeon armor from below floor 10: steps on it (buy included) stay at or under this.
const MAX_FLOOR = 10;
// The Mage clear set always goes Swarm + Golden Dragon: Chimera (which Swarm wipes) and the Crow / Sheep pets
// it would copy are never suggested there.
const RCM_SKIPPED_PETS = ['CROW', 'SHEEP', 'PIGMAN'];
const SCROLL_NAMES = { IMPLOSION_SCROLL: 'Implosion', SHADOW_WARP_SCROLL: 'Shadow Warp', WITHER_SHIELD_SCROLL: 'Wither Shield' };
// Bank deposits for Golden Dragon, 1M at a time.
const BANK_STEP = 1_000_000;
const BANK_LABEL = 'Put 1M coins in the bank (Golden Dragon Legendary Treasure)';
function depositBank(l) {
  return { ...cloneLoadout(l), bank: Math.min(GOLDEN_DRAGON_BANK_CAP, (l.bank ?? 0) + BANK_STEP) };
}

// Stats a pet item has to raise to be worth offering.
const PET_ITEM_STATS = ['strength', 'critDamage', 'critChance', 'intelligence', 'attackSpeed', 'abilityDamage'];
// Learning a power at Maxwell uses up this many of its power stone.
const POWER_STONES_TO_LEARN = 9;
// Mage weapon buys bundled with the current weapon's cheaper enchants: books up to this price each.
const COPY_BOOK_MAX = 10_000_000;
const DPS_STATS = ['damage', 'strength', 'critDamage', 'critChance', 'intelligence', 'speed', 'attackSpeed', 'abilityDamage', 'ferocity'];

// Weights for the run. Choosing a single target puts the full global weight on it.
// Mage RCM (mode 'rcm') is scored on clear mobs only, kept in the "wither" slot.
// Normal Mode (F7, Mage LCM only) has no dragon phase, so withers get all the weight.
export function weightsFor(cls, target, mode, floorMode = 'master') {
  if (cls === 'mage' && mode === 'rcm') return { wither: 100, dragon: 0 };
  if (cls === 'mage' && floorMode === 'normal') return { wither: 100, dragon: 0 };
  if (target === 'wither') return { wither: 100, dragon: 0 };
  if (target === 'dragon') return { wither: 0, dragon: 100 };
  return { ...CLASSES[cls].weights };
}

// Items a class never uses. The Mage boss set also never uses Maxor's Boots: the clear is done in the RCM set, and
// the boss only needs 450 speed (Storm's Boots win).
function excludedItem(id, ctx) {
  if (CLASS_EXCLUDED_ITEMS[ctx.cls]?.test(id)) return true;
  if (SKELETON_MASTER_CHEST_CLASSES.includes(ctx.cls) && (ctx.cataLevel ?? 0) >= SKELETON_MASTER_CHEST_CATA
    && ctx.resource?.get(id)?.category === 'CHESTPLATE' && !SKELETON_MASTER_CHESTS.includes(id)) return true;
  return ctx.cls === 'mage' && ctx.mode !== 'rcm' && /^(STARRED_)?SPEED_WITHER_BOOTS$/.test(id);
}

// Mage LCM (Master Mode) can swap to a second weapon, armor or equipment for the dragons. loadout.dragonGear
// { weapon, armor, equipment } holds only the slots that differ from the main set (null when nothing does).
export function hasDragonSet(ctx) {
  return ctx.cls === 'mage' && ctx.mode !== 'rcm' && ctx.floorMode !== 'normal';
}

const slotGroup = (slot) => (ARMOR_SLOTS.includes(slot) ? 'armor' : 'equipment');
function mainItem(l, slot) {
  return slot === 'weapon' ? l.weapon : l[slotGroup(slot)][slot];
}
function dragonItem(l, slot) {
  const d = l.dragonGear;
  return !d ? null : slot === 'weapon' ? d.weapon : d[slotGroup(slot)][slot] ?? null;
}
// Set (or clear, item null) a dragon slot on a loadout copy. An item the main set already uses is not stored.
function setDragonSlot(l, slot, item) {
  const d = { weapon: l.dragonGear?.weapon ?? null, armor: { ...l.dragonGear?.armor }, equipment: { ...l.dragonGear?.equipment } };
  const value = item && item !== mainItem(l, slot) ? item : null;
  if (slot === 'weapon') d.weapon = value;
  else if (value) d[slotGroup(slot)][slot] = value;
  else delete d[slotGroup(slot)][slot];
  l.dragonGear = d.weapon || Object.keys(d.armor).length || Object.keys(d.equipment).length ? d : null;
}
// Gear worth more against withers than dragons or the other way round (Necron's Blade +50%, Smite, Ender Slayer,
// boss-only stats).
function bossSpecific(item, ctx) {
  if (WITHER_IMPACT_WEAPONS.includes(item.id)) return true;
  const c = itemContribution(item, ctx);
  return Object.values(c.wither).some(Boolean) || Object.values(c.dragon).some(Boolean);
}
function dragonItems(l) {
  const d = l.dragonGear;
  return d ? [d.weapon, ...Object.values(d.armor), ...Object.values(d.equipment)].filter(Boolean) : [];
}

// profile: { cataLevel, secrets, power, tuning } from readMember.
// petProfile: { bank, goldCollection } for Golden Dragon perks (no bank data counts as an empty bank; a missing Gold
// collection counts as maxed).
// collections: { maxed, unmaxed } from readCollections (The One), or null (counts every collection as maxed).
// skills / slayers: the player's levels for item requirement checks (omit to treat every requirement as met).
// floorMode: 'master' (Master Mode: Titan Killer, Triple-Strike) or 'normal' (F7: First Strike, no master stars,
// Master Skulls or dragons). Mage LCM only; everything else is Master Mode.
// mode: Mage only, 'lcm' (boss set, mage beam) or 'rcm' (clear set, Wither Impact, 500 speed).
// upgradeMode: 'all' lists every level (Duplex I, II, ...); 'max' jumps straight to the max level at its cost.
export function makeContext({ cls, cataLevel, secrets = 0, power = null, tuning = null, profileSpeed = 0, petProfile = {}, archerLevel = 0, berserkLevel = 0, mageLevel = 0, combatLevel = 0, skills, slayers, profileStats = null, upgradeMode = 'all', mode = 'lcm', floorMode = 'master', blessings = null, collections = null, resource }) {
  const player = skills !== undefined || slayers !== undefined ? { cataLevel, skills: skills ?? null, slayers: slayers ?? null } : null;
  // The Normal Mode toggle is for Mage only, boss set and clear set (F7 Archer / Berserker damage is scored in Master Mode).
  const floor = cls === 'mage' ? floorMode : 'master';
  return { cls, cataLevel, secrets, power, tuning, profileSpeed, petProfile, archerLevel, berserkLevel, mageLevel, combatLevel, player, profileStats, upgradeMode, mode, floorMode: floor, blessings: blessingsWith(blessings ?? {}), collections, resource };
}

// A clean item bought from the market. Dungeonizable gear is assumed converted and 5-starred.
export function newItem(id, resource) {
  const info = resource.get(id);
  const dungeonizable = Boolean(info?.dungeon_item || info?.dungeon_item_conversion_cost);
  return {
    id, name: cleanName(info?.name ?? id), category: info?.category, baseRarity: info ? dropBaseRarity(id, resource) : 'COMMON', recomb: false,
    stars: dungeonizable ? MAX_STARS : 0, masterStars: 0, dungeon: dungeonizable ? true : undefined,
    enchants: {}, hpb: 0, artOfWar: false, reforge: null, gems: { slots: {}, unlocked: [] }, enrichment: null, scrolls: [],
  };
}

// True when \`fromId\` turns into \`toId\` and keeps its upgrades (fragging, Golden -> Diamond heads,
// Wither armor -> Necron's / Storm's / Maxor's / Goldor's).
export function upgradesInto(fromId, toId) {
  return FRAGGED[fromId]?.id === toId || Boolean(ITEM_UPGRADES[fromId]?.some((u) => u.id === toId));
}

function cloneLoadout(l) {
  return {
    weapon: l.weapon,
    armor: { ...l.armor },
    equipment: { ...l.equipment },
    rag: l.rag,
    clearBoots: l.clearBoots ?? null,
    // Never edited in place (setDragonSlot makes a new one).
    dragonGear: l.dragonGear ?? null,
    // Shared until a step changes the bag (copy on write), so unchanged bags keep their cached stats.
    accessories: l.accessories,
    owned: new Set(l.owned),
    extras: new Set(l.extras),
    // Shared until a step changes them (copy on write).
    shards: l.shards,
    perks: l.perks,
    pets: [...(l.pets ?? [])],
    activePets: { ...(l.activePets ?? {}) },
    power: l.power ?? null,
    bopBonus: l.bopBonus ?? null,
    bank: l.bank ?? 0,
    witherImpact: Boolean(l.witherImpact),
    blades: l.blades ?? [],
    spareBlades: l.spareBlades ?? [],
    rcmBlade: l.rcmBlade ?? null,
    unlockedPowers: new Set(l.unlockedPowers ?? []),
  };
}

function score(loadout, ctx, weights) {
  const r = evaluate(loadout, ctx, weights);
  return r.wither * weights.wither + r.dragon * weights.dragon;
}

// Profile -> starting loadout. Picks the best owned item for each slot (two passes so slots settle).
export function buildLoadout(profile, ctx, weights) {
  const { items = [], pets = [], shards = {}, perks = {} } = profile;
  const weaponCats = CLASS_WEAPON_CATEGORIES[ctx.cls];
  // Pets are assumed level 100 (Golden Dragon 200).
  const ownedPets = pets.map((p) => ({ type: p.type, tier: p.tier, heldItem: p.heldItem ?? null }));
  let loadout = { weapon: null, armor: {}, equipment: {}, rag: null, clearBoots: null, accessories: new Map(), owned: new Set(), extras: new Set(), shards: { ...shards }, perks: { ...perks }, pets: ownedPets, activePets: { wither: null, dragon: null } };
  const bySlot = { weapon: [], rag: [] };
  for (const slot of [...ARMOR_SLOTS, ...EQUIPMENT_SLOTS]) bySlot[slot] = [];

  for (const item of items) {
    loadout.owned.add(item.id);
    if (excludedItem(item.id, ctx)) continue;
    if (item.id === RAGNAROCK_ID) bySlot.rag.push(item);
    if (weaponCats.includes(item.category)) bySlot.weapon.push(item);
    else if (bySlot[equipmentSlot(item.category) ?? item.category]) bySlot[equipmentSlot(item.category) ?? item.category].push(item);
    else if (item.category === ACCESSORY_CATEGORY) {
      const prev = loadout.accessories.get(item.id);
      if (!prev || accessoryRank(item) > accessoryRank(prev)) loadout.accessories.set(item.id, item);
    }
  }

  // Consumed Rift Prism. The profile's highest Accessory Power is the truth: the prism's 11 is only added
  // when the bag comes up exactly 11 short of it (or when the profile gives no Accessory Power at all).
  // A consumed prism is always marked owned so it is never offered.
  const gap = profile.highestAp != null ? profile.highestAp - totalAccessoryPower(loadout, ctx.resource) : null;
  const addPrism = gap === AP_OVERRIDES[RIFT_PRISM_ID] || (gap === null && profile.riftPrism === true);
  if (addPrism && !loadout.accessories.has(RIFT_PRISM_ID)) loadout.accessories.set(RIFT_PRISM_ID, newItem(RIFT_PRISM_ID, ctx.resource));
  if (addPrism || profile.riftPrism) loadout.owned.add(RIFT_PRISM_ID);

  for (const entry of EXTRA_UPGRADES) if (extraOwned(entry, loadout.owned)) loadout.extras.add(entry.key);
  // Necron's Blade variants you own, and whether any has Wither Impact (for the Shadow Assassin Cloak in LCM).
  loadout.blades = items.filter((i) => WITHER_IMPACT_WEAPONS.includes(i.id));
  // Mage LCM with two or more Necron's Blades: the one built for the clear (Ultimate Wise, Giant Killer, Swarm) stays
  // the RCM blade, and the boss set uses another one.
  let rcmBlade = null;
  if (ctx.cls === 'mage' && ctx.mode !== 'rcm' && loadout.blades.length >= 2) {
    const rcm = loadout.blades.reduce((best, b) => (rcmBladeScore(b) > rcmBladeScore(best) ? b : best));
    if (rcmBladeScore(rcm) > 0) {
      rcmBlade = rcm;
      bySlot.weapon = bySlot.weapon.filter((w) => w !== rcm);
    }
  }
  loadout.witherImpact = loadout.blades.some(hasWitherImpact);
  bySlot.BOOTS = bySlot.BOOTS.filter((b) => bossBootsAllowed(loadout.owned, b.id, ctx.cls));
  // Clear boots: your fastest boots, worn during the clear for speed (e.g. Maxor's), swapped out for the boss.
  // Mage clears in RCM armor, so it has none.
  loadout.clearBoots = CLEAR_BOOTS_CLASSES.includes(ctx.cls) && ctx.mode !== 'rcm'
    ? bySlot.BOOTS.reduce((best, b) => (bootsSpeed(b, ctx) > bootsSpeed(best, ctx) ? b : best), null)
    : null;
  loadout = selectPet(loadout, ctx, weights);

  const dragonSet = hasDragonSet(ctx);
  for (let pass = 0; pass < 2; pass++) {
    for (const [slot, options] of Object.entries(bySlot)) {
      let best = null;
      let bestScore = -Infinity;
      for (const option of options) {
        const trial = cloneLoadout(loadout);
        setSlot(trial, slot, option);
        const s = score(trial, ctx, weights);
        if (s > bestScore) { bestScore = s; best = trial; }
        // Mage: the new piece for withers only, keeping the current one for dragons.
        const current = slot === 'rag' ? null : mainItem(loadout, slot);
        if (dragonSet && current && current !== option && !dragonItem(loadout, slot)) {
          const swap = cloneLoadout(loadout);
          setSlot(swap, slot, option);
          setDragonSlot(swap, slot, current);
          const s2 = score(swap, ctx, weights);
          if (s2 > bestScore) { bestScore = s2; best = swap; }
        }
      }
      if (best) loadout = best;
    }
    // Mage: the best dragon piece for each slot (none = the main set's).
    if (!dragonSet) continue;
    for (const [slot, options] of Object.entries(bySlot)) {
      if (slot === 'rag') continue;
      let best = null;
      let bestScore = score(loadout, ctx, weights);
      for (const option of options) {
        const trial = cloneLoadout(loadout);
        setDragonSlot(trial, slot, option);
        const s = score(trial, ctx, weights);
        if (s > bestScore) { bestScore = s; best = trial; }
      }
      if (best) loadout = best;
    }
  }

  loadout = selectPet(loadout, ctx, weights);
  // Blades left over (not the RCM one, not worn): they can become a dual-wield copy or another set's weapon for only
  // the upgrades they are missing.
  loadout.rcmBlade = rcmBlade;
  loadout.spareBlades = items.filter((i) => (WITHER_IMPACT_WEAPONS.includes(i.id) || DUAL_WIELD_WEAPONS.includes(i.id))
    && i !== rcmBlade && i !== loadout.weapon && i !== loadout.dragonGear?.weapon);
  // Powers you have learned are free to switch: use the best one.
  // With no power on the profile (or a test context), the class default (DEFAULT_POWER) is assumed learned.
  const current = ctx.power ?? DEFAULT_POWER[ctx.cls];
  loadout.unlockedPowers = new Set([...FREE_POWERS, ...(profile.unlockedPowers ?? []), current]);
  loadout.power = current;
  loadout.bopBonus = ctx.profileStats?.bopBonus ?? null;
  // Coins in the bank, from the bank API (0 when it is off or there is no bank): Golden Dragon's Legendary Treasure.
  loadout.bank = Number(ctx.petProfile?.bank) || 0;
  loadout = selectPower(loadout, ctx, weights);
  // Tuning is free to change, so leftover points use the best split for this loadout (10% steps over the
  // class's damage stats), not whatever the profile has saved. Stored on ctx for the rest of the run.
  ctx.tuning = { ...(ctx.tuning ?? {}), split: bestTuningSplit(loadout, ctx, weights) };
  return selectPet(loadout, ctx, weights);
}

// Best learned power for the loadout (switching at Maxwell is free).
export function selectPower(loadout, ctx, weights) {
  let best = loadout.power;
  let bestScore = -Infinity;
  for (const power of loadout.unlockedPowers ?? []) {
    if (!POWERS[power]) continue;
    const s = score({ ...loadout, power }, ctx, weights);
    if (s > bestScore) { bestScore = s; best = power; }
  }
  return best === loadout.power ? loadout : { ...loadout, power: best };
}

function bestTuningSplit(loadout, ctx, weights) {
  const stats = tuningStats(ctx);
  let best = null;
  let bestScore = -Infinity;
  // Every split of 10 tenths over the class's tuning stats.
  const walk = (i, left, split) => {
    if (i === stats.length - 1) {
      const full = { ...split, [stats[i]]: left / 10 };
      const r = evaluate(loadout, { ...ctx, tuning: { ...(ctx.tuning ?? {}), split: full } }, weights);
      const score = r.wither * weights.wither + r.dragon * weights.dragon;
      if (score > bestScore + 1e-9) { bestScore = score; best = full; }
      return;
    }
    for (let n = 0; n <= left; n++) walk(i + 1, left - n, { ...split, [stats[i]]: n / 10 });
  };
  if (stats.length) walk(0, 10, {});
  return best;
}

// Archer / Berserker with Maxor's boots owned: the boss boots are Maxor's or Necron's only (no swapping to
// other boots). Mage picks its boss boots freely.
function bossBootsAllowed(owned, id, cls) {
  if (!CLEAR_BOOTS_CLASSES.includes(cls)) return true;
  return !owned.has(MAXOR_BOOTS_ID) || id === MAXOR_BOOTS_ID || id === NECRON_BOOTS_ID;
}

function bootsSpeed(item, ctx) {
  return item ? itemContribution(item, ctx).global.speed ?? 0 : 0;
}

function accessoryRank(item) {
  return (item.recomb ? 2 : 0) + (item.enrichment ? 1 : 0);
}

function extraOwned(entry, ownedItems) {
  return Boolean(entry.owned?.items?.some((id) => ownedItems.has(id)));
}

function setSlot(l, slot, item) {
  if (slot === 'weapon') l.weapon = item;
  else if (slot === 'rag') l.rag = item;
  else if (ARMOR_SLOTS.includes(slot)) l.armor[slot] = item;
  else l.equipment[slot] = item;
  // The dragon set never repeats the main set's piece.
  if (slot !== 'rag' && dragonItem(l, slot) === item) setDragonSlot(l, slot, null);
}

// Stable name for where an item sits, used to group mutually exclusive upgrades.
function slotName(loadout, item) {
  if (loadout.weapon === item) return 'weapon';
  if (loadout.rag === item) return 'rag';
  for (const [slot, it] of Object.entries(loadout.armor)) if (it === item) return slot;
  if (loadout.clearBoots === item) return 'clearBoots';
  for (const [slot, it] of Object.entries(loadout.equipment)) if (it === item) return slot;
  const d = loadout.dragonGear;
  if (d?.weapon === item) return 'dragon:weapon';
  for (const [slot, it] of Object.entries({ ...d?.armor, ...d?.equipment })) if (it === item) return `dragon:${slot}`;
  return item.id;
}

// Replace an item everywhere it is used (a Ragnarock can be both weapon and ability item).
function replaceItem(loadout, oldItem, next) {
  const l = cloneLoadout(loadout);
  if (l.weapon === oldItem) l.weapon = next;
  if (l.rag === oldItem) l.rag = next;
  if (l.clearBoots === oldItem) l.clearBoots = next;
  for (const slot of Object.keys(l.armor)) if (l.armor[slot] === oldItem) l.armor[slot] = next;
  for (const slot of Object.keys(l.equipment)) if (l.equipment[slot] === oldItem) l.equipment[slot] = next;
  if (dragonItems(l).includes(oldItem)) {
    const d = l.dragonGear;
    const swap = (group) => Object.fromEntries(Object.entries(group).map(([k, v]) => [k, v === oldItem ? next : v]));
    l.dragonGear = { weapon: d.weapon === oldItem ? next : d.weapon, armor: swap(d.armor), equipment: swap(d.equipment) };
  }
  if (l.accessories.get(oldItem.id) === oldItem) {
    l.accessories = new Map(l.accessories);
    if (next.id !== oldItem.id) l.accessories.delete(oldItem.id);
    l.accessories.set(next.id, next);
  }
  // A fragged item is a new id the player now owns, so it is never offered as a buy.
  l.owned.add(next.id);
  return l;
}

function upgradableItems(loadout) {
  const set = new Set([loadout.weapon, loadout.rag, loadout.clearBoots, ...Object.values(loadout.armor), ...Object.values(loadout.equipment), ...dragonItems(loadout), ...loadout.accessories.values()]);
  set.delete(null);
  set.delete(undefined);
  return [...set];
}

// cost parts: [{ id, qty }] or [{ coins }] (gem slot unlock fees). Returns { cost, source } or null if unpriced.
function priceParts(parts, prices) {
  let cost = 0;
  const sources = new Set();
  for (const part of parts) {
    if (part.coins) {
      cost += part.coins;
      sources.add('coins');
      continue;
    }
    const p = prices.get(part.id);
    if (!p) return null;
    cost += p.price * (part.qty ?? 1);
    sources.add(p.source);
  }
  return { cost, source: [...sources].join('+') };
}

function hasDpsStats(stats) {
  return DPS_STATS.some((k) => stats?.[k]);
}

function hasUsefulStats(id, resource, cls) {
  const b = baseItemStats(id, resource, cls);
  return [b.global, b.wither, b.dragon].some((s) => Object.values(s).some((v) => v !== 0 && v !== 1));
}

// Cheapest way to raise a combinable enchant from `from` to `to` one level at a time: at each level, add the book of
// the current level (combine) or buy the next level's book. Returns { parts, cost, text } or null if a price is missing.
// Cheapest way to get an enchant to `level` from `current`: its book (bazaar / auction house), the enchanting table
// (Grand Experience Bottles), an upgrade item on the level below (Severed Hand, End Stone Idol), or combining
// ultimate books. Returns { parts, note, stacks } or null when none is priced.
export function enchantParts(key, level, current, prices) {
  const book = (lv) => `ENCHANTMENT_${key.toUpperCase()}_${lv}`;
  const ways = [{ parts: [{ id: book(level) }] }];
  const xp = TABLE_ENCHANT_XP[key]?.[level - 1];
  if (xp != null) ways.push({ parts: [{ id: GRAND_EXP_BOTTLE_ID, qty: xp >= ENCHANT_TABLE_TWO_BOTTLES ? 2 : 1 }], note: 'enchanting table' });
  const upgrade = ENCHANT_UPGRADE_ITEMS[key];
  if (upgrade?.level === level) {
    const below = current >= level - 1 ? { parts: [] } : enchantParts(key, level - 1, current, prices);
    if (below) ways.push({ parts: [...below.parts, { id: upgrade.id }], note: `add ${/^[AEIOU]/.test(upgrade.name) ? 'an' : 'a'} ${upgrade.name}`, stacks: current >= level - 1 });
  }
  if (key.startsWith('ultimate_') && current > 0) {
    const chain = combineParts(book, current, level, prices);
    if (chain) ways.push({ parts: chain.parts, note: chain.text, stacks: true });
  }
  let best = null;
  let bestCost = Infinity;
  for (const way of ways) {
    const cost = priceParts(way.parts, prices)?.cost;
    if (cost != null && cost < bestCost) { best = way; bestCost = cost; }
  }
  return best;
}

function combineParts(book, from, to, prices) {
  const parts = [];
  const combined = [];
  let cost = 0;
  for (let lv = from; lv < to; lv++) {
    const same = prices.get(book(lv))?.price;
    const next = prices.get(book(lv + 1))?.price;
    if (same == null && next == null) return null;
    const combine = same != null && !(next <= same);
    parts.push({ id: book(combine ? lv : lv + 1) });
    cost += combine ? same : next;
    if (combine) combined.push(roman(lv));
  }
  if (!combined.length) return null;
  return { parts, cost, text: `combine: add ${combined.map((r) => `a ${r} book`).join(', ')}` };
}

// Everything on an item that a full copy of it costs (priced parts only): enchant books, potato books, The Art
// of War, master stars, recombobulator, reforge, gems and Wither scrolls.
// How sure we are a blade is the RCM one: Ultimate Wise, Giant Killer and Swarm point to the clear; Titan Killer and
// Ender Slayer point to the boss.
function rcmBladeScore(b) {
  const e = b.enchants ?? {};
  return 3 * Boolean(e.ultimate_wise) + 2 * Boolean(e.giant_killer) + Boolean(e.ultimate_swarm) - 2 * Boolean(e.titan_killer) - Boolean(e.ender_slayer);
}

// A spare blade raised to match `src`: the enchants it lacks (only the levels missing), potato books, master stars,
// The Art of War, recombobulator, reforge and Wither scrolls. Returns { item, parts }.
function convertParts(spare, src, prices) {
  const item = { ...spare, enchants: { ...spare.enchants }, scrolls: [...(spare.scrolls ?? [])] };
  const parts = [];
  for (const [key, level] of Object.entries(src.enchants ?? {})) {
    const cur = item.enchants[key] ?? 0;
    if (cur >= level) continue;
    const way = enchantParts(key, level, cur, prices);
    if (!way) continue;
    parts.push(...way.parts);
    // One ultimate enchant per item: a new one replaces the old (Ultimate Wise -> Chimera).
    if (key.startsWith('ultimate_')) for (const k of Object.keys(item.enchants)) if (k.startsWith('ultimate_')) delete item.enchants[k];
    for (const c of ENCHANTS[key]?.conflicts ?? []) delete item.enchants[c];
    item.enchants[key] = level;
  }
  const hpb = Math.max(spare.hpb ?? 0, src.hpb ?? 0);
  const hot = Math.max(0, Math.min(hpb, HOT_POTATO.max) - Math.min(spare.hpb ?? 0, HOT_POTATO.max));
  const fuming = Math.max(0, Math.max(0, hpb - HOT_POTATO.max) - Math.max(0, (spare.hpb ?? 0) - HOT_POTATO.max));
  if (hot) parts.push({ id: HOT_POTATO.id, qty: hot });
  if (fuming) parts.push({ id: FUMING_POTATO.id, qty: fuming });
  item.hpb = hpb;
  if (src.artOfWar && !spare.artOfWar) { parts.push({ id: ART_OF_WAR.id }); item.artOfWar = true; }
  if ((src.masterStars ?? 0) > (spare.masterStars ?? 0)) {
    parts.push(...MASTER_STAR_IDS.slice(spare.masterStars ?? 0, src.masterStars).map((id) => ({ id })));
    item.masterStars = src.masterStars;
  }
  if (src.recomb && !spare.recomb) { parts.push({ id: RECOMBOBULATOR_ID }); item.recomb = true; }
  if (src.reforge && src.reforge !== spare.reforge && getReforge(src.reforge)) { parts.push(...reforgeCost(getReforge(src.reforge), spare)); item.reforge = src.reforge; }
  for (const x of src.scrolls ?? []) if (!item.scrolls.includes(x)) { parts.push({ id: x }); item.scrolls.push(x); }
  return { item, parts };
}

function copyParts(item, prices) {
  const parts = [
    ...Object.entries(item.enchants ?? {}).flatMap(([k, level]) => enchantParts(k, level, 0, prices)?.parts ?? []),
    ...(item.hpb ? [{ id: HOT_POTATO.id, qty: Math.min(item.hpb, HOT_POTATO.max) }] : []),
    ...(item.hpb > HOT_POTATO.max ? [{ id: FUMING_POTATO.id, qty: item.hpb - HOT_POTATO.max }] : []),
    ...(item.artOfWar ? [{ id: ART_OF_WAR.id }] : []),
    ...MASTER_STAR_IDS.slice(0, item.masterStars ?? 0).map((id) => ({ id })),
    ...(item.recomb ? [{ id: RECOMBOBULATOR_ID }] : []),
    ...(getReforge(item.reforge) ? reforgeCost(getReforge(item.reforge), item) : []),
    ...Object.values(item.gems?.slots ?? {}).map((g) => ({ id: `${g.quality}_${g.gem}_GEM` })),
    ...(item.scrolls ?? []).map((id) => ({ id })),
  ];
  return parts.filter((x) => x.coins || prices.get(x.id));
}

// Every upgrade available from the current loadout. Each one only depends on what the loadout
// already has, so a master star never lands on a missing item and the 5th star never comes before the 4th.
export function availableUpgrades(loadout, ctx, prices) {
  const ups = [];
  const { resource, cls } = ctx;
  const push = (label, parts, apply, extra = {}) => {
    const p = priceParts(parts, prices);
    if (p) ups.push({ label, ...p, apply, ...extra });
  };

  for (const item of upgradableItems(loadout)) {
    const name = cleanName(item.name ?? resource.get(item.id)?.name ?? item.id);
    const slot = slotName(loadout, item);
    const isAccessory = item.category === ACCESSORY_CATEGORY;
    const weaponLike = isWeaponCategory(item.category) && (item === loadout.weapon || item === loadout.rag || item === loadout.dragonGear?.weapon);
    // A dual-wielded weapon has a second copy, so every upgrade on it is bought twice.
    const pushItem = item.dualWield
      ? (label, parts, apply, extra) => push(`${label} (both copies)`, parts.map((x) => (x.coins ? { coins: 2 * x.coins } : { ...x, qty: 2 * (x.qty ?? 1) })), apply, extra)
      : push;

    // Dual wield (LCM): a second copy of a Prosecute Hyperion / Dark Claymore with Execute, swapped in once the boss
    // is low. Priced as the full first copy; counted as DUAL_WIELD_ADDITIVE more additive damage on average.
    if (weaponLike && item !== loadout.rag && DUAL_WIELD_WEAPONS.includes(item.id) && item.enchants?.prosecute && !item.dualWield && !isRcm(ctx)) {
      // A spare copy you own only needs the upgrades it is missing.
      const spare = (loadout.spareBlades ?? []).find((b) => b.id === item.id);
      const parts = spare ? convertParts(spare, item, prices).parts : [{ id: item.id }, ...copyParts(item, prices)];
      push(`Dual wield ${name}: a second copy with Execute in place of Prosecute${spare ? ' (your spare one, missing upgrades only)' : ''}`, parts.length ? parts : [{ coins: 1 }],
        (l) => { const next = replaceItem(l, item, { ...item, dualWield: true }); if (spare) next.spareBlades = next.spareBlades.filter((b) => b !== spare); return next; },
        { group: `${slot}:dualwield` });
    }

    // Master stars: next star only, dungeon gear with 5 regular stars.
    if (!isAccessory && ctx.floorMode !== 'normal' && isDungeon(item, resource) && (item.stars ?? 0) >= MAX_STARS && item.masterStars < MAX_MASTER_STARS) {
      if (ctx.upgradeMode === 'max') {
        const parts = MASTER_STAR_IDS.slice(item.masterStars, MAX_MASTER_STARS).map((id) => ({ id }));
        pushItem(`${parts.length > 1 ? `${ordinal(item.masterStars + 1)}–${ordinal(MAX_MASTER_STARS)} Master Stars` : `${ordinal(MAX_MASTER_STARS)} Master Star`} on ${name}`, parts,
          (l) => replaceItem(l, item, { ...item, masterStars: MAX_MASTER_STARS }));
      } else {
        pushItem(`${ordinal(item.masterStars + 1)} Master Star on ${name}`, [{ id: MASTER_STAR_IDS[item.masterStars] }],
          (l) => replaceItem(l, item, { ...item, masterStars: item.masterStars + 1 }));
      }
    }

    // Enchants: any level above the current one.
    if (!isAccessory) {
      for (const [key, ench] of Object.entries(ENCHANTS)) {
        if (!ench.categories.includes(item.category)) continue;
        if (ench.items && !ench.items.includes(item.id)) continue;
        if (ench.classes && !ench.classes.includes(cls)) continue;
        if (ench.chimera && ctx.mode === 'rcm') continue;
        // First Strike V on a Prosecute weapon stays: never swapped for Triple Strike (IV or less is).
        if (key === 'triple_strike' && (item.enchants?.first_strike ?? 0) >= 5 && item.enchants?.prosecute) continue;
        const current = item.enchants?.[key] ?? 0;
        const maxLevel = enchantMaxLevel(ench);
        const first = Math.max(current + 1, ench.minLevel ?? 1);
        for (let level = ctx.upgradeMode === 'max' ? Math.max(first, maxLevel) : first; level <= maxLevel; level++) {
          // Name the enchant it pushes off (First Strike replaces Triple Strike, Smite replaces Sharpness, ...).
          const replaced = (ench.conflicts ?? []).filter((c) => item.enchants?.[c]).map((c) => ENCHANTS[c]?.name ?? titleCase(c.replace(/^ultimate_/, '')));
          const label = `${ench.name ?? titleCase(key.replace(/^ultimate_/, ''))} ${roman(level)} on ${name}${replaced.length ? ` (replaces ${replaced.join(', ')})` : ''}`;
          const way = enchantParts(key, level, current, prices);
          if (!way) continue;
          pushItem(way.note ? `${label} (${way.note})` : label, way.parts, (l) => {
            const enchants = { ...item.enchants, [key]: level };
            for (const c of ench.conflicts ?? []) delete enchants[c];
            return replaceItem(l, item, { ...item, enchants });
          }, { group: `${slot}:enchant:${key}`, ...(way.stacks ? { stacksOn: `${slot}:enchant:${key}` } : {}) });
        }
      }
    }

    // Hot potato books (all remaining), then fuming potato books, and The Art of War (weapons and Ragnarock).
    if (weaponLike) {
      const hpb = item.hpb ?? 0;
      if (hpb < HOT_POTATO.max) {
        const n = HOT_POTATO.max - hpb;
        pushItem(`${n} Hot Potato Book${n > 1 ? 's' : ''} on ${name}`, [{ id: HOT_POTATO.id, qty: n }],
          (l) => replaceItem(l, item, { ...item, hpb: HOT_POTATO.max }));
      } else if (hpb < HOT_POTATO.max + FUMING_POTATO.max) {
        const n = HOT_POTATO.max + FUMING_POTATO.max - hpb;
        pushItem(`${n} Fuming Potato Book${n > 1 ? 's' : ''} on ${name}`, [{ id: FUMING_POTATO.id, qty: n }],
          (l) => replaceItem(l, item, { ...item, hpb: HOT_POTATO.max + FUMING_POTATO.max }));
      }
      if (!item.artOfWar) pushItem(`The Art of War on ${name}`, [{ id: ART_OF_WAR.id }], (l) => replaceItem(l, item, { ...item, artOfWar: true }));
    }

    // Fragging: base dungeon item + 8 boss fragments -> STARRED_ version (keeps its upgrades).
    const frag = FRAGGED[item.id];
    if (frag && resource.has(frag.id)) {
      const info = resource.get(frag.id);
      pushItem(`Frag ${name} (${frag.parts.map((p) => `${p.qty} ${titleCase(p.id.toLowerCase())}${p.qty > 1 ? 's' : ''}`).join(', ')})`, frag.parts,
        (l) => replaceItem(l, item, { ...item, id: frag.id, name: cleanName(info.name ?? item.name), category: info.category ?? item.category, baseRarity: info.tier ?? item.baseRarity }));
    }

    // Golden -> Diamond heads and Wither armor -> class armor (keeps its upgrades).
    for (const up of ITEM_UPGRADES[item.id] ?? []) {
      const info = resource.get(up.id);
      if (!info || loadout.owned.has(up.id) || excludedItem(up.id, ctx) || !meetsRequirements(info, ctx.player)) continue;
      pushItem(`Upgrade ${name} into ${cleanName(info.name ?? up.id)} (${up.parts.map((p) => `${p.qty} ${titleCase(p.id.toLowerCase())}${p.qty > 1 ? 's' : ''}`).join(', ')})`, up.parts,
        (l) => replaceItem(l, item, { ...item, id: up.id, name: cleanName(info.name ?? item.name), category: info.category ?? item.category, baseRarity: dropBaseRarity(up.id, resource) }));
    }

    // Recombobulator.
    if (canRecomb(item, resource)) pushItem(`Recombobulate ${name}`, [{ id: RECOMBOBULATOR_ID }], (l) => replaceItem(l, item, { ...item, recomb: true }));

    // Reforges: stones, and blacksmith reforges (random rolls, priced as the average number of rolls).
    if (!isAccessory) {
      const rarity = statRarity(itemRarity(item));
      for (const [key, reforge] of Object.entries(allReforges())) {
        if (!reforgeAllowed(key, reforge, item, ctx) || !hasDpsStats(reforge.stats?.[rarity]) && !reforge.cataStat) continue;
        if (getReforge(item.reforge) === reforge) continue;
        pushItem(`${reforge.name} reforge on ${name}${reforge.stone ? '' : ' (blacksmith)'}`, reforgeCost(reforge, item),
          (l) => replaceItem(l, item, { ...item, reforge: key }), { group: `${slot}:reforge` });
      }
    }

    // Gemstones.
    for (const up of gemUpgrades(item, ctx)) {
      pushItem(`${titleCase(up.quality.toLowerCase())} ${titleCase(up.gem.toLowerCase())} in ${name}${up.unlock ? ' (unlock slot)' : ''}`,
        up.parts, (l) => replaceItem(l, item, up.apply(item)), { group: `${slot}:gem:${up.slotKey}` });
    }

    // Enrichments on Legendary+ accessories.
    if (isAccessory && ENRICHABLE_RARITIES.includes(itemRarity(item))) {
      for (const kind of enrichmentKinds(ctx)) {
        if (item.enrichment === kind) continue;
        pushItem(`${enrichmentName(kind)} Enrichment on ${name}`, [{ id: `TALISMAN_ENRICHMENT_${ENRICHMENTS[kind].key}` }],
          (l) => replaceItem(l, item, { ...item, enrichment: kind }), { group: `${slot}:enrich` });
      }
    }
  }

  // Spare blades you own: made into the boss weapon (when it is a different blade) or the Mage dragon-set weapon, for
  // only the upgrades they lack compared with the weapon they copy.
  if (!isRcm(ctx)) {
    for (const spare of loadout.spareBlades ?? []) {
      if (!CLASS_WEAPON_CATEGORIES[cls].includes(spare.category)) continue;
      const spareName = cleanName(spare.name ?? resource.get(spare.id)?.name ?? spare.id);
      const targets = [];
      if (loadout.weapon && loadout.weapon.id !== spare.id) targets.push(['main', loadout.weapon]);
      if (hasDragonSet(ctx) && !loadout.dragonGear?.weapon && loadout.weapon) targets.push(['dragon', loadout.weapon]);
      for (const [set, src] of targets) {
        const { item, parts } = convertParts(spare, src, prices);
        push(`Use your spare ${spareName}${set === 'dragon' ? ' as the dragon-set weapon' : ' as the boss weapon'} with ${cleanName(src.name ?? src.id)}'s upgrades (missing ones only)`,
          parts.length ? parts : [{ coins: 1 }], (l) => {
            const next = cloneLoadout(l);
            next.spareBlades = next.spareBlades.filter((b) => b !== spare);
            if (set === 'dragon') setDragonSlot(next, 'weapon', item);
            else { next.spareBlades = [...next.spareBlades, l.weapon]; next.weapon = item; }
            return next;
          }, { group: `spare:${set}`, repick: true });
      }
    }
  }

  // New items from the auction house / bazaar.
  const weaponCats = CLASS_WEAPON_CATEGORIES[cls];
  const accIds = new Set(loadout.accessories.keys());
  const worn = [loadout.weapon, loadout.rag, ...Object.values(loadout.armor), ...Object.values(loadout.equipment), ...dragonItems(loadout)].filter(Boolean);
  const dragonSet = hasDragonSet(ctx);
  for (const [id, info] of resource) {
    if (loadout.owned.has(id) || UNBUYABLE_TIERS.has(info.tier) || !meetsRequirements(info, ctx.player)) continue;
    if (excludedItem(id, ctx)) continue;
    // Never buy a fresh copy of what an item you wear upgrades into (the upgrade keeps its stars, gems, etc.).
    if (worn.some((it) => upgradesInto(it.id, id))) continue;
    const cat = info.category;
    const isRag = id === RAGNAROCK_ID;
    const isWeapon = weaponCats.includes(cat);
    const isArmor = ARMOR_SLOTS.includes(cat);
    const equipSlot = equipmentSlot(cat);
    const isEquip = Boolean(equipSlot);
    const isAcc = cat === ACCESSORY_CATEGORY;
    if (!isRag && !isWeapon && !isArmor && !isEquip && !isAcc) continue;
    if (isAcc && accessorySuperseded(id, accIds, resource)) continue;
    if (!isAcc && !isRag && !hasUsefulStats(id, resource, cls) && !(id === IMPLOSION_BELT_ID && ctx.mode === 'rcm')) continue;
    if (cat === 'BOOTS' && !bossBootsAllowed(loadout.owned, id, cls)) continue;
    // A buy can come bundled with the changes it would get straight away (reforge, recombobulator, enrichment),
    // priced and scored as one step, so an expensive accessory is weighed with its recomb and enrichment.
    // Mage: gear can also go in the dragon set, or replace the main piece for withers only (the old one kept for
    // dragons).
    const slot = isWeapon && !isRag ? 'weapon' : isArmor ? cat : equipSlot;
    const sets = ['main'];
    // Swapping only pays when the new piece or the one in that slot is worth more against one boss than the other.
    const splits = dragonSet && slot && [newItem(id, resource), mainItem(loadout, slot), dragonItem(loadout, slot)].some((it) => it && bossSpecific(it, ctx));
    if (splits && (isWeapon ? !isRag : true)) {
      sets.push('dragon');
      if (mainItem(loadout, slot) && !dragonItem(loadout, slot)) sets.push('wither');
    }
    // A new weapon or armor piece also comes with the enchants and potato books of the one it replaces (priced as
    // books), so it is not judged bare against an enchanted one: every enchant, or only books up to COPY_BOOK_MAX
    // each (the pricey ones then come as their own steps).
    const current = isWeapon && !isRag ? loadout.weapon : isArmor ? loadout.armor[cat] : null;
    const src = current && current.id !== id && (Object.keys(current.enchants ?? {}).length || (isWeapon && current.hpb)) ? current : null;
    const buy = (reforgeKeyName, reforge, extra = {}) => {
      for (const set of sets) {
        // The dragon-set versions come only in the fullest bundle too.
        if (set !== 'main' && !extra.full) continue;
        buyInto(set, reforgeKeyName, reforge, extra);
        // The copied-enchant bundles come only in the fullest version (best reforge and recombobulator).
        if (src && extra.full) for (const copy of ['cheap', 'all']) buyInto(set, reforgeKeyName, reforge, { ...extra, copy });
      }
    };
    const buyInto = (set, reforgeKeyName, reforge, extra) => {
      const base = { ...newItem(id, resource), reforge: reforgeKeyName ?? null };
      const free = FREE_ITEMS[id];
      const freeUp = FREE_UPGRADES[id];
      const fromName = freeUp && cleanName(resource.get(freeUp.from)?.name ?? freeUp.from);
      const parts = [...(free ? [{ coins: 1 }] : freeUp ? [{ coins: 1 }, ...freeUp.parts] : [{ id: buyPriceId(id, resource, ctx.cataLevel) }]), ...(reforge ? reforgeCost(reforge, base) : [])];
      const labels = [`${freeUp ? `Claim ${fromName} (${FREE_ITEMS[freeUp.from]}) and upgrade it into ${cleanName(info.name ?? id)} (${freeUp.parts.map((x) => `${x.qty} ${titleCase(x.id.toLowerCase())}${x.qty > 1 ? 's' : ''}`).join(', ')})`
        : `${free ? 'Claim' : 'Buy'} ${cleanName(info.name ?? id)}${free ? ` (${free})` : ''}`}${set === 'dragon' ? ' (dragon set)'
        : set === 'wither' ? ` (for withers, keep ${cleanName(mainItem(loadout, slot).name ?? mainItem(loadout, slot).id)} for dragons)` : ''}`,
      ...(reforge ? [`${reforge.name} reforge`] : [])];
      // RCM: a bought blade comes with the 3 Wither scrolls.
      if (ctx.mode === 'rcm' && WITHER_IMPACT_WEAPONS.includes(id)) {
        base.scrolls = [...WITHER_SCROLLS];
        parts.push(...WITHER_SCROLLS.map((x) => ({ id: x })));
        labels.push('Wither scrolls');
      }
      if (extra.recomb) { parts.push({ id: RECOMBOBULATOR_ID }); labels.push('Recombobulator'); }
      // Gem slots filled (unlocked when needed), so a piece with slots is not judged without the gems it will get.
      if (extra.gems) {
        const filled = fillGems({ ...base, recomb: Boolean(extra.recomb) }, ctx, prices);
        if (!filled) return;
        base.gems = filled.gems;
        parts.push(...filled.parts);
        labels.push(filled.label);
      }
      if (extra.copy) {
        base.enchants = {};
        for (const [key, level] of Object.entries(src.enchants ?? {})) {
          const ench = ENCHANTS[key];
          const way = enchantParts(key, level, 0, prices);
          const price = way && priceParts(way.parts, prices)?.cost;
          if (!ench?.categories.includes(cat) || (ench.items && !ench.items.includes(id)) || !price || (extra.copy === 'cheap' && price > COPY_BOOK_MAX)) continue;
          base.enchants[key] = level;
          parts.push(...way.parts);
        }
        // Potato books only matter on weapons.
        const hpb = isWeapon ? src.hpb ?? 0 : 0;
        base.hpb = hpb;
        if (hpb) parts.push({ id: HOT_POTATO.id, qty: Math.min(hpb, HOT_POTATO.max) });
        if (hpb > HOT_POTATO.max) parts.push({ id: FUMING_POTATO.id, qty: hpb - HOT_POTATO.max });
        if (src.artOfWar) { base.artOfWar = true; parts.push({ id: ART_OF_WAR.id }); }
        if (extra.copy === 'all' && base.dungeon) {
          base.masterStars = src.masterStars ?? 0;
          parts.push(...MASTER_STAR_IDS.slice(0, base.masterStars).map((x) => ({ id: x })));
        }
        // A full Necron's Blade variant with a Shadow Assassin Cloak worn also comes with the 3 Wither scrolls (1.1x).
        if (extra.copy === 'all' && WITHER_IMPACT_WEAPONS.includes(id) && !base.scrolls.length && SA_CLOAK_IDS.includes(loadout.equipment?.CLOAK?.id)) {
          base.scrolls = [...WITHER_SCROLLS];
          parts.push(...WITHER_SCROLLS.map((x) => ({ id: x })));
          labels.push('Wither scrolls (Shadow Assassin Cloak ability)');
        }
        labels.push(`${extra.copy === 'cheap' ? 'enchants (books up to 10M)' : 'all enchants, master stars'}${isWeapon ? ' and potato books' : ''} like ${cleanName(src.name ?? src.id)}`);
        // Replacing a dual-wielded weapon: the full copy comes as a dual-wield pair too (two copies, Execute on one),
        // so the new blade is not judged against the old pair as a single weapon.
        if (extra.copy === 'all' && src.dualWield && DUAL_WIELD_WEAPONS.includes(id) && base.enchants.prosecute) {
          base.dualWield = true;
          parts.push(...parts.slice());
          labels.push('dual wield (2 copies)');
        }
      }
      if (extra.enrichment) { parts.push({ id: `TALISMAN_ENRICHMENT_${ENRICHMENTS[extra.enrichment].key}` }); labels.push(`${enrichmentName(extra.enrichment)} Enrichment`); }
      push(labels.join(' + '), parts, (l) => {
      const next = cloneLoadout(l);
      const item = { ...base, recomb: Boolean(extra.recomb), enrichment: extra.enrichment ?? null };
      next.owned.add(id);
      if (set === 'dragon') { setDragonSlot(next, slot, item); return next; }
      if (isRag) next.rag = item;
      if (isWeapon && (!isRag || !next.weapon)) next.weapon = item;
      else if (isArmor) next.armor[cat] = item;
      else if (isEquip) next.equipment[equipSlot] = item;
      else if (isAcc) (next.accessories = new Map(next.accessories)).set(id, item);
      if (set === 'wither') setDragonSlot(next, slot, mainItem(l, slot));
      return next;
      }, { group: isAcc ? undefined : `buy:${set === 'dragon' ? 'dragon:' : ''}${isRag ? 'rag' : isWeapon ? 'weapon' : equipSlot ?? cat}`, repick: isWeapon,
        ...(isArmor && set === 'main' ? { armorSlot: cat, armorId: id } : {}) });
    };
    const fresh = newItem(id, resource);
    const recombable = canRecomb(fresh, resource);
    // Gear can come with the best reforge for its slot (and a recombobulator).
    const best = isAcc ? null : bestReforge(loadout, ctx, fresh);
    buy(null, null, { full: !best && !recombable });
    if (recombable) buy(null, null, { recomb: true, full: !best });
    if (isAcc) {
      // Enrichments need Legendary or higher, so most come after the recombobulator.
      for (const recomb of recombable ? [false, true] : [false]) {
        if (!ENRICHABLE_RARITIES.includes(itemRarity({ ...fresh, recomb }))) continue;
        for (const kind of enrichmentKinds(ctx)) buy(null, null, { recomb, enrichment: kind });
      }
    } else if (best) {
      buy(best.key, best.reforge, { full: !recombable });
      if (recombable) buy(best.key, best.reforge, { recomb: true, full: true });
    }
    if (!isAcc && resource.get(id)?.gemstone_slots?.length) buy(best?.key ?? null, best?.reforge ?? null, { recomb: recombable, full: true, gems: true });
  }

  // Clear boots: any boots faster than your current clear boots, worn only during the clear.
  const clearSpeed = bootsSpeed(loadout.clearBoots, ctx);
  for (const [id, info] of CLEAR_BOOTS_CLASSES.includes(cls) && ctx.mode !== 'rcm' ? resource : []) {
    if (info.category !== 'BOOTS' || loadout.owned.has(id) || !meetsRequirements(info, ctx.player)) continue;
    const boots = newItem(id, resource);
    if (bootsSpeed(boots, ctx) <= clearSpeed) continue;
    push(`Buy ${cleanName(info.name ?? id)} (clear boots for speed)`, [{ id: buyPriceId(id, resource, ctx.cataLevel) }],
      (l) => { const next = cloneLoadout(l); next.owned.add(id); next.clearBoots = boots; return next; }, { group: 'buy:clearBoots' });
  }

  // Attribute shards: straight to max level, like the sheet.
  for (const [id, shard] of Object.entries(SHARDS)) {
    if (shard.classes && !shard.classes.includes(cls)) continue;
    const hunting = ctx.player?.skills?.HUNTING;
    if (hunting != null && hunting < (SHARD_HUNTING_REQ[shard.rarity] ?? 0)) continue;
    const current = shardLevel(loadout, id);
    if (current >= SHARD_MAX_LEVEL) continue;
    const cum = SHARDS_CUMULATIVE[shard.rarity];
    const qty = cum[SHARD_MAX_LEVEL - 1] - (current ? cum[current - 1] : 0);
    push(`${shard.name} Shard → level ${SHARD_MAX_LEVEL} (${qty} shards)`, [{ id: `SHARD_${id}`, qty }],
      (l) => ({ ...cloneLoadout(l), shards: { ...l.shards, [id]: SHARD_MAX_LEVEL } }));
  }

  // Accessory Enrichment Swapper: one swapper changes every enriched accessory in the bag to the enrichment you pick.
  // Accessories you want to keep are taken out first, so a swap can cover all enriched accessories, only the dungeon
  // ones or only the regular ones.
  const enriched = [...loadout.accessories.values()].filter((a) => a.enrichment);
  const isDungeonAcc = (a) => Boolean(resource.get(a.id)?.dungeon_item);
  const SWAP_GROUPS = [['all', () => true], ['dungeon', isDungeonAcc], ['non-dungeon', (a) => !isDungeonAcc(a)]];
  for (const kind of enrichmentKinds(ctx)) {
    const seen = new Set();
    for (const [groupName, keep] of SWAP_GROUPS) {
      const targets = enriched.filter((a) => keep(a) && a.enrichment !== kind);
      const key = targets.map((a) => a.id).sort().join(',');
      if (!targets.length || seen.has(key)) continue;
      seen.add(key);
      const which = groupName === 'all' ? 'all' : `the ${groupName}`;
      push(`Swap ${which} enriched accessories (${targets.length}) to ${enrichmentName(kind)} (1 Accessory Enrichment Swapper${groupName === 'all' ? '' : ', take the others out first'})`,
        [{ id: ENRICHMENT_SWAPPER_ID }], (l) => {
          let next = l;
          for (const t of targets) {
            const cur = next.accessories.get(t.id) ?? t;
            next = replaceItem(next, cur, { ...cur, enrichment: kind });
          }
          return next;
        }, { group: 'enrichswap' });
    }
  }

  // Book of Progression not on damage (and SkyBlock level 75+): switching costs 500 coins x SkyBlock level.
  const sbLevel = ctx.profileStats?.sbLevel ?? 0;
  if (loadout.accessories.has(BOOK_OF_PROGRESSION_ID) && ctx.profileStats && sbLevel >= BOOK_DAMAGE_UNLOCK_LEVEL && loadout.bopBonus !== BOOK_DAMAGE_OPTION) {
    push('Set Book of Progression to Extra Infliction (damage)', [{ coins: BOOK_SWAP_COINS_PER_LEVEL * sbLevel }],
      (l) => ({ ...cloneLoadout(l), bopBonus: BOOK_DAMAGE_OPTION }), { group: 'bop' });
  }

  // Golden Dragon's Legendary Treasure: 1M more coins in the bank (+0.25% damage), up to 1B. The planner merges
  // deposits made back to back into one step.
  if (loadout.pets.some((pet) => pet.type === 'GOLDEN_DRAGON') && (loadout.bank ?? 0) < GOLDEN_DRAGON_BANK_CAP) {
    push(BANK_LABEL, [{ coins: BANK_STEP }], depositBank, { bank: true });
  }

  // Power stones: learn a new power at Maxwell and switch to it.
  for (const [power, stone] of Object.entries(POWER_STONES)) {
    if (loadout.unlockedPowers?.has(power) || !POWERS[power]) continue;
    push(`Learn ${titleCase(power)} power (${POWER_STONES_TO_LEARN} ${cleanName(resource.get(stone)?.name ?? stone)})`, [{ id: stone, qty: POWER_STONES_TO_LEARN }], (l) => {
      const next = cloneLoadout(l);
      next.unlockedPowers.add(power);
      next.power = power;
      return next;
    }, { group: 'power' });
  }

  // Wither scrolls: all 3 or nothing, on a blade you hold (RCM: for Wither Impact; LCM: for the Shadow Assassin
  // Cloak's 1.1x, which only works with that blade in hand). Covers the main and dragon-set weapon.
  const scrollTargets = ctx.mode === 'rcm' && cls === 'mage' ? [loadout.weapon] : [loadout.weapon, loadout.dragonGear?.weapon];
  for (const blade of scrollTargets.filter((b) => b && WITHER_IMPACT_WEAPONS.includes(b.id) && !hasWitherImpact(b))) {
    const missing = WITHER_SCROLLS.filter((x) => !(blade.scrolls ?? []).includes(x));
    push(`${missing.map((x) => SCROLL_NAMES[x]).join(', ')} on ${cleanName(blade.name ?? blade.id)}${ctx.mode === 'rcm' ? '' : ' (Shadow Assassin Cloak ability)'}`, missing.map((x) => ({ id: x })), (l) => {
      const done = { ...blade, scrolls: [...new Set([...(blade.scrolls ?? []), ...WITHER_SCROLLS])] };
      const next = replaceItem(l, blade, done);
      next.blades = (next.blades ?? []).map((b) => (b === blade ? done : b));
      next.witherImpact = true;
      return next;
    }, { group: `scrolls:${blade.id}` });
  }

  // Pets (level 100, Golden Dragon 200) from the auction house. One active at a time.
  const ownedTypes = new Set(loadout.pets.map((p) => p.type));
  // RCM: Golden Dragon is the clear pet for everyone, so an owner gets no other pet suggestions.
  const rcmHasGdrag = ctx.mode === 'rcm' && ownedTypes.has('GOLDEN_DRAGON');
  for (const type of rcmHasGdrag ? [] : allPetTypes()) {
    if (ownedTypes.has(type) || (ctx.mode === 'rcm' && RCM_SKIPPED_PETS.includes(type))) continue;
    const tier = BUY_RARITIES.find((t) => petTiers(type).includes(t));
    if (!tier) continue;
    const pet = { type, tier, heldItem: null };
    if (!dpsPets([pet], ctx).length) continue;
    const petName = titleCase(type.toLowerCase());
    const buyPet = (p, label, parts) => push(label, parts,
      (l) => { const next = cloneLoadout(l); next.pets.push(p); return selectPet(next, ctx, ctx.weights ?? weightsFor(cls, null, ctx.mode, ctx.floorMode)); }, { group: 'pet' });
    buyPet(pet, `Buy ${petName} Pet (${titleCase(tier.toLowerCase())}, level ${maxPetLevel(type)})`, [{ id: `PET_${type}_${tier}_MAX` }]);
    // Also bundled with each pet item that raises its stats, so a pet is not judged without the item it will get.
    const bare = petStats(pet, ctx.petProfile).stats;
    for (const [itemId, item] of Object.entries(PET_ITEMS)) {
      const held = petStats({ ...pet, heldItem: itemId }, ctx.petProfile).stats;
      if (!PET_ITEM_STATS.some((k) => (held[k] ?? 0) > (bare[k] ?? 0))) continue;
      buyPet({ ...pet, heldItem: itemId }, `Buy ${petName} Pet (${titleCase(tier.toLowerCase())}, level ${maxPetLevel(type)}) + ${item.name}`,
        [{ id: `PET_${type}_${tier}_MAX` }, { id: itemId }]);
    }
    // The rarity below plus a Tier Boost plays at the same rarity, often for less.
    const below = petTiers(type).find((t) => effectiveTier({ type, tier: t, heldItem: TIER_BOOST_ID }) === tier && t !== tier);
    if (below) {
      buyPet({ type, tier: below, heldItem: TIER_BOOST_ID }, `Buy ${petName} Pet (${titleCase(below.toLowerCase())}, level ${maxPetLevel(type)}) + Tier Boost`,
        [{ id: `PET_${type}_${below}_MAX` }, { id: TIER_BOOST_ID }]);
    }
  }


  // Pet items for each active pet (the wither and dragon pets).
  for (const active of new Set(Object.values(loadout.activePets ?? {}).filter(Boolean))) {
    const before = petStats(active, ctx.petProfile).stats;
    for (const [itemId, item] of Object.entries(PET_ITEMS)) {
      if (active.heldItem === itemId) continue;
      const after = petStats({ ...active, heldItem: itemId }, ctx.petProfile).stats;
      if (!PET_ITEM_STATS.some((k) => (after[k] ?? 0) > (before[k] ?? 0))) continue;
      push(`${item.name} on ${titleCase(active.type.toLowerCase())} Pet`, [{ id: itemId }], (l) => {
        const next = cloneLoadout(l);
        const pet = { ...active, heldItem: itemId };
        next.pets = next.pets.map((p) => (p === active ? pet : p));
        for (const [t, p] of Object.entries(next.activePets)) if (p === active) next.activePets[t] = pet;
        return next;
      }, { group: `petitem:${active.type}` });
    }
    // A Tier Boost pet loses its rarity when the Tier Boost comes off, so a new pet item can come with a
    // Kat upgrade to that rarity (Kat's level-100 coin cost and items, plus a Kat Flower).
    const kat = active.heldItem === TIER_BOOST_ID && KAT_UPGRADES[active.type]?.[active.tier];
    if (kat) {
      const level = maxPetLevel(active.type);
      const katParts = [{ coins: Math.round(kat.coins * (1 - KAT_LEVEL_DISCOUNT * level)) }, ...kat.items, { id: KAT_FLOWER_ID }];
      for (const [itemId, item] of Object.entries(PET_ITEMS)) {
        push(`Kat upgrade ${titleCase(active.type.toLowerCase())} Pet to ${titleCase(kat.to.toLowerCase())} + ${item.name}`, [...katParts, { id: itemId }], (l) => {
          const next = cloneLoadout(l);
          const pet = { ...active, tier: kat.to, heldItem: itemId };
          next.pets = next.pets.map((p) => (p === active ? pet : p));
          for (const [t, p] of Object.entries(next.activePets)) if (p === active) next.activePets[t] = pet;
          return next;
        }, { group: `petitem:${active.type}` });
      }
    }
  }

  // Essence shop perks: any level above the current one (essence bought on the bazaar, Catacombs 20+).
  if (ctx.cataLevel >= ESSENCE_MIN_CATA) {
    for (const [key, perk] of Object.entries(ESSENCE_PERKS)) {
      if (perk.profileOnly || (perk.pet && !loadout.pets.some((p) => p.type === perk.pet))) continue;
      const current = loadout.perks?.[key] ?? 0;
      for (let level = ctx.upgradeMode === 'max' ? Math.max(current + 1, perk.costs.length) : current + 1; level <= perk.costs.length; level++) {
        const qty = perk.costs.slice(current, level).reduce((a, b) => a + b, 0);
        push(`${perk.name} ${roman(level)} (${qty.toLocaleString('en-US')} ${titleCase(perk.essence.toLowerCase())} Essence)`,
          [{ id: `ESSENCE_${perk.essence}`, qty }],
          (l) => ({ ...cloneLoadout(l), perks: { ...l.perks, [key]: level } }), { group: `perk:${key}` });
      }
    }
  }

  // Pets and ability items.
  for (const entry of EXTRA_UPGRADES) {
    if (loadout.extras.has(entry.key)) continue;
    if (entry.classes && !entry.classes.includes(cls)) continue;
    if (entry.requires && !loadout.extras.has(entry.requires)) continue;
    if (entry.cost.some((part) => !meetsRequirements(resource.get(part.id), ctx.player))) continue;
    push(entry.name + (entry.owned ? '' : ' (?)'), entry.cost,
      (l) => { const next = cloneLoadout(l); next.extras.add(entry.key); return next; },
      { unverified: !entry.owned, group: entry.group && `extra:${entry.group}` });
  }
  return ups;
}

// Armor only takes Ancient (ARMOR_REFORGE); everything else takes any reforge that fits. The Mage RCM clear set
// is its own armor, so any armor reforge (Necrotic, Loving, ...) is allowed there.
function reforgeAllowed(key, reforge, item, ctx) {
  if (ARMOR_SLOTS.includes(item.category) && key !== ARMOR_REFORGE && ctx?.mode !== 'rcm') return false;
  return reforgeAppliesTo(reforge, item);
}

// Cost parts for one reforge: its stone, or the coins for the expected number of blacksmith rolls.
function reforgeCost(reforge, item) {
  if (reforge.stone) return [{ id: reforge.stone }];
  const rolls = Object.values(allReforges()).filter((r) => !r.stone && reforgeAppliesTo(r, item)).length;
  return [{ coins: BLACKSMITH_REFORGE_COST * Math.max(1, rolls) }];
}

// Best reforge (stone or blacksmith) for a fresh item of this category/rarity, tested in the current loadout. Cached per run.
function bestReforge(loadout, ctx, item) {
  const rarity = statRarity(itemRarity(item));
  const dungeon = isDungeon(item, ctx.resource);
  const key = `${item.category}:${rarity}:${dungeon}`;
  ctx.reforgeCache ??= new Map();
  if (ctx.reforgeCache.has(key)) return ctx.reforgeCache.get(key);
  const slot = ARMOR_SLOTS.includes(item.category) ? item.category : equipmentSlot(item.category) ?? 'weapon';
  const dummy = { ...item, id: '__REFORGE_TEST__', name: 'test' };
  let best = null;
  let bestScore = -Infinity;
  for (const [rKey, reforge] of Object.entries(allReforges())) {
    if (!reforgeAllowed(rKey, reforge, dummy, ctx)) continue;
    if (!hasDpsStats(reforge.stats?.[rarity]) && !reforge.cataStat) continue;
    const trial = cloneLoadout(loadout);
    setSlot(trial, slot, { ...dummy, reforge: rKey });
    const r = evaluate(trial, ctx, ctx.weights ?? { wither: 50, dragon: 50 });
    const score = r.wither + r.dragon;
    if (score > bestScore) { bestScore = score; best = { key: rKey, reforge }; }
  }
  ctx.reforgeCache.set(key, best);
  return best;
}

// Gem upgrades for one item: the best DPS gem at each upgrade quality, per slot.
// Gems a class puts in a slot, best first (the first one the slot accepts is used).
const GEM_PICKS = { arch: ['JASPER', 'ONYX'], bers: ['JASPER', 'ONYX'], mage: ['SAPPHIRE'] };

// Every gem slot on a fresh item filled with the class's gem at the best priced quality (Perfect, else Flawless),
// unlock costs included. Returns { gems, parts, label } or null when no slot can be filled.
function fillGems(item, ctx, prices) {
  const slots = ctx.resource.get(item.id)?.gemstone_slots;
  if (!Array.isArray(slots) || !slots.length) return null;
  const counts = {};
  const gems = { slots: {}, unlocked: [] };
  const parts = [];
  const names = new Set();
  for (const slot of slots) {
    const type = slot.slot_type;
    const key = `${type}_${counts[type] = (counts[type] ?? -1) + 1}`;
    const gem = (GEM_PICKS[ctx.cls] ?? []).find((g) => (SLOT_GEMS[type] ?? [type]).includes(g) && GEMSTONES[g]);
    const quality = gem && ['PERFECT', 'FLAWLESS'].find((q) => prices.get(`${q}_${gem}_GEM`));
    if (!quality) continue;
    gems.slots[key] = { gem, quality };
    parts.push({ id: `${quality}_${gem}_GEM` });
    if (slot.costs) {
      gems.unlocked.push(key);
      for (const c of slot.costs) {
        if (c.type === 'COINS') parts.push({ coins: c.coins });
        else if (c.type === 'ITEM') parts.push({ id: c.item_id, qty: c.amount });
      }
    }
    names.add(`${titleCase(quality.toLowerCase())} ${titleCase(gem.toLowerCase())}`);
  }
  if (!parts.length) return null;
  return { gems, parts, label: `${[...names].join(', ')} gems` };
}

function gemUpgrades(item, ctx) {
  const slots = ctx.resource.get(item.id)?.gemstone_slots;
  if (!Array.isArray(slots) || !slots.length) return [];
  const rarity = statRarity(itemRarity(item));
  const counts = {};
  const out = [];
  for (const slot of slots) {
    const type = slot.slot_type;
    const index = counts[type] = (counts[type] ?? -1) + 1;
    const key = `${type}_${index}`;
    const current = item.gems?.slots?.[key];
    const unlocked = !slot.costs || item.gems?.unlocked?.includes(key);
    const accepts = (SLOT_GEMS[type] ?? [type]).filter((g) => GEMSTONES[g]);
    for (const gem of accepts) {
      for (const quality of ctx.upgradeMode === 'max' ? UPGRADE_QUALITIES.slice(-1) : UPGRADE_QUALITIES) {
        if (current && current.gem === gem && GEM_QUALITIES.indexOf(current.quality) >= GEM_QUALITIES.indexOf(quality)) continue;
        const gain = gemStat(gem, quality, rarity);
        if (!gain) continue;
        const parts = [{ id: `${quality}_${gem}_GEM` }];
        if (!unlocked) {
          for (const c of slot.costs) {
            if (c.type === 'COINS') parts.push({ coins: c.coins });
            else if (c.type === 'ITEM') parts.push({ id: c.item_id, qty: c.amount });
          }
        }
        out.push({
          gem, quality, unlock: !unlocked, parts, slotKey: key,
          apply: (it) => ({
            ...it,
            gems: {
              slots: { ...(it.gems?.slots ?? {}), [key]: { gem, quality } },
              unlocked: unlocked ? [...(it.gems?.unlocked ?? [])] : [...(it.gems?.unlocked ?? []), key],
            },
          }),
        });
      }
    }
  }
  return out;
}

// Weapon, armor and equipment pieces a step touches. Same id = an upgrade on that piece; an id it upgrades into
// (Golden -> Diamond head, Wither -> Necron's, fragging) = the same piece under a new id; any other id = a
// buy that replaces the old piece.
function pieceChanges(prev, next) {
  const changes = [];
  const seen = new Set();
  // A piece moved from the main set to the dragon set is still worn, so it is not replaced.
  const wornAfter = new Set(['wither', 'dragon'].flatMap((t) => { const g = gearView(next, t); return [g.weapon, ...Object.values(g.armor), ...Object.values(g.equipment)]; }));
  for (const t of ['wither', 'dragon']) {
    const before = gearView(prev, t);
    const after = gearView(next, t);
    for (const [group, slots] of [['weapon', ['weapon']], ['armor', ARMOR_SLOTS], ['equipment', EQUIPMENT_SLOTS]]) {
      for (const slot of slots) {
        const a = group === 'weapon' ? before.weapon : before[group]?.[slot];
        const b = group === 'weapon' ? after.weapon : after[group]?.[slot];
        if (!b || a === b) continue;
        const key = `${slot}:${b.id}`;
        if (seen.has(key)) continue;
        seen.add(key);
        const prevKey = a ? `${slot}:${a.id}` : null;
        const kind = !a || a.id === b.id || wornAfter.has(a) ? 'same' : upgradesInto(a.id, b.id) ? 'upgrade' : 'replace';
        changes.push({ key, prevKey, kind, item: b });
      }
    }
  }
  return changes;
}

// What a step sets, as keys a later step can overwrite. Stacking upgrades (stars, master stars, potato books,
// shards, essence perks) and one-way ones (recombobulator) never get overwritten, so they have no keys.
// A new piece in a slot (not an upgrade of the old one) also overwrites every key on that slot.
function stepFacts(prev, next) {
  const facts = new Set();
  const item = (slot, a, b) => {
    if (a === b || !b) return;
    if (!a || (a.id !== b.id && !upgradesInto(a.id, b.id))) { facts.add(`${slot}:piece`); return; }
    if ((a.reforge ?? null) !== (b.reforge ?? null)) facts.add(`${slot}:reforge`);
    if ((a.enrichment ?? null) !== (b.enrichment ?? null)) facts.add(`${slot}:enrichment`);
    for (const k of new Set([...Object.keys(a.enchants ?? {}), ...Object.keys(b.enchants ?? {})])) {
      if ((a.enchants?.[k] ?? 0) !== (b.enchants?.[k] ?? 0)) facts.add(`${slot}:enchant:${k}`);
    }
    const ga = a.gems?.slots ?? {};
    const gb = b.gems?.slots ?? {};
    for (const k of new Set([...Object.keys(ga), ...Object.keys(gb)])) {
      if (JSON.stringify(ga[k] ?? null) !== JSON.stringify(gb[k] ?? null)) facts.add(`${slot}:gem:${k}`);
    }
  };
  item('weapon', prev.weapon, next.weapon);
  item('rag', prev.rag, next.rag);
  for (const slot of ARMOR_SLOTS) item(slot, prev.armor?.[slot], next.armor?.[slot]);
  for (const slot of EQUIPMENT_SLOTS) item(slot, prev.equipment?.[slot], next.equipment?.[slot]);
  for (const slot of ['weapon', ...ARMOR_SLOTS, ...EQUIPMENT_SLOTS]) item(`dragon:${slot}`, dragonItem(prev, slot), dragonItem(next, slot));
  for (const [id, b] of next.accessories) {
    const a = prev.accessories.get(id);
    // A new accessory takes over its family (a ring after its talisman).
    if (!a) facts.add(`acc:${accessoryFamily(id)}:piece`);
    else if ((a.enrichment ?? null) !== (b.enrichment ?? null)) facts.add(`acc:${accessoryFamily(id)}:enrichment`);
  }
  for (const t of Object.keys(next.activePets ?? {})) {
    const a = prev.activePets?.[t];
    const b = next.activePets?.[t];
    if (!b || a === b) continue;
    if (a && a.type === b.type) { if (a.heldItem !== b.heldItem || a.tier !== b.tier) facts.add(`pet:${b.type}:item`); }
    else facts.add(`activePet:${t}`);
  }
  for (const key of next.extras ?? []) {
    if (prev.extras?.has(key)) continue;
    const group = EXTRA_UPGRADES.find((e) => e.key === key)?.group;
    if (group) facts.add(`extra:${group}`);
  }
  return facts;
}

// The gear slot a fact belongs to ("HELMET", "acc:<family>"), or null for pets and extras.
const ITEM_SLOTS = new Set(['weapon', 'rag', ...ARMOR_SLOTS, ...EQUIPMENT_SLOTS]);
function factSlot(fact) {
  const parts = fact.split(':');
  if (parts[0] === 'acc' || parts[0] === 'dragon') return `${parts[0]}:${parts[1]}`;
  return ITEM_SLOTS.has(parts[0]) ? parts[0] : null;
}

// True when `later` overwrites something `earlier` set (the same key, or a new piece in that slot).
function overwrites(later, earlier) {
  for (const f of earlier) {
    if (later.has(f)) return true;
    const slot = factSlot(f);
    if (slot && later.has(`${slot}:piece`)) return true;
  }
  return false;
}

// Steps overwritten within REPLACED_SOON_COINS that add REPLACED_SOON_MIN_GAIN% or less.
function replacedSoon(steps) {
  const out = [];
  for (let i = 0; i < steps.length; i++) {
    if (!steps[i].facts.size || steps[i].gain > REPLACED_SOON_MIN_GAIN) continue;
    let coins = 0;
    for (let j = i + 1; j < steps.length; j++) {
      coins += steps[j].cost;
      if (coins > REPLACED_SOON_COINS) break;
      if (overwrites(steps[j].facts, steps[i].facts)) { out.push(steps[i].label); break; }
    }
  }
  return out;
}

// Scores candidates ups[i] for every i where i % parts === part (the evaluation workers split the list this way).
// Returns { index, changes, gain } for the ones that pass the path-free rules and add damage.
export function scoreUpgrades(ups, loadout, current, ctx, weights, banned, part = 0, parts = 1) {
  const skip = new Set(banned);
  const out = [];
  for (let index = part; index < ups.length; index += parts) {
    const up = ups[index];
    if (!(up.cost > 0) || skip.has(up.label)) continue;
    // A new weapon can change the best pet and power (both free to switch), so one that already adds damage is
    // scored with them re-picked, the same way the step is applied. (Re-picking only ever raises the score; a weapon
    // that loses damage before it is not worth the extra work.)
    let next = up.apply(loadout);
    const changes = pieceChanges(loadout, next);
    let gain = weightedGain(current, evaluate(next, ctx, weights), weights);
    if (up.repick && gain > 1e-9) {
      next = selectPower(selectPet(next, ctx, weights), ctx, weights);
      gain = weightedGain(current, evaluate(next, ctx, weights), weights);
    }
    if (gain > 1e-9) out.push({ index, changes, gain });
    // Armor buys that lose damage alone (they break a set bonus) can still win in a bundle: kept for the shortlist.
    else if (up.armorSlot) out.push({ index, changes, gain, shortlistOnly: true });
  }
  return out;
}

// Armor bundles: several armor pieces bought as one step (any mix of slots and sets: Diamond Necron Head with
// Skeleton Master pieces, 3 Frozen Blaze pieces, ...), so a set bonus never traps the plan. Each slot's shortlist
// is its ARMOR_BUNDLE_SHORTLIST best single buys (by gain, even a loss), and every mix of 2 or more slots is tried.
// singles: [{ up, gain }] for the main-set armor buys of one loadout.
function armorBundles(singles) {
  const bySlot = new Map();
  for (const { up, gain } of singles) {
    const slot = bySlot.get(up.armorSlot) ?? new Map();
    const prev = slot.get(up.armorId);
    if (!prev || gain > prev.gain) slot.set(up.armorId, { up, gain });
    bySlot.set(up.armorSlot, slot);
  }
  const lists = ARMOR_SLOTS.map((slot) => [null, ...[...(bySlot.get(slot)?.values() ?? [])].sort((a, b) => b.gain - a.gain).slice(0, ARMOR_BUNDLE_SHORTLIST).map((x) => x.up)]);
  const bundles = [];
  const walk = (i, picked) => {
    if (i === lists.length) {
      if (picked.length < 2) return;
      bundles.push({
        label: `Armor bundle: ${picked.map((u) => u.label).join('; ')}`,
        cost: picked.reduce((sum, u) => sum + u.cost, 0),
        source: [...new Set(picked.flatMap((u) => String(u.source).split('+')))].join('+'),
        unverified: picked.some((u) => u.unverified),
        apply: (l) => picked.reduce((acc, u) => u.apply(acc), l),
        repick: true,
      });
      return;
    }
    for (const up of lists[i]) walk(i + 1, up ? [...picked, up] : picked);
  };
  walk(0, []);
  return bundles;
}

// Greedy plan: each step is the best % per million coins on the current loadout, applied before the next, then
// every option is recalculated. Nothing locks: Duplex I can be followed by Duplex II and later V.
// rules.doomed: pieces the plan replaces later, which get under REPLACED_PIECE_BUDGET coins in total (buy +
// upgrades, carried through Golden -> Diamond and Wither -> Necron's upgrades). rules.banned: step labels that get
// overwritten too soon. Both only ever remove candidates, so a re-plan (prev) keeps the previous plan up to the
// first step the rules now reject and resumes there. rules.states caches every distinct loadout's scored
// candidates by content, so a re-plan that reaches gear it has already scored (often in a different order) reuses
// the scores instead of evaluating ~2000 candidates again.
async function greedyPlan(startLoadout, ctx, prices, weights, limit, rules, scorer, prev, isCancelled) {
  const pieceKey = (c) => (c.kind === 'upgrade' ? c.prevKey : c.key);
  // A floor-scaled drop from below floor 10 is replaced by the floor 10 one once you can enter floor 10 (a floor 9
  // Skeleton Master Chestplate by a floor 10 one), so it is doomed from the start: under the 25M budget, buy included.
  const lowFloor = highestFloor(ctx.cataLevel) < MAX_FLOOR;
  const doomed = (c) => rules.doomed.has(pieceKey(c)) || (lowFloor && isFloorDrop(c.item.id, ctx.resource));
  const allowed = (cand, spend) => !rules.banned.has(cand.label)
    && !cand.changes.some((c) => doomed(c) && (spend.get(pieceKey(c)) ?? 0) + cand.cost >= REPLACED_PIECE_BUDGET);

  // The scored state for a loadout, shared by every loadout with the same content (the candidate list also depends
  // on what was ever owned, learned powers and blades, so they join the key).
  const stateFor = (loadout) => {
    const key = [loadoutKey(loadout), [...loadout.owned].sort(), [...(loadout.unlockedPowers ?? [])].sort(), JSON.stringify(loadout.blades ?? [])].join('\n');
    if (!rules.states.has(key)) rules.states.set(key, { loadout, current: evaluate(loadout, ctx, weights), candidates: null });
    return rules.states.get(key);
  };
  // Every upgrade from this loadout that adds damage, kept light (no loadouts): scored here or, with a scorer,
  // across the evaluation workers. Only rules that do not depend on the path taken are applied while scoring; the
  // spending budget is checked when picking.
  const candidatesFor = async ({ loadout, current }) => {
    const ups = availableUpgrades(loadout, ctx, prices);
    const banned = [...rules.banned];
    const scored = scorer ? await scorer({ loadout, current, banned, count: ups.length }) : scoreUpgrades(ups, loadout, current, ctx, weights, banned);
    const bundles = armorBundles(scored.filter((x) => ups[x.index].armorSlot).map((x) => ({ up: ups[x.index], gain: x.gain })));
    const bundleScores = scoreUpgrades(bundles, loadout, current, ctx, weights, banned).filter((x) => !x.shortlistOnly);
    const light = (list) => ({ index, changes, gain }) => {
      const { label, cost, source, unverified, apply, bank, stacksOn } = list[index];
      return { label, cost, source, unverified, apply, bank, stacksOn, changes, gain, perMillion: gain / (cost / 1_000_000) };
    };
    return [...scored.filter((x) => !x.shortlistOnly).map(light(ups)), ...bundleScores.map(light(bundles))];
  };

  const keep = prev ? prev.steps.findIndex((st) => !allowed(st.choice, st.path.spend)) : 0;
  if (keep < 0) return prev;
  const steps = prev ? prev.steps.slice(0, keep) : [];
  let path = prev ? prev.steps[keep].path : { state: stateFor(startLoadout), spend: new Map(), parent: new Map(), replaced: new Set() };
  const startDamage = prev ? prev.startDamage : path.state.current;

  while (steps.length < limit) {
    // Cooperative cancel: the /dps session went inactive, so stop here.
    if (isCancelled?.()) throw new Error('Plan cancelled.');
    const { state } = path;
    state.candidates ??= await candidatesFor(state);
    let best = null;
    for (const cand of state.candidates) if (allowed(cand, path.spend) && (!best || cand.perMillion > best.perMillion)) best = cand;
    if (!best) break;
    let loadout = selectPower(selectPet(best.apply(state.loadout), ctx, weights), ctx, weights);
    let step = { label: best.label, cost: best.cost, source: best.source, unverified: best.unverified, gain: best.gain, perMillion: best.perMillion };
    // Deposits right after a deposit join the step before, so the plan shows one bank step.
    const joined = best.bank && steps.at(-1)?.choice.bank ? steps.pop() : null;
    const base = joined ? joined.path.state : state;
    if (joined) path = joined.path;
    if (best.bank) {
      // Keep depositing 1M at a time while the next million still beats every other option, as one step.
      let next = null;
      for (const cand of state.candidates) if (!cand.bank && allowed(cand, path.spend) && (!next || cand.perMillion > next.perMillion)) next = cand;
      let now = evaluate(loadout, ctx, weights);
      while ((loadout.bank ?? 0) < GOLDEN_DRAGON_BANK_CAP) {
        const more = depositBank(loadout);
        const after = evaluate(more, ctx, weights);
        const gain = weightedGain(now, after, weights);
        if (!(gain > 1e-9) || (next && gain < next.perMillion)) break;
        loadout = more;
        now = after;
      }
      const amount = (loadout.bank ?? 0) - (base.loadout.bank ?? 0);
      const gain = weightedGain(base.current, now, weights);
      step = { ...step, label: `Put ${amount >= 1e9 ? `${+(amount / 1e9).toFixed(3)}B` : `${(amount / 1e6).toLocaleString('en-US')}M`} coins in the bank (Golden Dragon Legendary Treasure)`, cost: amount, gain, perMillion: gain / (amount / 1e6) };
    }
    const facts = stepFacts(base.loadout, loadout);
    // A combined enchant level builds on the one before (nothing is wasted), so it does not overwrite it.
    if (best.stacksOn) facts.delete(best.stacksOn);
    steps.push({ ...step, facts, choice: best, path });
    const spend = new Map(path.spend);
    const parent = new Map(path.parent);
    const replaced = new Set(path.replaced);
    for (const c of best.changes) {
      spend.set(c.key, (spend.get(pieceKey(c)) ?? 0) + best.cost);
      if (c.kind === 'upgrade') parent.set(c.key, c.prevKey);
      if (c.kind === 'replace') replaced.add(c.prevKey);
    }
    path = { state: stateFor(loadout), spend, parent, replaced };
  }
  // Pieces this plan replaces after spending the budget or more on them, with the ids they had before
  // being upgraded (so the budget also covers spending before the upgrade).
  const overspent = [];
  for (const key of path.replaced) {
    if ((path.spend.get(key) ?? 0) < REPLACED_PIECE_BUDGET) continue;
    for (let k = key; k; k = path.parent.get(k)) overspent.push(k);
  }
  return { steps, startDamage, endDamage: path.state.current, loadout: path.state.loadout, overspent };
}

// scorer (optional): scores a state's candidates in parallel (src/worker/evalPool.js), same results as in-thread.
export async function planUpgrades(startLoadout, ctx, prices, weights, limit = MAX_UPGRADES_LISTED, scorer = null, isCancelled = null) {
  ctx.weights = weights;
  const rules = { doomed: new Set(), banned: new Set(), states: new Map() };
  let plan = await greedyPlan(startLoadout, ctx, prices, weights, limit, rules, scorer, null, isCancelled);
  // Re-plan until no piece is overspent before being replaced (25M budget) and no small step is overwritten
  // within 10M coins. Each round resumes from the first step the new rules reject.
  for (let round = 0; round < REPLAN_ROUNDS; round++) {
    const freshPieces = plan.overspent.filter((key) => !rules.doomed.has(key));
    const freshSteps = replacedSoon(plan.steps).filter((label) => !rules.banned.has(label));
    if (!freshPieces.length && !freshSteps.length) break;
    for (const key of freshPieces) rules.doomed.add(key);
    for (const label of freshSteps) rules.banned.add(label);
    plan = await greedyPlan(startLoadout, ctx, prices, weights, limit, rules, scorer, plan, isCancelled);
  }
  const { overspent, ...rest } = plan;
  // % change vs withers and dragons for each step (the weighted gain combines them).
  const pct = (a, b) => (a > 0 ? (b / a - 1) * 100 : b > 0 ? 100 : 0);
  rest.steps = rest.steps.map(({ facts, choice, path, ...step }, i) => {
    const before = path.state.current;
    const after = plan.steps[i + 1]?.path.state.current ?? plan.endDamage;
    return { ...step, witherGain: pct(before.wither, after.wither), dragonGain: pct(before.dragon, after.dragon) };
  });
  return rest;
}


// Profile upgrades (no coins): the next level of each profile stat source, ranked by % damage gained.
const CLASS_LEVEL_KEY = { arch: 'archerLevel', bers: 'berserkLevel', mage: 'mageLevel' };
const CLASS_NAME = { arch: 'Archer', bers: 'Berserk', mage: 'Mage' };
// prices (optional): accessories with no price that cannot be traded (quest / free talismans) are listed here.
export function profileUpgrades(loadout, ctx, weights, prices = null) {
  const base = evaluate(loadout, ctx, weights);
  const rows = [];
  const tryCtx = (label, changes) => {
    const next = { ...ctx, ...changes };
    const gain = weightedGain(base, evaluate(loadout, next, weights), weights);
    if (gain > 1e-9) rows.push({ label, gain });
  };
  const tryLoadout = (label, next) => {
    const gain = weightedGain(base, evaluate(selectPet(next, ctx, weights), ctx, weights), weights);
    if (gain > 1e-9) rows.push({ label, gain });
  };
  const withPlayer = (patch) => ({ player: { ...(ctx.player ?? {}), ...patch } });
  if (ctx.cataLevel < CATA_STAT_BONUS.length - 1) tryCtx(`Catacombs ${ctx.cataLevel} → ${ctx.cataLevel + 1}`, { cataLevel: ctx.cataLevel + 1 });
  const classKey = CLASS_LEVEL_KEY[ctx.cls];
  if ((ctx[classKey] ?? 0) < 50) tryCtx(`${CLASS_NAME[ctx.cls]} ${ctx[classKey]} → ${ctx[classKey] + 1}`, { [classKey]: ctx[classKey] + 1 });
  if (ctx.combatLevel < COMBAT_MAX_LEVEL) tryCtx(`Combat ${ctx.combatLevel} → ${ctx.combatLevel + 1}`, { combatLevel: ctx.combatLevel + 1 });
  const skills = ctx.player?.skills;
  if (skills) {
    for (const [skill, { max }] of Object.entries(SKILL_BONUSES)) {
      const level = skills[skill] ?? 0;
      if (level < max) tryCtx(`${titleCase(skill.toLowerCase())} ${level} → ${level + 1}`, withPlayer({ skills: { ...skills, [skill]: level + 1 } }));
    }
  }
  const slayers = ctx.player?.slayers;
  if (slayers) {
    for (const boss of new Set([...Object.keys(SLAYER_BONUSES), 'wolf'])) {
      const level = slayers[boss] ?? 0;
      if (level >= SLAYER_MAX_LEVEL) continue;
      const changes = withPlayer({ slayers: { ...slayers, [boss]: level + 1 } });
      if (boss === 'wolf' && WOLF_SLAYER_SPEED_LEVELS.includes(level + 1)) changes.profileSpeed = (ctx.profileSpeed ?? 0) + 1;
      tryCtx(`${titleCase(boss)} Slayer ${level} → ${level + 1}`, changes);
    }
  }
  const p = ctx.profileStats;
  if (p) {
    const target = (Math.floor((p.sbLevel ?? 0) / SB_STRENGTH_EVERY) + 1) * SB_STRENGTH_EVERY;
    tryCtx(`SkyBlock level ${p.sbLevel} → ${target}`, { profileStats: { ...p, sbLevel: target } });
    for (const [key, song] of Object.entries(HARP_SONGS)) {
      if (!(p.harpSongs ?? []).includes(key)) tryCtx(`Melody's Harp: ${song.name} (perfect)`, { profileStats: { ...p, harpSongs: [...(p.harpSongs ?? []), key] } });
    }
  }
  // Golden Dragon's Shining Scales: +11.1 strength per digit of Gold collection (max 9 digits).
  const gold = ctx.petProfile?.goldCollection;
  if (gold != null && loadout.pets.some((pet) => pet.type === 'GOLDEN_DRAGON')) {
    const digits = String(Math.max(0, Math.floor(gold))).length;
    if (digits < GOLD_COLLECTION_DIGITS) {
      const target = 10 ** digits;
      tryCtx(`Gold collection to ${target.toLocaleString('en-US')} (Golden Dragon: +11.1 strength)`, { petProfile: { ...ctx.petProfile, goldCollection: target } });
    }
  }
  // The One on a worn necklace: each collection maxed adds its strength.
  const necklaces = [loadout.equipment?.NECKLACE, loadout.dragonGear?.equipment?.NECKLACE].filter(Boolean);
  const theOne = Math.max(0, ...necklaces.map((n) => n.enchants?.ultimate_the_one ?? 0));
  if (theOne >= (ENCHANTS.ultimate_the_one.minLevel ?? 1) && ctx.collections) {
    const per = ENCHANTS.ultimate_the_one.theOne[theOne - 1];
    for (const c of ctx.collections.unmaxed) {
      tryCtx(`Max ${c.name} collection (${c.amount.toLocaleString('en-US')} / ${c.max.toLocaleString('en-US')}; The One: +${per} strength)`,
        { collections: { ...ctx.collections, maxed: ctx.collections.maxed + 1 } });
    }
  }
  // Free quest accessories (soulbound, no price): only the ones that raise accessory (magical) power. In an upgrade
  // chain, only the next tier that does (a Campfire badge that stays the same rarity is skipped).
  if (prices) {
    const { resource } = ctx;
    const accIds = new Set(loadout.accessories.keys());
    const apNow = totalAccessoryPower(loadout, resource);
    const best = new Map();
    for (const [id, info] of resource) {
      if (info.category !== ACCESSORY_CATEGORY || !info.soulbound || loadout.accessories.has(id) || prices.get(id)) continue;
      if (accessorySuperseded(id, accIds, resource) || !meetsRequirements(info, ctx.player)) continue;
      const next = { ...loadout, accessories: new Map(loadout.accessories).set(id, newItem(id, resource)) };
      const ap = totalAccessoryPower(next, resource) - apNow;
      if (!(ap > 0)) continue;
      const family = accessoryFamily(id) ?? id;
      const prev = best.get(family);
      if (!prev || accessoryChainPosition(id) < accessoryChainPosition(prev.id)) best.set(family, { id, info, next, ap });
    }
    for (const { id, info, next, ap } of best.values()) {
      const note = ITEM_STATS[id]?.note;
      tryLoadout(`${cleanName(info.name ?? id)} (free quest accessory: +${ap} magical power${note ? `, ${note}` : ''})`, next);
    }
  }
  return { rows: rows.sort((a, b) => b.gain - a.gain), damage: base };
}

function ordinal(n) {
  return ['1st', '2nd', '3rd', '4th', '5th'][n - 1] ?? `${n}th`;
}

function roman(n) {
  return ['I', 'II', 'III', 'IV', 'V', 'VI', 'VII', 'VIII', 'IX', 'X'][n - 1] ?? String(n);
}

function enrichmentName(kind) {
  return kind === 'walk_speed' ? 'Speed' : titleCase(kind);
}

function titleCase(s) {
  return s.split('_').map((w) => (w ? w[0].toUpperCase() + w.slice(1) : w)).join(' ');
}
