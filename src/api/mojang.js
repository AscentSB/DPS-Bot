import { getJson } from './http.js';

// Minecraft usernames: 2 to 16 letters, digits or underscores. Checked before any request.
export const IGN_PATTERN = /^[a-zA-Z0-9_]{2,16}$/;
const UUID_PATTERN = /^[0-9a-f]{32}$/i;
export const isValidIgn = (name) => typeof name === 'string' && IGN_PATTERN.test(name);
export const isValidUuid = (uuid) => typeof uuid === 'string' && UUID_PATTERN.test(uuid);

// Username lookups are cached (names rarely change): found accounts for a day, unknown names for 10 minutes.
const FOUND_MS = 24 * 60 * 60 * 1000;
const MISSING_MS = 10 * 60 * 1000;
const cache = new Map();

// Look up a Minecraft account. Returns { uuid, name } or null if it does not exist. Throws if Mojang cannot be reached.
export async function getMinecraftProfile(username) {
  if (!isValidIgn(username)) return null;
  const key = username.toLowerCase();
  const now = Date.now();
  for (const [k, e] of cache) if (e.expires <= now) cache.delete(k);
  const hit = cache.get(key);
  if (hit) return hit.value;
  const data = await getJson(`https://api.mojang.com/users/profiles/minecraft/${username}`);
  const value = isValidUuid(data?.id) && isValidIgn(data?.name) ? { uuid: data.id, name: data.name } : null;
  cache.set(key, { value, expires: now + (value ? FOUND_MS : MISSING_MS) });
  return value;
}
