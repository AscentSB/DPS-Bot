import {
  CLASSES, CLASS_WEAPON_CATEGORIES, ARMOR_SLOTS, EQUIPMENT_SLOTS, ACCESSORY_CATEGORY,
  MASTER_STAR_IDS, MAX_MASTER_STARS, HOT_POTATO, FUMING_POTATO, MAX_UPGRADES_LISTED,
} from '../config/constants.js';
import { ENCHANTS } from '../config/enchants.js';
import { evaluate, weightedGain, baseItemStats } from './stats.js';

const ALL_WEAPON_CATEGORIES = new Set(Object.values(CLASS_WEAPON_CATEGORIES).flat());

// Weights for the run. Choosing a single target puts the full global weight on it.
export function weightsFor(cls, target) {
  if (target === 'wither') return { wither: 100, dragon: 0 };
  if (target === 'dragon') return { wither: 0, dragon: 100 };
  return { ...CLASSES[cls].weights };
}

export function makeContext({ cls, cataLevel, secretDigit = 0, resource }) {
  return {
    cls,
    cataLevel,
    secretDigit,
    resource,
    isWeapon: (item) => ALL_WEAPON_CATEGORIES.has(item.category),
  };
}

export function newItem(id, resource) {
  const info = resource.get(id);
  return { id, name: info?.name ?? id, category: info?.category, rarity: info?.tier, masterStars: 0, enchants: {}, hpb: 0, reforge: null };
}

function cloneLoadout(l) {
  return {
    weapon: l.weapon,
    armor: { ...l.armor },
    equipment: { ...l.equipment },
    accessories: new Map(l.accessories),
    owned: new Set(l.owned),
  };
}

function score(loadout, ctx, weights) {
  const r = evaluate(loadout, ctx);
  return r.wither * weights.wither + r.dragon * weights.dragon;
}

// Pick the best owned item for each slot. Runs two passes so slots settle against each other.
export function buildLoadout(ownedItems, ctx, weights) {
  const weaponCats = CLASS_WEAPON_CATEGORIES[ctx.cls];
  const loadout = { weapon: null, armor: {}, equipment: {}, accessories: new Map(), owned: new Set() };
  const bySlot = { weapon: [] };
  for (const slot of [...ARMOR_SLOTS, ...EQUIPMENT_SLOTS]) bySlot[slot] = [];

  for (const item of ownedItems) {
    loadout.owned.add(item.id);
    if (weaponCats.includes(item.category)) bySlot.weapon.push(item);
    else if (bySlot[item.category]) bySlot[item.category].push(item);
    else if (item.category === ACCESSORY_CATEGORY) {
      const prev = loadout.accessories.get(item.id);
      if (!prev || (item.masterStars ?? 0) > (prev.masterStars ?? 0)) loadout.accessories.set(item.id, item);
    }
  }

  const setSlot = (l, slot, item) => {
    if (slot === 'weapon') l.weapon = item;
    else if (ARMOR_SLOTS.includes(slot)) l.armor[slot] = item;
    else l.equipment[slot] = item;
  };

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
  return loadout;
}

function slotOf(loadout, item) {
  if (loadout.weapon === item) return 'weapon';
  for (const [slot, it] of Object.entries(loadout.armor)) if (it === item) return slot;
  for (const [slot, it] of Object.entries(loadout.equipment)) if (it === item) return slot;
  if (loadout.accessories.get(item.id) === item) return 'accessory';
  return null;
}

function replaceItem(loadout, oldItem, newItemState) {
  const l = cloneLoadout(loadout);
  const slot = slotOf(loadout, oldItem);
  if (slot === 'weapon') l.weapon = newItemState;
  else if (slot === 'accessory') l.accessories.set(newItemState.id, newItemState);
  else if (ARMOR_SLOTS.includes(slot)) l.armor[slot] = newItemState;
  else if (slot) l.equipment[slot] = newItemState;
  return l;
}

function equippedItems(loadout) {
  return [loadout.weapon, ...Object.values(loadout.armor), ...Object.values(loadout.equipment), ...loadout.accessories.values()].filter(Boolean);
}

