import { getJson } from './http.js';

// Look up a Minecraft account. Returns { uuid, name } or null if it does not exist.
export async function getMinecraftProfile(username) {
  const data = await getJson(`https://api.mojang.com/users/profiles/minecraft/${encodeURIComponent(username)}`);
  if (!data?.id) return null;
  return { uuid: data.id, name: data.name };
}
