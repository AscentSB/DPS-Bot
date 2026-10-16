import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Running total of /dps runs, kept in usage.json next to package.json (USAGE_FILE to move it). Each time the total
// gains a digit (10, 100, 1,000, ...) the bot owner gets a DM: OWNER_ID if set, else the app's owner (or team owner)
// from the Developer Portal. The DM needs DISCORD_TOKEN; without it the milestone is only logged.

const API = 'https://discord.com/api/v10';
const DEFAULT_FILE = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'usage.json');

// True when n is the first number with its digit count (10, 100, 1000, ...).
export function isNewDigit(n) {
  return n >= 10 && /^10*$/.test(String(n));
}

export function createUsageCounter({ file = process.env.USAGE_FILE || DEFAULT_FILE, notify = notifyOwner } = {}) {
  let total = null;
  let loading = null;
  let writing = Promise.resolve();

  const load = () => loading ??= fs.readFile(file, 'utf8')
    .then((text) => { total = Math.max(0, Math.floor(Number(JSON.parse(text).total) || 0)); })
    .catch((err) => { if (err.code !== 'ENOENT') console.error('usage.json unreadable, counting from 0:', err.message); total = 0; });

  // Writes run one at a time; each one writes the latest total through a temp file, so a crash never leaves half a file.
  const save = () => writing = writing.then(async () => {
    const tmp = `${file}.tmp`;
    await fs.writeFile(tmp, JSON.stringify({ total }) + '\n');
    await fs.rename(tmp, file);
  }).catch((err) => console.error('usage.json write failed:', err.message));

  return {
    async record() {
      await load();
      const n = ++total;
      save();
      if (isNewDigit(n)) notify(`/dps has been run ${n.toLocaleString('en-US')} times.`).catch((err) => console.error('Usage milestone DM failed:', err.message));
      return n;
    },
    async total() { await load(); return total; },
    flush: () => writing,
  };
}

async function discord(route, token, init = {}) {
  const res = await fetch(`${API}${route}`, { ...init, headers: { Authorization: `Bot ${token}`, 'Content-Type': 'application/json' }, signal: AbortSignal.timeout(15_000) });
  if (!res.ok) throw new Error(`${route}: HTTP ${res.status}`);
  return res.json();
}

let ownerId = null;
async function notifyOwner(text) {
  console.log(text);
  const token = process.env.DISCORD_TOKEN?.trim();
  if (!token) return;
  ownerId ??= process.env.OWNER_ID?.trim() || await discord('/oauth2/applications/@me', token)
    .then((app) => app.team?.owner_user_id ?? app.owner?.id);
  if (!ownerId) throw new Error('No owner found. Set OWNER_ID in .env.');
  const channel = await discord('/users/@me/channels', token, { method: 'POST', body: JSON.stringify({ recipient_id: ownerId }) });
  await discord(`/channels/${channel.id}/messages`, token, { method: 'POST', body: JSON.stringify({ content: text, allowed_mentions: { parse: [] } }) });
}

export const usage = createUsageCounter();
