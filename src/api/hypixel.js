import { getJson } from './http.js';
import { isValidUuid } from './mojang.js';
import { ITEMS_REFRESH_MS, PROFILE_CACHE_MS } from '../config/constants.js';
import { decodeItems, toItemState } from '../lib/nbt.js';
import { cataLevelFromXp, skillLevelFromXp } from '../calc/formulas.js';
import { TUNING_KEYS, MISSING_ITEMS } from '../config/accessories.js';
import { WOLF_SLAYER_XP, WOLF_SLAYER_SPEED_LEVELS, COMBAT_MAX_LEVEL, SLAYER_XP } from '../config/constants.js';
import { SB_LEVEL_XP, HARP_SONGS, CAKE_STATS } from '../config/profile.js';
import { SHARDS, SHARDS_CUMULATIVE } from '../config/shards.js';

const API = 'https://api.hypixel.net/v2';

let itemsCache = { map: null, fetchedAt: 0 };
let itemsLoading = null;

// Item resource keyed by ITEM_ID. Holds stats, category, tier and dungeon_item flag. Loaded once (callers share
// the request) and refreshed hourly in the background: a stale list is served while the new one loads.
export async function getItemsResource() {
  if (itemsCache.map && Date.now() - itemsCache.fetchedAt < ITEMS_REFRESH_MS) return itemsCache.map;
  itemsLoading ??= (async () => {
    const data = await getJson(`${API}/resources/skyblock/items`);
    if (!data?.success) throw new Error('Could not load the SkyBlock items resource.');
    const map = new Map();
    for (const item of data.items) map.set(item.id, item);
    addMissingItems(map);
    itemsCache = { map, fetchedAt: Date.now() };
    return map;
  })().finally(() => { itemsLoading = null; });
  if (itemsCache.map) {
    itemsLoading.catch((err) => console.error('Item list refresh failed:', err.message));
    return itemsCache.map;
  }
  return itemsLoading;
}

// Items the resource leaves out (Balloon Hats), unless it lists them by now.
export function addMissingItems(map) {
  for (const item of MISSING_ITEMS) if (!map.has(item.id)) map.set(item.id, item);
  return map;
}

let collectionsCache = { map: null, fetchedAt: 0 };
let collectionsLoading = null;

// Collections resource: Map id -> { name, max } (amount needed for the last tier). Same caching as the items list.
export async function getCollectionsResource() {
  if (collectionsCache.map && Date.now() - collectionsCache.fetchedAt < ITEMS_REFRESH_MS) return collectionsCache.map;
  collectionsLoading ??= (async () => {
    const data = await getJson(`${API}/resources/skyblock/collections`);
    if (!data?.success) throw new Error('Could not load the SkyBlock collections resource.');
    const map = new Map();
    for (const category of Object.values(data.collections ?? {})) {
      for (const [id, c] of Object.entries(category.items ?? {})) {
        const max = Math.max(0, ...(c.tiers ?? []).map((t) => t.amountRequired ?? 0));
        if (max > 0) map.set(id, { name: c.name ?? id, max });
      }
    }
    collectionsCache = { map, fetchedAt: Date.now() };
    return map;
  })().finally(() => { collectionsLoading = null; });
  if (collectionsCache.map) {
    collectionsLoading.catch((err) => console.error('Collections refresh failed:', err.message));
    return collectionsCache.map;
  }
  return collectionsLoading;
}

// Maxed and unmaxed collections ({ maxed, unmaxed: [{ id, name, amount, max }] }) for the co-op, or null when the
// collections API is off or the resource is not loaded.
// collection: amounts by id (coopCollection output).
export function readCollections(collection, collections) {
  if (!collections?.size || !collection) return null;
  const unmaxed = [];
  let maxed = 0;
  for (const [id, c] of collections) {
    const amount = Number(collection[id]) || 0;
    if (amount >= c.max) maxed++;
    else unmaxed.push({ id, name: c.name, amount, max: c.max });
  }
  return { maxed, unmaxed };
}

// Selected profiles by UUID: { at, profile } once loaded, or { promise } while loading (callers share one request).
const profileCache = new Map();

// The player's selected SkyBlock profile, reused for PROFILE_CACHE_MS so repeat runs do not spend the API key.
export async function getSelectedProfile(uuid) {
  if (!isValidUuid(uuid)) throw new Error('Invalid UUID.');
  const now = Date.now();
  for (const [id, e] of profileCache) if (e.at && now - e.at > PROFILE_CACHE_MS) profileCache.delete(id);
  const hit = profileCache.get(uuid);
  if (hit) return hit.promise ?? hit.profile;
  const promise = fetchSelectedProfile(uuid);
  profileCache.set(uuid, { promise });
  try {
    const profile = await promise;
    profileCache.set(uuid, { at: Date.now(), profile });
    return profile;
  } catch (err) {
    profileCache.delete(uuid);
    throw err;
  }
}