// Every upgrade available from the current loadout. Each upgrade only depends on
// things already in the loadout, so a master star can never land on a missing item
// and the 5th master star can never come before the 4th.
export function availableUpgrades(loadout, ctx, prices) {
  const upgrades = [];
  const { resource } = ctx;

  for (const item of equippedItems(loadout)) {
    const name = item.name ?? resource.get(item.id)?.name ?? item.id;

    // Master stars: only the next star, only on dungeon items.
    if (baseItemStats(item.id, resource).dungeon && item.masterStars < MAX_MASTER_STARS) {
      const starId = MASTER_STAR_IDS[item.masterStars];
      const p = prices.get(starId);
      if (p) {
        upgrades.push({
          label: `${ordinal(item.masterStars + 1)} Master Star on ${name}`,
          cost: p.price, source: p.source,
          apply: (l) => replaceItem(l, item, { ...item, masterStars: item.masterStars + 1 }),
        });
      }
    }

    // Enchants: any level above the current one.
    for (const [key, ench] of Object.entries(ENCHANTS)) {
      if (!ench.categories.includes(item.category)) continue;
      const current = item.enchants?.[key] ?? 0;
      for (let level = current + 1; level <= ench.values.length; level++) {
        const p = prices.get(`ENCHANTMENT_${key.toUpperCase()}_${level}`);
        if (!p) continue;
        upgrades.push({
          label: `${titleCase(key)} ${roman(level)} on ${name}`,
          cost: p.price, source: p.source,
          apply: (l) => {
            const enchants = { ...item.enchants, [key]: level };
            for (const c of ench.conflicts ?? []) delete enchants[c];
            return replaceItem(l, item, { ...item, enchants });
          },
        });
      }
    }

    // Hot potato books, then fuming potato books.
    if (ctx.isWeapon(item) && item === loadout.weapon) {
      const hpb = item.hpb ?? 0;
      const book = hpb < HOT_POTATO.max ? HOT_POTATO : hpb < HOT_POTATO.max + FUMING_POTATO.max ? FUMING_POTATO : null;
      const p = book && prices.get(book.id);
      if (p) {
        upgrades.push({
          label: `${book === HOT_POTATO ? 'Hot' : 'Fuming'} Potato Book #${hpb + 1} on ${name}`,
          cost: p.price, source: p.source,
          apply: (l) => replaceItem(l, item, { ...item, hpb: hpb + 1 }),
        });
      }
    }
  }

  // New items from the auction house / bazaar.
  const weaponCats = CLASS_WEAPON_CATEGORIES[ctx.cls];
  for (const [id, info] of resource) {
    if (loadout.owned.has(id)) continue;
    const cat = info.category;
    const isWeapon = weaponCats.includes(cat);
    const isArmor = ARMOR_SLOTS.includes(cat);
    const isEquip = EQUIPMENT_SLOTS.includes(cat);
    const isAcc = cat === ACCESSORY_CATEGORY;
    if (!isWeapon && !isArmor && !isEquip && !isAcc) continue;
    if (!hasUsefulStats(id, resource)) continue;
    const p = prices.get(id);
    if (!p) continue;
    upgrades.push({
      label: `Buy ${info.name ?? id}`,
      cost: p.price, source: p.source,
      apply: (l) => {
        const next = cloneLoadout(l);
        const item = newItem(id, resource);
        next.owned.add(id);
        if (isWeapon) next.weapon = item;
        else if (isArmor) next.armor[cat] = item;
        else if (isEquip) next.equipment[cat] = item;
        else next.accessories.set(id, item);
        return next;
      },
    });
  }
  return upgrades;
}

function hasUsefulStats(id, resource) {
  const b = baseItemStats(id, resource);
  return [b.global, b.wither, b.dragon].some((s) => Object.values(s).some((v) => v !== 0));
}

// Greedy plan: take the best weighted % DPS per million coins, apply it, repeat.
export function planUpgrades(startLoadout, ctx, prices, weights, limit = MAX_UPGRADES_LISTED) {
  const steps = [];
  let loadout = startLoadout;
  let current = evaluate(loadout, ctx);
  const startDamage = current;

  for (let i = 0; i < limit; i++) {
    let best = null;
    for (const up of availableUpgrades(loadout, ctx, prices)) {
      if (!(up.cost > 0)) continue;
      const next = up.apply(loadout);
      const after = evaluate(next, ctx);
      const gain = weightedGain(current, after, weights);
      if (gain <= 1e-9) continue;
      const perMillion = gain / (up.cost / 1_000_000);
      if (!best || perMillion > best.perMillion) best = { ...up, gain, perMillion, next, after };
    }
    if (!best) break;
    steps.push({ label: best.label, cost: best.cost, source: best.source, gain: best.gain, perMillion: best.perMillion, after: best.after });
    loadout = best.next;
    current = best.after;
  }
  return { steps, startDamage, endDamage: current, loadout };
}

function ordinal(n) {
  return ['1st', '2nd', '3rd', '4th', '5th'][n - 1] ?? `${n}th`;
}

function roman(n) {
  return ['I', 'II', 'III', 'IV', 'V', 'VI', 'VII', 'VIII', 'IX', 'X'][n - 1] ?? String(n);
}

function titleCase(s) {
  return s.split('_').map((w) => w[0].toUpperCase() + w.slice(1)).join(' ');
}
