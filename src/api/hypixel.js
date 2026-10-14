import { getJson } from './http.js';
import { ITEMS_REFRESH_MS } from '../config/constants.js';
import { decodeItems, toItemState } from '../lib/nbt.js';
import { cataLevelFromXp, skillLevelFromXp } from '../calc/formulas.js';
import { TUNING_KEYS } from '../config/accessories.js';
import { WOLF_SLAYER_XP, WOLF_SLAYER_SPEED_LEVELS, COMBAT_MAX_LEVEL, SLAYER_XP } from '../config/constants.js';
import { SHARDS, SHARDS_CUMULATIVE } from '../config/shards.js';

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

// Every inventory blob on the member that can hold gear, including wardrobe armor and
// equipment sets stored under member.loadout.
function inventoryBlobs(member) {
  const inv = member.inventory;
  if (!inv) return [];
  const blobs = [
    inv.inv_contents, inv.inv_armor, inv.equipment_contents, inv.ender_chest_contents,
    inv.wardrobe_contents, inv.personal_vault_contents, inv.bag_contents?.talisman_bag,
  ];
  for (const bp of Object.values(inv.backpack_contents ?? {})) blobs.push(bp);
  for (const group of [member.loadout?.armor, member.loadout?.equipment]) {
    for (const set of Object.values(group ?? {})) {
      for (const piece of Object.values(set ?? {})) if (piece?.data) blobs.push(piece);
    }
  }
  return blobs.filter((b) => b?.data).map((b) => b.data);
}

// Pull the player's items, Catacombs level and other DPS-relevant data from one profile response.
export function readMember(profile, uuid, resource) {
  const member = profile?.members?.[uuid];
  if (!member) throw new Error('Player is not a member of this profile.');
  const blobs = inventoryBlobs(member);
  if (blobs.length === 0) throw new Error('Inventory API is disabled for this profile. Turn it on in SkyBlock settings.');

  const items = [];
  for (const blob of blobs) {
    for (const nbtItem of decodeItems(blob)) {
      const state = toItemState(nbtItem, resource);
      if (state) items.push(state);
    }
  }
  const cataXp = member.dungeons?.dungeon_types?.catacombs?.experience ?? 0;
  const bag = member.accessory_bag_storage ?? {};
  return {
    items,
    cataLevel: cataLevelFromXp(cataXp),
    archerLevel: cataLevelFromXp(member.dungeons?.player_classes?.archer?.experience ?? 0),
    cataXp,
    secrets: Number(member.dungeons?.secrets ?? 0),
    // Skill and slayer levels for item requirements (null when the profile hides them).
    skills: member.player_data?.experience ? Object.fromEntries(Object.entries(member.player_data.experience)
      .filter(([k]) => k.startsWith('SKILL_')).map(([k, xp]) => [k.slice(6), skillLevelFromXp(xp, 60)])) : null,
    slayers: member.slayer?.slayer_bosses ? Object.fromEntries(Object.entries(SLAYER_XP)
      .map(([boss, table]) => [boss, table.filter((need) => (member.slayer.slayer_bosses[boss]?.xp ?? 0) >= need).length])) : null,
    // Combat skill (base crit chance). Skills API off: assume maxed.
    combatLevel: member.player_data?.experience?.SKILL_COMBAT == null ? COMBAT_MAX_LEVEL
      : skillLevelFromXp(member.player_data.experience.SKILL_COMBAT, COMBAT_MAX_LEVEL),
    pets: (member.pets_data?.pets ?? []).map((p) => ({ type: p.type, tier: p.tier, heldItem: p.heldItem ?? null })),
    power: typeof bag.selected_power === 'string' ? bag.selected_power.toLowerCase() : null,
    tuning: readTuning(bag.tuning?.slot_0),
    shards: readShards(member.attributes),
    perks: { ...(member.player_data?.perks ?? member.perks ?? {}) },
    // Consumed Rift Prism (+11 Accessory Power). null when the profile does not say.
    riftPrism: typeof member.rift?.access?.consumed_prism === 'boolean' ? member.rift.access.consumed_prism : null,
    highestAp: bag.highest_magical_power ?? null,
    profileSpeed: wolfSlayerSpeed(member.slayer?.slayer_bosses?.wolf?.xp ?? 0),
    // Golden Dragon perks. Missing values count as maxed.
    petProfile: {
      bank: profile.banking?.balance ?? undefined,
      goldCollection: member.collection?.GOLD_INGOT ?? undefined,
    },
  };
}

// Speed from Wolf Slayer levels.
export function wolfSlayerSpeed(xp) {
  const level = WOLF_SLAYER_XP.filter((need) => xp >= need).length;
  return WOLF_SLAYER_SPEED_LEVELS.filter((l) => level >= l).length;
}

// { allocation: { strength: 40, critDamage: 30 }, split: { strength: 0.57, critDamage: 0.43 } } or null
function readTuning(slot) {
  if (!slot || typeof slot !== 'object') return null;
  const allocation = {};
  for (const [key, value] of Object.entries(slot)) {
    const stat = TUNING_KEYS[key];
    if (stat && value > 0) allocation[stat] = (allocation[stat] ?? 0) + Number(value);
  }
  const total = Object.values(allocation).reduce((a, b) => a + b, 0);
  if (!total) return { allocation: {}, split: null };
  const split = Object.fromEntries(Object.entries(allocation).map(([k, v]) => [k, v / total]));
  return { allocation, split };
}

// Attribute levels from member.attributes.stacks: { attribute_name: syphoned shard count }.
function readShards(attributes) {
  const stacks = attributes?.stacks ?? {};
  const levels = {};
  for (const [id, shard] of Object.entries(SHARDS)) {
    const count = Number(stacks[shard.attribute] ?? 0);
    if (count > 0) levels[id] = SHARDS_CUMULATIVE[shard.rarity].filter((need) => count >= need).length;
  }
  return levels;
}
