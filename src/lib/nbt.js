import zlib from 'node:zlib';
import nbt from 'prismarine-nbt';

// Decode a base64, gzipped NBT blob (Hypixel inventory data or auction item_bytes)
// into a list of simplified item objects.
export function decodeItems(base64) {
  if (!base64) return [];
  const raw = zlib.gunzipSync(Buffer.from(base64, 'base64'));
  const parsed = nbt.simplify(nbt.parseUncompressed(raw));
  return Array.isArray(parsed?.i) ? parsed.i : [];
}

// Id used for pricing and lookups.
//   ENCHANTED_BOOK with one enchant -> ENCHANTMENT_<NAME>_<LEVEL> (matches bazaar ids)
//   PET -> PET_<TYPE>_<TIER>
function resolveId(extra) {
  const id = extra.id;
  if (id === 'ENCHANTED_BOOK') {
    const entries = Object.entries(extra.enchantments ?? {});
    if (entries.length === 1) return `ENCHANTMENT_${entries[0][0].toUpperCase()}_${entries[0][1]}`;
  }
  const pet = petInfo(extra);
  if (pet?.type && pet?.tier) return `PET_${pet.type}_${pet.tier}`;
  return id;
}

export function petInfo(extra) {
  if (extra?.id !== 'PET' || !extra.petInfo) return null;
  try {
    return typeof extra.petInfo === 'string' ? JSON.parse(extra.petInfo) : extra.petInfo;
  } catch {
    return null;
  }
}

// Item resource names can carry formatting like "%%light_purple%%" or "§d".
export function cleanName(name) {
  return String(name ?? '').replace(/%%[a-z_]+%%/gi, '').replace(/§./g, '').trim();
}

// Turn one simplified NBT item into the item state the calculator uses.
// Returns null for empty slots or items with no SkyBlock ITEM_ID.
export function toItemState(nbtItem, resource) {
  const extra = nbtItem?.tag?.ExtraAttributes;
  if (!extra?.id) return null;

  const id = resolveId(extra);
  const isBook = extra.id === 'ENCHANTED_BOOK';
  const stars = Number(extra.upgrade_level ?? extra.dungeon_item_level ?? 0);
  const info = resource?.get(id);
  return {
    id,
    name: cleanName(info?.name ?? id),
    category: info?.category,
    baseRarity: info?.tier ?? 'COMMON',
    recomb: Number(extra.rarity_upgrades ?? 0) > 0,
    stars: Math.min(stars, 5),
    masterStars: Math.max(0, Math.min(5, stars - 5)),
    dungeon: Number(extra.dungeon_item ?? 0) > 0 || stars > 0 ? true : undefined,
    enchants: isBook ? {} : { ...(extra.enchantments ?? {}) },
    hpb: Number(extra.hot_potato_count ?? 0),
    artOfWar: Number(extra.art_of_war_count ?? 0) > 0,
    reforge: extra.modifier ?? null,
    gems: normalizeGems(extra.gems),
    enrichment: extra.talisman_enrichment ?? null,
    pet: petInfo(extra),
  };
}

// gems NBT looks like { JASPER_0: 'PERFECT', COMBAT_0: { quality: 'FLAWLESS' }, COMBAT_0_gem: 'JASPER', unlocked_slots: [...] }
// Returns { slots: { COMBAT_0: { gem: 'JASPER', quality: 'FLAWLESS' } }, unlocked: [...] }
export function normalizeGems(raw) {
  const out = { slots: {}, unlocked: Array.isArray(raw?.unlocked_slots) ? [...raw.unlocked_slots] : [] };
  if (!raw || typeof raw !== 'object') return out;
  for (const [key, value] of Object.entries(raw)) {
    if (key === 'unlocked_slots' || key.endsWith('_gem')) continue;
    const quality = typeof value === 'string' ? value : value?.quality;
    if (!quality) continue;
    const type = key.replace(/_\d+$/, '');
    const gem = raw[`${key}_gem`] ?? type;
    out.slots[key] = { gem, quality };
  }
  return out;
}
