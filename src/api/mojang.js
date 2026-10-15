import { getJson } from './http.js';

// Minecraft usernames: 2 to 16 letters, digits or underscores. Checked before any request.
export const IGN_PATTERN = /^[a-zA-Z0-9_]{2,16}$/;
const UUID_PATTERN = /^[0-9a-f]{32}$/i;
export const isValidIgn = (name) => typeof name === 'string' && IGN_PATTERN.test(name);
export const isValidUuid = (uuid) => typeof uuid === 'string' && UUID_PATTERN.test(uuid);

// Look up a Minecraft account. Returns { uuid, name } or null if it does not exist.
export async function getMinecraftProfile(username) {
  if (!isValidIgn(username)) return null;
  const data = await getJson(`https://api.mojang.com/users/profiles/minecraft/${username}`);
  if (!isValidUuid(data?.id) || !isValidIgn(data?.name)) return null;
  return { uuid: data.id, name: data.name };
}