// Needs HYPIXEL_API_KEY.
async function fetchSelectedProfile(uuid) {
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
// collections (optional): getCollectionsResource() output, for The One.
export function readMember(profile, uuid, resource, collections = null) {
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
    berserkLevel: cataLevelFromXp(member.dungeons?.player_classes?.berserk?.experience ?? 0),
    mageLevel: cataLevelFromXp(member.dungeons?.player_classes?.mage?.experience ?? 0),
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
    collections: readCollections(coopCollection(profile, member), collections),
    pets: (member.pets_data?.pets ?? []).map((p) => ({ type: p.type, tier: p.tier, heldItem: p.heldItem ?? null })),
    power: typeof bag.selected_power === 'string' ? bag.selected_power.toLowerCase() : null,
    unlockedPowers: (bag.unlocked_powers ?? []).map((x) => String(x).toLowerCase()),
    tuning: readTuning(bag.tuning?.slot_0),
    shards: readShards(member.attributes),
    perks: { ...(member.player_data?.perks ?? member.perks ?? {}) },
    // Consumed Rift Prism (+11 Accessory Power). null when the profile does not say.
    riftPrism: typeof member.rift?.access?.consumed_prism === 'boolean' ? member.rift.access.consumed_prism : null,
    highestAp: bag.highest_magical_power ?? null,
    profileSpeed: wolfSlayerSpeed(member.slayer?.slayer_bosses?.wolf?.xp ?? 0),
    // Other profile stats: SkyBlock level, mastered Harp songs, active cake buffs (skills and slayers above).
    profileStats: {
      sbLevel: Math.floor((member.leveling?.experience ?? 0) / SB_LEVEL_XP),
      // Book of Progression option (EXTRA_INFLICTION = damage).
      bopBonus: member.leveling?.bop_bonus ?? null,
      harpSongs: Object.keys(HARP_SONGS).filter((k) => (member.foraging?.songs?.harp?.[`song_${k}_perfect_completions`] ?? member.quests?.harp_quest?.[`song_${k}_perfect_completions`] ?? 0) > 0),
      cakes: (member.player_data?.temp_stat_buffs ?? []).filter((b) => CAKE_STATS[b.stat_id] && (b.expire_at ?? Infinity) > Date.now())
        .map((b) => ({ stat: CAKE_STATS[b.stat_id], amount: Number(b.amount) || 0 })),
    },
    // Golden Dragon perks. Missing values count as maxed.
    petProfile: {
      bank: bankBalance(profile, member),
      // Golden Dragon counts the player's own Gold collection only.
      goldCollection: member.collection?.GOLD_INGOT ?? undefined,
    },
  };
}

// Members still on the profile. A member who left keeps an entry with profile.deletion_notice, and an invite not
// yet accepted has coop_invitation.confirmed false. Neither counts.
export function activeMembers(profile) {
  return activeMemberList(profile).length;
}

function activeMemberList(profile) {
  return Object.values(profile?.members ?? {}).filter((m) => !m?.profile?.deletion_notice && m?.profile?.coop_invitation?.confirmed !== false
    && m?.coop_invitation?.confirmed !== false);
}

// Collections for The One count for the whole co-op: every member adds their amounts, members who left included.
// null when no member shares collections (collections API off).
export function coopCollection(profile, member) {
  const shared = [member, ...Object.values(profile?.members ?? {}).filter((m) => m !== member)].filter((m) => m?.collection);
  if (!shared.length) return null;
  const total = {};
  for (const m of shared) for (const [id, amount] of Object.entries(m.collection)) total[id] = (total[id] ?? 0) + (Number(amount) || 0);
  return total;
}

// Coins in the bank for Golden Dragon. A co-op whose other members all left counts as solo. A co-op counts the co-op bank only (never a personal bank). A solo profile
// has one bank, which the API can report in either field, so the larger one counts. Bank API off: undefined.
function bankBalance(profile, member) {
  const shared = profile.banking?.balance;
  const solo = activeMembers(profile) <= 1;
  const personal = solo ? member.profile?.bank_account : null;
  if (shared == null && personal == null) return undefined;
  return Math.max(Number(shared) || 0, Number(personal) || 0);
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
