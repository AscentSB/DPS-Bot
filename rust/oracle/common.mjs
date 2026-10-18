// Shared paths and helpers for the oracle scripts. They run in the dps-bot:latest image (see run.sh), where the JS
// bot sources are mounted at /app/src and rust/testdata at /td. Both can be overridden with JS_SRC / TD.
import fs from 'node:fs';
import path from 'node:path';

export const TD = process.env.TD ?? '/td';
export const SRC = process.env.JS_SRC ?? '/app/src';

export const PROFILES_DIR = path.join(TD, 'profiles');
export const MEMBERS_DIR = path.join(TD, 'members');
export const CTX_DIR = path.join(TD, 'ctx');
export const EVALS_DIR = path.join(TD, 'evals');

// Plan steps per coin plan (PLAN_STEPS in src/commands/dps.js).
export const PLAN_STEPS = 40;
export const DEFAULT_BLESSINGS = { power: 29, wisdom: 14, time: 5 };

export function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

export function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(value));
}

// Profile names (file names without .json), in code-unit order so every script walks them the same way.
export function profileNames() {
  return fs.readdirSync(PROFILES_DIR).filter((f) => f.endsWith('.json')).map((f) => f.slice(0, -5)).sort();
}

// testdata/snapshot.json as the JS bot holds it: the items resource and collections as Maps (API order), the price
// tables as Maps, plus the fixed `now` used for readMember.
let snapshotCache = null;
export function loadSnapshot() {
  if (snapshotCache) return snapshotCache;
  const s = readJson(path.join(TD, 'snapshot.json'));
  if (typeof s.now !== 'number') throw new Error('snapshot.json has no "now": run snapshot.mjs first.');
  snapshotCache = {
    now: s.now,
    resource: new Map(s.items),
    collections: new Map(s.collections),
    tables: { bazaar: new Map(s.bazaar), auctions: new Map(s.auctions) },
  };
  return snapshotCache;
}

// Freeze Date.now at the snapshot time (readMember filters cake buffs by it). Call before importing JS bot modules.
export function freezeNow(now) {
  Date.now = () => now;
}
