import { floorDropKey, FULL_STAT_BOOST } from '../lib/drops.js';
import { getJson } from './http.js';
import { decodeItems, toItemState } from '../lib/nbt.js';
import { petLevel, maxPetLevel } from '../lib/pets.js';

// Downloads and decoding for prices. Runs in the price worker (src/worker/priceWorker.js), never on the planner.
// Prices come from the bazaar (instant buy) and the auction house (lowest BIN); NPC shop prices (src/data/npcShop.json)
// are added at lookup time in src/api/prices.js.

const API = 'https://api.hypixel.net/v2/skyblock';
const PAGE_CONCURRENCY = 6;
const PAGE_ATTEMPTS = 3;

// Bazaar instant-buy prices as [id, price] entries.
export async function fetchBazaar() {
  const data = await getJson(`${API}/bazaar`);
  if (!data?.success) throw new Error('Bazaar request failed.');
  // Under 1 coin is a troll sell offer (e.g. 10 stones listed at 0.1), not a real price.
  return Object.entries(data.products).map(([id, p]) => [id, p.quick_status?.buyPrice]).filter(([, price]) => price >= 1);
}

async function fetchPage(page) {
  for (let attempt = 1; ; attempt++) {
    try {
      const data = await getJson(`${API}/auctions?page=${page}`);
      if (data?.success) return data;
      throw new Error(`Auction page ${page} request failed.`);
    } catch (err) {
      if (attempt >= PAGE_ATTEMPTS) throw err;
      await new Promise((r) => setTimeout(r, 1000 * attempt));
    }
  }
}

// Lowest BIN per ITEM_ID: { lastUpdated, entries: [id, price][] }. Pages download a few at a time with retries; if a
// page still fails the whole refresh fails, so a partial auction house never replaces the last full one.
// Returns null without downloading the other pages when Hypixel has not updated the auction house since `since`.
export async function fetchAuctions(since = null) {
  const first = await fetchPage(0);
  if (since !== null && first.lastUpdated === since) return null;
  const pages = [first];
  let next = 1;
  const workers = Array.from({ length: PAGE_CONCURRENCY }, async () => {
    while (next < first.totalPages) pages.push(await fetchPage(next++));
  });
  await Promise.all(workers);

  const lowest = new Map();
  for (const page of pages) {
    for (const auction of page.auctions ?? []) {
      if (!auction.bin) continue;
      let item;
      try {
        item = toItemState(decodeItems(auction.item_bytes)[0]);
      } catch {
        continue;
      }
      if (!item?.id) continue;
      const keys = [item.id];
      // Max-level pets also get PET_<TYPE>_<TIER>_MAX (level 100, Golden Dragon 200).
      if (item.pet && petLevel(item.pet.type, item.pet.tier, item.pet.exp) >= maxPetLevel(item.pet.type)) keys.push(`${item.id}_MAX`);
      // Floor-scaled dungeon items with the full stat boost also get <ID>_FLOOR<n> for every floor up to the one they
      // dropped on, so the bot prices the piece it scores (highest floor you can enter, full boost), never a cheap
      // low-floor or low-boost one.
      if (item.floor != null && item.statBoost >= FULL_STAT_BOOST) for (let f = 1; f <= item.floor; f++) keys.push(floorDropKey(item.id, f));
      for (const key of keys) if (!(lowest.get(key) <= auction.starting_bid)) lowest.set(key, auction.starting_bid);
    }
  }
  return { lastUpdated: first.lastUpdated, entries: [...lowest] };
}

// Median lowest BIN per item over recent downloads (Maps, oldest first). Only items listed in the newest download
// get a price, so an item with no listings left is never priced.
export function smoothLowest(history) {
  const latest = history.at(-1);
  return [...latest.keys()].map((id) => {
    const seen = history.filter((m) => m.has(id)).map((m) => m.get(id)).sort((a, b) => a - b);
    const mid = seen.length >> 1;
    return [id, seen.length % 2 ? seen[mid] : (seen[mid - 1] + seen[mid]) / 2];
  });
}
