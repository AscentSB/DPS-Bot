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

// Turn one simplified NBT item into the item state the calculator uses.
// Returns null for empty slots or items with no SkyBlock ITEM_ID.
export function toItemState(nbtItem, resource) {
  const extra = nbtItem?.tag?.ExtraAttributes;
  const id = extra?.id;
  if (!id) return null;

  let enchants = { ...(extra.enchantments ?? {}) };
  let resolvedId = id;
  // Enchanted books are keyed as ENCHANTMENT_<NAME>_<LEVEL> so they match bazaar ids.
  if (id === 'ENCHANTED_BOOK') {
    const entries = Object.entries(enchants);
    if (entries.length === 1) resolvedId = `ENCHANTMENT_${entries[0][0].toUpperCase()}_${entries[0][1]}`;
    enchants = {};
  }

  const stars = Number(extra.upgrade_level ?? extra.dungeon_item_level ?? 0);
  const info = resource?.get(resolvedId);
  return {
    id: resolvedId,
    name: info?.name ?? resolvedId,
    category: info?.category,
    rarity: info?.tier,
    stars,
    masterStars: Math.max(0, Math.min(5, stars - 5)),
    enchants,
    hpb: Number(extra.hot_potato_count ?? 0),
    reforge: extra.modifier ?? null,
  };
}
