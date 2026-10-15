import { Worker } from 'node:worker_threads';
import { AUCTION_REFRESH_MS, BAZAAR_REFRESH_MS } from '../config/constants.js';

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

// Price lookup over { bazaar, auctions }, read at lookup time (a refresh swaps the maps in place of the old ones).
// Bazaar first, then auction house lowest BIN.
export function priceLookup(t) {
  return {
    get(id) {
      if (t.bazaar.has(id)) return { price: t.bazaar.get(id), source: 'BZ' };
      if (t.auctions.has(id)) return { price: t.auctions.get(id), source: 'AH' };
      return null;
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
