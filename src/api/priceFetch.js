import { getJson } from './http.js';
import { decodeItems, toItemState } from '../lib/nbt.js';
import { petLevel, maxPetLevel } from '../lib/pets.js';

// Downloads and decoding for prices. Runs in the price worker (src/worker/priceWorker.js), never on the planner.
// Prices come only from the bazaar (instant buy) and the auction house (lowest BIN). NPC prices are never used.

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

// Lowest BIN per ITEM_ID as [id, price] entries. Pages download a few at a time with retries; if a page still fails
// the whole refresh fails, so a partial auction house never replaces the last full one.
export async function fetchAuctions() {
  const first = await fetchPage(0);
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
      for (const key of keys) if (!(lowest.get(key) <= auction.starting_bid)) lowest.set(key, auction.starting_bid);
    }
  }
  return [...lowest];
}
