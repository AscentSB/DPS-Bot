import { getJson } from './http.js';
import { AUCTION_REFRESH_MS, BAZAAR_REFRESH_MS } from '../config/constants.js';
import { decodeItems, toItemState } from '../lib/nbt.js';

// Prices come only from the bazaar (instant buy) and the auction house (lowest BIN).
// NPC prices are never used.

const API = 'https://api.hypixel.net/v2/skyblock';

let bazaar = new Map();
let auctions = new Map();
let bazaarAt = 0;
let auctionsAt = 0;
let loading = null;

async function refreshBazaar() {
  const data = await getJson(`${API}/bazaar`);
  if (!data?.success) throw new Error('Bazaar request failed.');
  const next = new Map();
  for (const [id, product] of Object.entries(data.products)) {
    const price = product.quick_status?.buyPrice;
    if (price > 0) next.set(id, price);
  }
  bazaar = next;
  bazaarAt = Date.now();
}

// Scan every auction page and keep the lowest BIN for each ITEM_ID.
async function refreshAuctions() {
  const first = await getJson(`${API}/auctions?page=0`);
  if (!first?.success) throw new Error('Auction house request failed.');
  const pages = [first];
  const rest = await Promise.all(
    Array.from({ length: first.totalPages - 1 }, (_, i) => getJson(`${API}/auctions?page=${i + 1}`).catch(() => null)),
  );
  pages.push(...rest.filter(Boolean));

  const next = new Map();
  for (const page of pages) {
    for (const auction of page.auctions ?? []) {
      if (!auction.bin) continue;
      let id;
      try {
        id = toItemState(decodeItems(auction.item_bytes)[0])?.id;
      } catch {
        continue;
      }
      if (!id) continue;
      const prev = next.get(id);
      if (prev === undefined || auction.starting_bid < prev) next.set(id, auction.starting_bid);
    }
  }
  auctions = next;
  auctionsAt = Date.now();
}

// Make sure prices are loaded and not stale. Safe to call often.
export async function ensurePrices() {
  const now = Date.now();
  const tasks = [];
  if (now - bazaarAt > BAZAAR_REFRESH_MS) tasks.push(refreshBazaar());
  if (now - auctionsAt > AUCTION_REFRESH_MS) tasks.push(refreshAuctions());
  if (tasks.length === 0) return;
  if (!loading) loading = Promise.all(tasks).finally(() => { loading = null; });
  // Only block when nothing has loaded yet. Otherwise refresh in the background.
  if (bazaarAt === 0 || auctionsAt === 0) await loading;
  else loading.catch((err) => console.error('Price refresh failed:', err.message));
}

export function startPriceRefresh() {
  ensurePrices().catch((err) => console.error('Initial price load failed:', err.message));
  setInterval(() => ensurePrices().catch((err) => console.error('Price refresh failed:', err.message)), BAZAAR_REFRESH_MS).unref();
}

// Price lookup used by the planner. Bazaar first, then auction house lowest BIN.
export const prices = {
  get(id) {
    if (bazaar.has(id)) return { price: bazaar.get(id), source: 'BZ' };
    if (auctions.has(id)) return { price: auctions.get(id), source: 'AH' };
    return null;
  },
};
