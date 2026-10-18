// Gathers more real dungeon profiles into testdata/profiles/<name>.json ({ uuid, profile }, the selected profile).
// Run with network and the API key: oracle/run.sh --net fetch-profiles.mjs [target]
//
// Candidates are the guild members of the seed players (the profiles already in testdata/profiles and their co-op
// members). A candidate is kept when its selected profile gives Catacombs level >= 20 and has at least one inventory
// blob (inventory API on, possibly only partly). Kept profiles are spread over selected class and Catacombs bands.
// Polite: at most one request per second per host, backs off on 429 and on RateLimit-Remaining / RateLimit-Reset.
// The API key is read from the mounted .env and never printed or written. Profile contents are never printed.
import fs from 'node:fs';
import path from 'node:path';
import { PROFILES_DIR, SRC, readJson } from './common.mjs';

const { cataLevelFromXp } = await import(`${SRC}/calc/formulas.js`);

const TARGET_NEW = Number(process.argv[2] ?? 28);
const MAX_PROFILE_REQUESTS = 220;
const MIN_CATA = 20;
const API = 'https://api.hypixel.net/v2';

function apiKey() {
  const file = process.env.ENV_FILE ?? '/secrets/env';
  for (const line of fs.readFileSync(file, 'utf8').split(/\r?\n/)) {
    const m = /^\s*HYPIXEL_API_KEY\s*=\s*(.*?)\s*$/.exec(line);
    if (m) return m[1].replace(/^(['"])(.*)\1$/, '$2');
  }
  throw new Error('HYPIXEL_API_KEY not found in the env file.');
}
const KEY = apiKey();

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
// One request per second per host.
const lastAt = new Map();
async function throttle(host) {
  const wait = (lastAt.get(host) ?? 0) + 1000 - Date.now();
  if (wait > 0) await sleep(wait);
  lastAt.set(host, Date.now());
}

let hypixelRequests = 0;
async function hypixel(pathAndQuery) {
  for (let attempt = 0; attempt < 6; attempt++) {
    await throttle('hypixel');
    hypixelRequests++;
    const res = await fetch(`${API}${pathAndQuery}`, { headers: { 'API-Key': KEY }, signal: AbortSignal.timeout(30000) });
    const remaining = Number(res.headers.get('ratelimit-remaining'));
    const reset = Number(res.headers.get('ratelimit-reset'));
    if (res.status === 429) {
      const retry = Number(res.headers.get('retry-after'));
      const s = Number.isFinite(retry) && retry > 0 ? retry : Number.isFinite(reset) && reset > 0 ? reset : 60;
      console.log(`  429 from Hypixel, waiting ${s + 1}s`);
      await res.arrayBuffer().catch(() => {});
      await sleep((s + 1) * 1000);
      continue;
    }
    if (Number.isFinite(remaining) && remaining <= 3 && Number.isFinite(reset)) {
      console.log(`  rate limit nearly used (${remaining} left), waiting ${reset + 1}s`);
      await sleep((reset + 1) * 1000);
    }
    if (res.status >= 500) {
      await res.arrayBuffer().catch(() => {});
      await sleep(5000 * (attempt + 1));
      continue;
    }
    const body = await res.json().catch(() => null);
    if (!res.ok || !body?.success) throw new Error(`Hypixel ${pathAndQuery.split('?')[0]} failed: HTTP ${res.status} ${body?.cause ?? ''}`);
    return body;
  }
  throw new Error('Hypixel kept failing.');
}

async function mojangName(uuid) {
  for (let attempt = 0; attempt < 4; attempt++) {
    await throttle('mojang');
    const res = await fetch(`https://sessionserver.mojang.com/session/minecraft/profile/${uuid}`, { signal: AbortSignal.timeout(15000) });
    if (res.status === 429) { await sleep(30000); continue; }
    if (res.status === 204 || res.status === 404) return null;
    if (!res.ok) { await sleep(5000); continue; }
    const data = await res.json().catch(() => null);
    return /^[A-Za-z0-9_]{2,16}$/.test(data?.name ?? '') ? data.name : null;
  }
  return null;
}

// Same blob list as inventoryBlobs in src/api/hypixel.js.
function inventoryBlobCount(member) {
  const inv = member.inventory;
  if (!inv) return { count: 0, missing: [] };
  const named = {
    inv_contents: inv.inv_contents, inv_armor: inv.inv_armor, equipment_contents: inv.equipment_contents,
    ender_chest_contents: inv.ender_chest_contents, wardrobe_contents: inv.wardrobe_contents,
    personal_vault_contents: inv.personal_vault_contents, talisman_bag: inv.bag_contents?.talisman_bag,
  };
  const blobs = Object.values(named);
  for (const bp of Object.values(inv.backpack_contents ?? {})) blobs.push(bp);
  for (const group of [member.loadout?.armor, member.loadout?.equipment]) {
    for (const set of Object.values(group ?? {})) for (const piece of Object.values(set ?? {})) if (piece?.data) blobs.push(piece);
  }
  return { count: blobs.filter((b) => b?.data).length, missing: Object.entries(named).filter(([, b]) => !b?.data).map(([k]) => k) };
}

const band = (lvl) => (lvl < 35 ? '20-34' : lvl < 45 ? '35-44' : lvl < 50 ? '45-49' : '50+');
const CLASS_KEY = { mage: 'mage', archer: 'archer', berserk: 'berserk' };

// --- Seeds ---
fs.mkdirSync(PROFILES_DIR, { recursive: true });
const existing = fs.readdirSync(PROFILES_DIR).filter((f) => f.endsWith('.json'));
const haveUuids = new Set();
const seedUuids = [];
for (const f of existing) {
  const { uuid, profile } = readJson(path.join(PROFILES_DIR, f));
  haveUuids.add(uuid);
  seedUuids.push(uuid);
  for (const [id, m] of Object.entries(profile?.members ?? {})) {
    if (id !== uuid && !m?.profile?.deletion_notice) seedUuids.push(id);
  }
}
console.log(`${existing.length} existing profiles, ${seedUuids.length} seed players`);

// --- Guild members (interleaved across guilds) ---
const guildLists = [];
const seenGuilds = new Set();
for (const uuid of seedUuids) {
  const data = await hypixel(`/guild?player=${uuid}`);
  const g = data.guild;
  if (!g || seenGuilds.has(g._id)) continue;
  seenGuilds.add(g._id);
  guildLists.push((g.members ?? []).map((m) => m.uuid).filter((u) => /^[0-9a-f]{32}$/i.test(u ?? '')));
  console.log(`guild ${guildLists.length}: ${guildLists.at(-1).length} members`);
}
const candidates = [];
const queued = new Set([...haveUuids]);
for (let i = 0; guildLists.some((l) => i < l.length); i++) {
  for (const l of guildLists) if (i < l.length && !queued.has(l[i])) { queued.add(l[i]); candidates.push(l[i]); }
}
console.log(`${candidates.length} candidates`);

// --- Profiles ---
const kept = [];
const eligibleRejected = [];
const count = { cls: new Map(), band: new Map(), pair: new Map() };
const inc = (m, k) => m.set(k, (m.get(k) ?? 0) + 1);
const fits = (e, caps) => (count.cls.get(e.cls) ?? 0) < caps.cls && (count.band.get(e.band) ?? 0) < caps.band
  && (count.pair.get(`${e.cls}|${e.band}`) ?? 0) < caps.pair;
const STRICT = { cls: Math.ceil(TARGET_NEW * 0.4), band: Math.ceil(TARGET_NEW * 0.34), pair: Math.ceil(TARGET_NEW * 0.15) };

async function keep(e) {
  const name = await mojangName(e.uuid);
  if (!name) { console.log(`  no Mojang name for a candidate, skipped`); return false; }
  const file = path.join(PROFILES_DIR, `${name}.json`);
  if (fs.existsSync(file)) return false;
  fs.writeFileSync(file, JSON.stringify({ uuid: e.uuid, profile: e.profile }));
  kept.push({ ...e, name });
  inc(count.cls, e.cls); inc(count.band, e.band); inc(count.pair, `${e.cls}|${e.band}`);
  console.log(`  kept ${name}: cata ${e.cata}, ${e.cls}, ${e.blobs} blobs${e.missing.length ? `, missing ${e.missing.join(' ')}` : ''}`);
  e.profile = null;
  return true;
}

let profileRequests = 0;
for (const uuid of candidates) {
  if (kept.length >= TARGET_NEW || profileRequests >= MAX_PROFILE_REQUESTS) break;
  profileRequests++;
  let data;
  try { data = await hypixel(`/skyblock/profiles?uuid=${uuid}`); } catch (err) { console.log(`  ${err.message}`); continue; }
  const profiles = data.profiles ?? [];
  const profile = profiles.find((p) => p.selected) ?? profiles[0];
  const member = profile?.members?.[uuid];
  if (!member) { console.log(`#${profileRequests}: no SkyBlock profile`); continue; }
  const cata = cataLevelFromXp(member.dungeons?.dungeon_types?.catacombs?.experience ?? 0);
  const { count: blobs, missing } = inventoryBlobCount(member);
  if (cata < MIN_CATA || blobs === 0) { console.log(`#${profileRequests}: cata ${cata}, ${blobs} blobs: skipped`); continue; }
  const e = { uuid, profile, cata, band: band(cata), cls: CLASS_KEY[member.dungeons?.selected_dungeon_class] ?? 'other', blobs, missing };
  console.log(`#${profileRequests}: cata ${cata} ${e.cls} eligible`);
  if (fits(e, STRICT)) await keep(e);
  else eligibleRejected.push(e);
}
// Not enough with the variety caps: take the eligible ones turned away, spread the same way.
if (kept.length < TARGET_NEW) {
  for (const caps of [{ cls: TARGET_NEW, band: TARGET_NEW, pair: Math.ceil(TARGET_NEW * 0.25) }, { cls: Infinity, band: Infinity, pair: Infinity }]) {
    for (const e of eligibleRejected) {
      if (kept.length >= TARGET_NEW) break;
      if (e.profile && fits(e, caps)) await keep(e);
    }
  }
}

console.log(`\nkept ${kept.length} new profiles (${profileRequests} profile requests, ${hypixelRequests} Hypixel requests in all)`);
console.log('by class:', Object.fromEntries(count.cls), 'by cata band:', Object.fromEntries(count.band));
