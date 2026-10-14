import {
  CLASSES, CLASS_WEAPON_CATEGORIES, ARMOR_SLOTS, EQUIPMENT_SLOTS, ACCESSORY_CATEGORY, MASTER_STAR_IDS,
  MAX_MASTER_STARS, MAX_STARS, HOT_POTATO, FUMING_POTATO, ART_OF_WAR, RECOMBOBULATOR_ID, RAGNAROCK_ID,
  MAX_UPGRADES_LISTED, BLACKSMITH_REFORGE_COST, ARMOR_REFORGE, equipmentSlot, MAXOR_BOOTS_ID, NECRON_BOOTS_ID,
} from '../config/constants.js';
import { ENCHANTS, enchantMaxLevel } from '../config/enchants.js';
import { GEMSTONES, GEM_QUALITIES, UPGRADE_QUALITIES, SLOT_GEMS } from '../config/gemstones.js';
import { ENRICHMENTS, ENRICHABLE_RARITIES, CLASS_ENRICHMENTS, AP_OVERRIDES, RIFT_PRISM_ID } from '../config/accessories.js';
import { SHARDS, SHARD_MAX_LEVEL, SHARDS_CUMULATIVE, SHARD_HUNTING_REQ } from '../config/shards.js';
import { EXTRA_UPGRADES } from '../config/upgrades.js';
import { ESSENCE_PERKS, ESSENCE_MIN_CATA } from '../config/essence.js';
import FRAGGED from '../data/fragged.json' with { type: 'json' };
import {
  evaluate, weightedGain, baseItemStats, isDungeon, isWeaponCategory, itemRarity, canRecomb, allReforges,
  getReforge, reforgeAppliesTo, gemStat, accessorySuperseded, statRarity, itemContribution, shardLevel, totalAccessoryPower, selectPet, dpsPets,
} from './stats.js';
import { PET_ITEMS, BUY_RARITIES, TIER_BOOST_ID, KAT_UPGRADES, KAT_LEVEL_DISCOUNT, KAT_FLOWER_ID } from '../config/pets.js';
import { allPetTypes, petTiers, petStats, maxPetLevel, effectiveTier } from '../lib/pets.js';
import { cleanName } from '../lib/nbt.js';
import { dropBaseRarity } from '../lib/drops.js';
import { meetsRequirements } from '../lib/requirements.js';

// Items nobody can buy (admin and unobtainable items like the Raygun).
const UNBUYABLE_TIERS = new Set(['UNOBTAINABLE', 'ADMIN']);
const DPS_STATS = ['damage', 'strength', 'critDamage', 'critChance', 'intelligence', 'speed', 'attackSpeed'];

// Weights for the run. Choosing a single target puts the full global weight on it.
export function weightsFor(cls, target) {
  if (target === 'wither') return { wither: 100, dragon: 0 };
  if (target === 'dragon') return { wither: 0, dragon: 100 };
  return { ...CLASSES[cls].weights };
}

// profile: { cataLevel, secrets, power, tuning } from readMember.
// petProfile: { bank, goldCollection } for Golden Dragon perks (missing values count as maxed).
// skills / slayers: the player's levels for item requirement checks (omit to treat every requirement as met).
// upgradeMode: 'all' lists every level (Duplex I, II, ...); 'max' jumps straight to the max level at its cost.
export function makeContext({ cls, cataLevel, secrets = 0, power = null, tuning = null, profileSpeed = 0, petProfile = {}, archerLevel = 0, combatLevel = 0, skills, slayers, upgradeMode = 'all', resource }) {
  const player = skills !== undefined || slayers !== undefined ? { cataLevel, skills: skills ?? null, slayers: slayers ?? null } : null;
  return { cls, cataLevel, secrets, power, tuning, profileSpeed, petProfile, archerLevel, combatLevel, player, upgradeMode, resource };
}

