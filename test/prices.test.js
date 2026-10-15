import test from 'node:test';
import assert from 'node:assert/strict';
import zlib from 'node:zlib';
import nbt from 'prismarine-nbt';
import { fetchAuctions } from '../src/api/priceFetch.js';
import { priceLookup } from '../src/api/prices.js';

const itemBytes = (id) => {
  const tag = nbt.comp({ ExtraAttributes: nbt.comp({ id: nbt.string(id) }) });
  const root = nbt.comp({ i: nbt.list(nbt.comp([{ id: nbt.short(1), Count: nbt.byte(1), tag }])) }, '');
  return zlib.gzipSync(nbt.writeUncompressed(root)).toString('base64');
};

// Fake auction house: `pages` pages, each with one BIN of TERMINATOR at a price, and per-page failure counts.
function mockAuctions(pages, failures = {}) {
  let active = 0;
  let peak = 0;
  globalThis.fetch = async (url) => {
    const page = Number(new URL(url).searchParams.get('page'));
    active++;
    peak = Math.max(peak, active);
    await new Promise((r) => setTimeout(r, 5));
    active--;
    if ((failures[page] ?? 0) > 0) {
      failures[page]--;
      return { ok: false, status: 500, text: async () => 'down' };
    }
    const auctions = [{ bin: true, starting_bid: 1000 + page, item_bytes: itemBytes('TERMINATOR') }];
    return { ok: true, status: 200, json: async () => ({ success: true, totalPages: pages, auctions }) };
  };
  return () => peak;
}

test('auction pages download a few at a time, retry, and keep the lowest BIN', async () => {
  const peak = mockAuctions(20, { 7: 2 });
  const entries = new Map(await fetchAuctions());
  assert.equal(entries.get('TERMINATOR'), 1000);
  assert.ok(peak() <= 6, `peak ${peak()}`);
});

test('a page that keeps failing fails the whole refresh (no partial auction house)', async () => {
  mockAuctions(5, { 3: 99 });
  await assert.rejects(fetchAuctions(), /HTTP 500/);
});

test('price lookups read the current tables, so a refresh that swaps them in is seen', () => {
  const tables = { bazaar: new Map(), auctions: new Map() };
  const lookup = priceLookup(tables);
  tables.bazaar = new Map([['X', 5]]);
  assert.deepEqual(lookup.get('X'), { price: 5, source: 'BZ' });
});
