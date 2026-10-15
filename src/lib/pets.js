import PET_DATA from '../data/pets.json' with { type: 'json' };
import { PET_ITEMS, PET_PERKS, TIER_BOOST_ID } from '../config/pets.js';

const TIER_ORDER = ['COMMON', 'UNCOMMON', 'RARE', 'EPIC', 'LEGENDARY', 'MYTHIC'];

// Rarity the pet plays at: one higher with a Tier Boost (never up to Mythic).
export function effectiveTier(pet) {
  if (pet.heldItem !== TIER_BOOST_ID) return pet.tier;
  const next = TIER_ORDER[TIER_ORDER.indexOf(pet.tier) + 1];
  return next && next !== 'MYTHIC' ? next : pet.tier;
}

export function maxPetLevel(type) {
  return PET_DATA.custom[type]?.maxLevel ?? 100;
}

// Pet level from total pet XP.
export function petLevel(type, tier, exp) {
  const offset = PET_DATA.rarityOffset[tier] ?? 0;
  const custom = PET_DATA.custom[type];
  const table = [...PET_DATA.levels.slice(offset, offset + 99), ...(custom?.type === 1 ? custom.levels : [])];
  const max = maxPetLevel(type);
  let level = 1;
  let remaining = exp ?? 0;
  for (const need of table) {
    if (level >= max || remaining < need) break;
    remaining -= need;
    level++;
  }
  return level;
}

export function petData(type, tier) {
  return PET_DATA.stats[type]?.[tier] ?? null;
}

export function petTiers(type) {
  return Object.keys(PET_DATA.stats[type] ?? {});
}

export function allPetTypes() {
  return Object.keys(PET_DATA.stats);
}

// 0..1 progress used to interpolate between level-1 and max stats.
function progress(data, type, level) {
  if (data.curve) {
    // "101:200:1": levels 101-200 map to stat levels 1-100.
    const [from, to] = data.curve.split(':').map(Number);
    return Math.max(0, Math.min(1, (level - from) / (to - from)));
  }
  return Math.max(0, Math.min(1, (level - 1) / 99));
}

function lerp(a, b, t) {
  return a + (b - a) * t;
}

// Stats one pet gives: { stats: {...}, dragon: {...}, petStrength, potionBoost }.
// pet: { type, tier, level, heldItem }. profile: { bank, goldCollection }.
export function petStats(pet, profile = {}) {
  const data = petData(pet.type, effectiveTier(pet));
  if (!data) return { stats: {}, dragon: {}, petStrength: 0, potionBoost: 0 };
  const t = progress(data, pet.type, pet.level ?? maxPetLevel(pet.type));
  const base = {};
  for (const key of new Set([...Object.keys(data.min), ...Object.keys(data.max)])) base[key] = lerp(data.min[key] ?? 0, data.max[key] ?? 0, t);
  const n = data.otherMax.map((v, i) => lerp(data.otherMin[i] ?? 0, v, t));
  const perk = PET_PERKS[pet.type]?.(n, { bank: profile.bank ?? Infinity, goldCollection: profile.goldCollection ?? 1e9 }) ?? {};

  // Perk strength that counts as the pet's own stat (boosted by pet items, copied by Chimera).
  if (perk.petStrength) base.strength = (base.strength ?? 0) + perk.petStrength;
  const item = PET_ITEMS[pet.heldItem];
  // Tyrant (T-Rex) makes pet item stats stronger.
  const itemBoost = 1 + (perk.special?.itemBoost ?? 0);
  for (const [key, m] of Object.entries(item?.mult ?? {})) if (base[key]) base[key] *= 1 + (m - 1) * itemBoost;
  for (const [key, v] of Object.entries(item?.flat ?? {})) base[key] = (base[key] ?? 0) + v * itemBoost;

  // special: perk effects that are not pet stats (not copied by Chimera), applied in rawStats.
  const { petStrength, potionBoost = 0, dragon = {}, wither = {}, special = {}, ...extra } = perk;
  return { stats: { ...base, ...addTo(base, extra) }, dragon, wither, special, petStrength: base.strength ?? 0, potionBoost };
}

function addTo(base, extra) {
  const out = {};
  for (const [k, v] of Object.entries(extra)) out[k] = (base[k] ?? 0) + v;
  return out;
}