// A clean item bought from the market. Dungeonizable gear is assumed converted and 5-starred.
export function newItem(id, resource) {
  const info = resource.get(id);
  const dungeonizable = Boolean(info?.dungeon_item || info?.dungeon_item_conversion_cost);
  return {
    id, name: cleanName(info?.name ?? id), category: info?.category, baseRarity: info ? dropBaseRarity(id, resource) : 'COMMON', recomb: false,
    stars: dungeonizable ? MAX_STARS : 0, masterStars: 0, dungeon: dungeonizable ? true : undefined,
    enchants: {}, hpb: 0, artOfWar: false, reforge: null, gems: { slots: {}, unlocked: [] }, enrichment: null,
  };
}

function cloneLoadout(l) {
  return {
    weapon: l.weapon,
    armor: { ...l.armor },
    equipment: { ...l.equipment },
    rag: l.rag,
    clearBoots: l.clearBoots ?? null,
    accessories: new Map(l.accessories),
    owned: new Set(l.owned),
    extras: new Set(l.extras),
    shards: { ...l.shards },
    perks: { ...l.perks },
    pets: [...(l.pets ?? [])],
    activePets: { ...(l.activePets ?? {}) },
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
  bySlot.BOOTS = bySlot.BOOTS.filter((b) => bossBootsAllowed(loadout.owned, b.id));
  // Clear boots: your fastest boots, worn during the clear for speed (e.g. Maxor's), swapped out for the boss.
  loadout.clearBoots = bySlot.BOOTS.reduce((best, b) => (bootsSpeed(b, ctx) > bootsSpeed(best, ctx) ? b : best), null);
  loadout = selectPet(loadout, ctx, weights);

  for (let pass = 0; pass < 2; pass++) {
    for (const [slot, options] of Object.entries(bySlot)) {
      let best = null;
      let bestScore = -Infinity;
      for (const option of options) {
        const trial = cloneLoadout(loadout);
        setSlot(trial, slot, option);
        const s = score(trial, ctx, weights);
        if (s > bestScore) { bestScore = s; best = option; }
      }
      if (best) setSlot(loadout, slot, best);
    }
  }

  return selectPet(loadout, ctx, weights);
}

// With Maxor's boots owned, the boss boots are Maxor's or Necron's only (no swapping to other boots).
function bossBootsAllowed(owned, id) {
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
}

// Stable name for where an item sits, used to group mutually exclusive upgrades.
function slotName(loadout, item) {
  if (loadout.weapon === item) return 'weapon';
  if (loadout.rag === item) return 'rag';
  for (const [slot, it] of Object.entries(loadout.armor)) if (it === item) return slot;
  if (loadout.clearBoots === item) return 'clearBoots';
  for (const [slot, it] of Object.entries(loadout.equipment)) if (it === item) return slot;
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
  if (l.accessories.get(oldItem.id) === oldItem) {
    if (next.id !== oldItem.id) l.accessories.delete(oldItem.id);
    l.accessories.set(next.id, next);
  }
  // A fragged item is a new id the player now owns, so it is never offered as a buy.
  l.owned.add(next.id);
  return l;
}

function upgradableItems(loadout) {
  const set = new Set([loadout.weapon, loadout.rag, loadout.clearBoots, ...Object.values(loadout.armor), ...Object.values(loadout.equipment), ...loadout.accessories.values()]);
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
    const weaponLike = isWeaponCategory(item.category) && (item === loadout.weapon || item === loadout.rag);

    // Master stars: next star only, dungeon gear with 5 regular stars.
    if (!isAccessory && isDungeon(item, resource) && (item.stars ?? 0) >= MAX_STARS && item.masterStars < MAX_MASTER_STARS) {
      if (ctx.upgradeMode === 'max') {
        const parts = MASTER_STAR_IDS.slice(item.masterStars, MAX_MASTER_STARS).map((id) => ({ id }));
        push(`${parts.length > 1 ? `${ordinal(item.masterStars + 1)}–${ordinal(MAX_MASTER_STARS)} Master Stars` : `${ordinal(MAX_MASTER_STARS)} Master Star`} on ${name}`, parts,
          (l) => replaceItem(l, item, { ...item, masterStars: MAX_MASTER_STARS }));
      } else {
        push(`${ordinal(item.masterStars + 1)} Master Star on ${name}`, [{ id: MASTER_STAR_IDS[item.masterStars] }],
          (l) => replaceItem(l, item, { ...item, masterStars: item.masterStars + 1 }));
      }
    }

    // Enchants: any level above the current one.
    if (!isAccessory) {
      for (const [key, ench] of Object.entries(ENCHANTS)) {
        if (!ench.categories.includes(item.category)) continue;
        if (ench.items && !ench.items.includes(item.id)) continue;
        if (ench.classes && !ench.classes.includes(cls)) continue;
        const current = item.enchants?.[key] ?? 0;
        const maxLevel = enchantMaxLevel(ench);
        for (let level = ctx.upgradeMode === 'max' ? Math.max(current + 1, maxLevel) : current + 1; level <= maxLevel; level++) {
          push(`${ench.name ?? titleCase(key.replace(/^ultimate_/, ''))} ${roman(level)} on ${name}`, [{ id: `ENCHANTMENT_${key.toUpperCase()}_${level}` }], (l) => {
            const enchants = { ...item.enchants, [key]: level };
            for (const c of ench.conflicts ?? []) delete enchants[c];
            return replaceItem(l, item, { ...item, enchants });
          }, { group: `${slot}:enchant:${key}` });
        }
      }
    }

    // Hot potato books (all remaining), then fuming potato books, and The Art of War (weapons and Ragnarock).
    if (weaponLike) {
      const hpb = item.hpb ?? 0;
      if (hpb < HOT_POTATO.max) {
        const n = HOT_POTATO.max - hpb;
        push(`${n} Hot Potato Book${n > 1 ? 's' : ''} on ${name}`, [{ id: HOT_POTATO.id, qty: n }],
          (l) => replaceItem(l, item, { ...item, hpb: HOT_POTATO.max }));
      } else if (hpb < HOT_POTATO.max + FUMING_POTATO.max) {
        const n = HOT_POTATO.max + FUMING_POTATO.max - hpb;
        push(`${n} Fuming Potato Book${n > 1 ? 's' : ''} on ${name}`, [{ id: FUMING_POTATO.id, qty: n }],
          (l) => replaceItem(l, item, { ...item, hpb: HOT_POTATO.max + FUMING_POTATO.max }));
      }
      if (!item.artOfWar) push(`The Art of War on ${name}`, [{ id: ART_OF_WAR.id }], (l) => replaceItem(l, item, { ...item, artOfWar: true }));
    }

    // Fragging: base dungeon item + 8 boss fragments -> STARRED_ version (keeps its upgrades).
    const frag = FRAGGED[item.id];
    if (frag && resource.has(frag.id)) {
      const info = resource.get(frag.id);
      push(`Frag ${name} (${frag.parts.map((p) => `${p.qty} ${titleCase(p.id.toLowerCase())}`).join(', ')})`, frag.parts,
        (l) => replaceItem(l, item, { ...item, id: frag.id, name: cleanName(info.name ?? item.name), category: info.category ?? item.category, baseRarity: info.tier ?? item.baseRarity }));
    }

    // Recombobulator.
    if (canRecomb(item, resource)) push(`Recombobulate ${name}`, [{ id: RECOMBOBULATOR_ID }], (l) => replaceItem(l, item, { ...item, recomb: true }));

    // Reforges: stones, and blacksmith reforges (random rolls, priced as the average number of rolls).
    if (!isAccessory) {
      const rarity = statRarity(itemRarity(item));
      for (const [key, reforge] of Object.entries(allReforges())) {
        if (!reforgeAllowed(key, reforge, item) || !hasDpsStats(reforge.stats?.[rarity]) && !reforge.cataStat) continue;
        if (getReforge(item.reforge) === reforge) continue;
        push(`${reforge.name} reforge on ${name}${reforge.stone ? '' : ' (blacksmith)'}`, reforgeCost(reforge, item),
          (l) => replaceItem(l, item, { ...item, reforge: key }), { group: `${slot}:reforge` });
      }
    }

    // Gemstones.
    for (const up of gemUpgrades(item, ctx)) {
      push(`${titleCase(up.quality.toLowerCase())} ${titleCase(up.gem.toLowerCase())} in ${name}${up.unlock ? ' (unlock slot)' : ''}`,
        up.parts, (l) => replaceItem(l, item, up.apply(item)), { group: `${slot}:gem:${up.slotKey}` });
    }

    // Enrichments on Legendary+ accessories.
    if (isAccessory && ENRICHABLE_RARITIES.includes(itemRarity(item))) {
      for (const kind of CLASS_ENRICHMENTS[cls]) {
        if (item.enrichment === kind) continue;
        push(`${enrichmentName(kind)} Enrichment on ${name}`, [{ id: `TALISMAN_ENRICHMENT_${ENRICHMENTS[kind].key}` }],
          (l) => replaceItem(l, item, { ...item, enrichment: kind }), { group: `${slot}:enrich` });
      }
    }
  }

  // New items from the auction house / bazaar.
  const weaponCats = CLASS_WEAPON_CATEGORIES[cls];
  const accIds = new Set(loadout.accessories.keys());
  for (const [id, info] of resource) {
    if (loadout.owned.has(id) || UNBUYABLE_TIERS.has(info.tier) || !meetsRequirements(info, ctx.player)) continue;
    const cat = info.category;
    const isRag = id === RAGNAROCK_ID;
    const isWeapon = weaponCats.includes(cat);
    const isArmor = ARMOR_SLOTS.includes(cat);
    const equipSlot = equipmentSlot(cat);
    const isEquip = Boolean(equipSlot);
    const isAcc = cat === ACCESSORY_CATEGORY;
    if (!isRag && !isWeapon && !isArmor && !isEquip && !isAcc) continue;
    if (isAcc && accessorySuperseded(id, accIds, resource)) continue;
    if (!isAcc && !isRag && !hasUsefulStats(id, resource, cls)) continue;
    if (cat === 'BOOTS' && !bossBootsAllowed(loadout.owned, id)) continue;
    // A buy can come bundled with the changes it would get straight away (reforge, recombobulator, enrichment),
    // priced and scored as one step, so an expensive accessory is weighed with its recomb and enrichment.
    const buy = (reforgeKeyName, reforge, extra = {}) => {
      const base = { ...newItem(id, resource), reforge: reforgeKeyName ?? null };
      const parts = [{ id }, ...(reforge ? reforgeCost(reforge, base) : [])];
      const labels = [`Buy ${cleanName(info.name ?? id)}`, ...(reforge ? [`${reforge.name} reforge`] : [])];
      if (extra.recomb) { parts.push({ id: RECOMBOBULATOR_ID }); labels.push('Recombobulator'); }
      if (extra.enrichment) { parts.push({ id: `TALISMAN_ENRICHMENT_${ENRICHMENTS[extra.enrichment].key}` }); labels.push(`${enrichmentName(extra.enrichment)} Enrichment`); }
      push(labels.join(' + '), parts, (l) => {
      const next = cloneLoadout(l);
      const item = { ...base, recomb: Boolean(extra.recomb), enrichment: extra.enrichment ?? null };
      next.owned.add(id);
      if (isRag) next.rag = item;
      if (isWeapon && (!isRag || !next.weapon)) next.weapon = item;
      else if (isArmor) next.armor[cat] = item;
      else if (isEquip) next.equipment[equipSlot] = item;
      else if (isAcc) next.accessories.set(id, item);
      return next;
      }, { group: isAcc ? undefined : `buy:${isRag ? 'rag' : isWeapon ? 'weapon' : equipSlot ?? cat}` });
    };
    const fresh = newItem(id, resource);
    const recombable = canRecomb(fresh, resource);
    buy();
    if (recombable) buy(null, null, { recomb: true });
    if (isAcc) {
      // Enrichments need Legendary or higher, so most come after the recombobulator.
      for (const recomb of recombable ? [false, true] : [false]) {
        if (!ENRICHABLE_RARITIES.includes(itemRarity({ ...fresh, recomb }))) continue;
        for (const kind of CLASS_ENRICHMENTS[cls]) buy(null, null, { recomb, enrichment: kind });
      }
    } else {
      // Gear can come with the best reforge for its slot (and a recombobulator).
      const best = bestReforge(loadout, ctx, fresh);
      if (best) {
        buy(best.key, best.reforge);
        if (recombable) buy(best.key, best.reforge, { recomb: true });
      }
    }
  }

  // Clear boots: any boots faster than your current clear boots, worn only during the clear.
  const clearSpeed = bootsSpeed(loadout.clearBoots, ctx);
  for (const [id, info] of resource) {
    if (info.category !== 'BOOTS' || loadout.owned.has(id) || !meetsRequirements(info, ctx.player)) continue;
    const boots = newItem(id, resource);
    if (bootsSpeed(boots, ctx) <= clearSpeed) continue;
    push(`Buy ${cleanName(info.name ?? id)} (clear boots for speed)`, [{ id }],
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
      (l) => { const next = cloneLoadout(l); next.shards[id] = SHARD_MAX_LEVEL; return next; });
  }

  // Pets (level 100, Golden Dragon 200) from the auction house. One active at a time.
  const ownedTypes = new Set(loadout.pets.map((p) => p.type));
  for (const type of allPetTypes()) {
    if (ownedTypes.has(type)) continue;
    const tier = BUY_RARITIES.find((t) => petTiers(type).includes(t));
    if (!tier) continue;
    const pet = { type, tier, heldItem: null };
    if (!dpsPets([pet], ctx).length) continue;
    const petName = titleCase(type.toLowerCase());
    const buyPet = (p, label, parts) => push(label, parts,
      (l) => { const next = cloneLoadout(l); next.pets.push(p); return selectPet(next, ctx, ctx.weights ?? weightsFor(cls)); }, { group: 'pet' });
    buyPet(pet, `Buy ${petName} Pet (${titleCase(tier.toLowerCase())}, level ${maxPetLevel(type)})`, [{ id: `PET_${type}_${tier}_MAX` }]);
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
      if (!['strength', 'critDamage', 'critChance', 'intelligence', 'attackSpeed'].some((k) => (after[k] ?? 0) > (before[k] ?? 0))) continue;
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
          (l) => { const next = cloneLoadout(l); next.perks[key] = level; return next; }, { group: `perk:${key}` });
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

// Armor only takes Ancient (ARMOR_REFORGE); everything else takes any reforge that fits.
function reforgeAllowed(key, reforge, item) {
  if (ARMOR_SLOTS.includes(item.category) && key !== ARMOR_REFORGE) return false;
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
    if (!reforgeAllowed(rKey, reforge, dummy)) continue;
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

// Greedy plan: take the upgrade with the best weighted % DPS per million coins, apply it, then recalculate every
// option from the new stats and repeat. Nothing locks: Duplex I can be followed by Duplex II and later V, and a
// later reforge or purchase can replace an earlier one, as long as each step adds damage.
export function planUpgrades(startLoadout, ctx, prices, weights, limit = MAX_UPGRADES_LISTED) {
  const steps = [];
  ctx.weights = weights;
  let loadout = startLoadout;
  let current = evaluate(loadout, ctx, weights);
  const startDamage = current;

  for (let i = 0; i < limit; i++) {
    const candidates = [];
    for (const up of availableUpgrades(loadout, ctx, prices)) {
      if (!(up.cost > 0)) continue;
      const next = up.apply(loadout);
      const after = evaluate(next, ctx, weights);
      const gain = weightedGain(current, after, weights);
      if (gain <= 1e-9) continue;
      candidates.push({ ...up, gain, perMillion: gain / (up.cost / 1_000_000), next, after });
    }
    let best = null;
    for (const c of candidates) if (!best || c.perMillion > best.perMillion) best = c;
    if (!best) break;
    steps.push({ label: best.label, cost: best.cost, source: best.source, unverified: best.unverified, gain: best.gain, perMillion: best.perMillion, after: best.after });
    loadout = selectPet(best.next, ctx, weights);
    current = evaluate(loadout, ctx, weights);
  }
  return { steps, startDamage, endDamage: current, loadout };
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
