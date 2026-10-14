import { getJson } from './http.js';
import { ITEMS_REFRESH_MS } from '../config/constants.js';
import { decodeItems, toItemState } from '../lib/nbt.js';
import { cataLevelFromXp } from '../calc/formulas.js';

const API = 'https://api.hypixel.net/v2';

let itemsCache = { map: null, fetchedAt: 0 };

// Item resource keyed by ITEM_ID. Holds stats, category, tier and dungeon_item flag.
export async function getItemsResource() {
  if (itemsCache.map && Date.now() - itemsCache.fetchedAt < ITEMS_REFRESH_MS) return itemsCache.map;
  const data = await getJson(`${API}/resources/skyblock/items`);
  if (!data?.success) throw new Error('Could not load the SkyBlock items resource.');
  const map = new Map();
  for (const item of data.items) map.set(item.id, item);
  itemsCache = { map, fetchedAt: Date.now() };
  return map;
}

// Fetch the player's selected SkyBlock profile. Needs HYPIXEL_API_KEY.
export async function getSelectedProfile(uuid) {
  const key = process.env.HYPIXEL_API_KEY;
  if (!key) throw new Error('HYPIXEL_API_KEY is not set.');
  const data = await getJson(`${API}/skyblock/profiles?uuid=${uuid}`, { headers: { 'API-Key': key } });
  if (!data?.success) throw new Error(data?.cause ?? 'Hypixel API request failed.');
  const profiles = data.profiles ?? [];
  if (profiles.length === 0) return null;
  return profiles.find((p) => p.selected) ?? profiles[0];
}

// Every inventory blob on the member that can hold gear.
function inventoryBlobs(inv) {
  if (!inv) return [];
  const blobs = [
    inv.inv_contents, inv.inv_armor, inv.equipment_contents, inv.ender_chest_contents,
    inv.wardrobe_contents, inv.personal_vault_contents, inv.bag_contents?.talisman_bag,
  ];
  for (const bp of Object.values(inv.backpack_contents ?? {})) blobs.push(bp);
  return blobs.filter((b) => b?.data).map((b) => b.data);
}

// Pull the player's items and Catacombs level from one profile response.
export function readMember(profile, uuid, resource) {
  const member = profile?.members?.[uuid];
  if (!member) throw new Error('Player is not a member of this profile.');
  const blobs = inventoryBlobs(member.inventory);
  if (blobs.length === 0) throw new Error('Inventory API is disabled for this profile. Turn it on in SkyBlock settings.');

  const items = [];
  for (const blob of blobs) {
    for (const nbtItem of decodeItems(blob)) {
      const state = toItemState(nbtItem, resource);
      if (state) items.push(state);
    }
  }
  const cataXp = member.dungeons?.dungeon_types?.catacombs?.experience ?? 0;
  return { items, cataLevel: cataLevelFromXp(cataXp), cataXp };
}
