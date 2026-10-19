import { Worker } from 'node:worker_threads';
import { AUCTION_REFRESH_MS, BAZAAR_REFRESH_MS } from '../config/constants.js';
// NPC shop offers (NEU repo, `npm run update-data`).
import NPC_SHOP from '../data/npcShop.json' with { type: 'json' };

// Price cache for the planner. Downloads run in a separate price worker (src/worker/priceWorker.js), and each
// source has at most one refresh in flight: callers share it instead of starting another.

const REFRESH_MS = { bazaar: BAZAAR_REFRESH_MS, auctions: AUCTION_REFRESH_MS };
const tables = { bazaar: new Map(), auctions: new Map() };
const loadedAt = { bazaar: 0, auctions: 0 };
const inFlight = {};
const pending = new Map();
let worker = null;
let nextId = 1;

function priceWorker() {
  if (worker) return worker;
  worker = new Worker(new URL('../worker/priceWorker.js', import.meta.url));
  worker.unref();
  worker.on('message', ({ id, entries, error }) => {
    const job = pending.get(id);
    pending.delete(id);
    if (error) job?.reject(new Error(error));
    else job?.resolve(entries);
  });
  worker.on('exit', () => {
    for (const job of pending.values()) job.reject(new Error('Price worker stopped.'));
    pending.clear();
    worker = null;
  });
  return worker;
}

function refresh(kind) {
  inFlight[kind] ??= new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    priceWorker().postMessage({ id, kind });
  }).then((entries) => {
    // null: the auction house has not changed, so the current table stays.
    if (entries) tables[kind] = new Map(entries);
    loadedAt[kind] = Date.now();
  }).finally(() => { delete inFlight[kind]; });
  return inFlight[kind];
}

// Make sure prices are loaded and not stale. Safe to call often: waits only until each source has loaded once,
// later refreshes run in the background.
export async function ensurePrices() {
  const waits = [];
  for (const kind of Object.keys(tables)) {
    if (Date.now() - loadedAt[kind] <= REFRESH_MS[kind]) continue;
    const job = refresh(kind);
    if (loadedAt[kind] === 0) waits.push(job);
    else job.catch((err) => console.error(`Price refresh (${kind}) failed:`, err.message));
  }
  await Promise.all(waits);
}

// True once both price sources have loaded at least once (commands answer "try again" until then).
export function pricesReady() {
  return loadedAt.bazaar > 0 && loadedAt.auctions > 0;
}

export function startPriceRefresh() {
  const tick = () => ensurePrices().catch((err) => console.error('Price load failed:', err.message));
  tick();
  setInterval(tick, Math.min(BAZAAR_REFRESH_MS, AUCTION_REFRESH_MS)).unref();
}

// Cheapest NPC shop offer for `id`: { price, npc: [{ id, npc }] } (the NPC purchases: this item first, then any part
// of its cost that is also bought from an NPC), or null. Item parts of a cost are priced like any item; a part with
// no price (bits, copper, North Stars, motes) rules the offer out. `seen` holds the items being priced, so a shop
// loop (A costs B, B costs A) ends.
function npcBuy(id, t, seen) {
  if (!Object.hasOwn(NPC_SHOP, id) || seen.has(id)) return null;
  seen.add(id);
  let best = null;
  for (const offer of NPC_SHOP[id]) {
    let price = offer.coins;
    const npc = [{ id, npc: offer.npc }];
    let priced = true;
    for (const part of offer.items ?? []) {
      const p = cheapest(part.id, t, seen);
      if (!p) { priced = false; break; }
      price += p.price * part.qty;
      if (p.npc) npc.push(...p.npc);
    }
    if (priced && (!best || price < best.price)) best = { price, npc };
  }
  seen.delete(id);
  return best;
}

// Bazaar instant buy, else auction house lowest BIN; an NPC that sells it for less wins. NPC sell prices (what an
// NPC pays you) are never used.
function cheapest(id, t, seen = new Set()) {
  const market = t.bazaar.has(id) ? { price: t.bazaar.get(id), source: 'BZ' }
    : t.auctions.has(id) ? { price: t.auctions.get(id), source: 'AH' } : null;
  const npc = npcBuy(id, t, seen);
  return npc && (!market || npc.price < market.price) ? { price: npc.price, source: 'NPC', npc: npc.npc } : market;
}

// Price lookup over { bazaar, auctions }, read at lookup time (a refresh swaps the maps in place of the old ones).
// { price, source: 'BZ' | 'AH' | 'NPC', npc?: [{ id, npc }] } or null.
export function priceLookup(t) {
  return {
    get(id) {
      return cheapest(id, t);
    },
  };
}

// Live prices.
export const prices = priceLookup(tables);

// Prices frozen for one plan (a refresh swaps in new tables and never edits these), so every step and every
// evaluation worker prices the same way.
export function snapshotPrices() {
  const snapshot = { bazaar: tables.bazaar, auctions: tables.auctions };
  return { ...priceLookup(snapshot), tables: snapshot };
}
