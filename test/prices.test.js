import test from 'node:test';
import assert from 'node:assert/strict';
import zlib from 'node:zlib';
import nbt from 'prismarine-nbt';
import { fetchAuctions, smoothLowest } from '../src/api/priceFetch.js';
import { priceLookup } from '../src/api/prices.js';

const itemBytes = (id) => {
  const tag = nbt.comp({ ExtraAttributes: nbt.comp({ id: nbt.string(id) }) });
  const root = nbt.comp({ i: nbt.list(nbt.comp([{ id: nbt.short(1), Count: nbt.byte(1), tag }])) }, '');
  return zlib.gzipSync(nbt.writeUncompressed(root)).toString('base64');
};

// Fake auction house: `pages` pages, each with one BIN of TERMINATOR at a price, and per-page failure counts.
function mockAuctions(pages, failures = {}, lastUpdated = 1) {
  let active = 0;
  let calls = 0;
  let peak = 0;
  globalThis.fetch = async (url) => {
    const page = Number(new URL(url).searchParams.get('page'));
    calls++;
    active++;
    peak = Math.max(peak, active);
    await new Promise((r) => setTimeout(r, 5));
    active--;
    if ((failures[page] ?? 0) > 0) {
      failures[page]--;
      return { ok: false, status: 500, text: async () => 'down' };
    }
    const auctions = [{ bin: true, starting_bid: 1000 + page, item_bytes: itemBytes('TERMINATOR') }];
    return { ok: true, status: 200, json: async () => ({ success: true, totalPages: pages, lastUpdated, auctions }) };
  };
  return { peak: () => peak, calls: () => calls };
}

test('auction pages download a few at a time, retry, and keep the lowest BIN', async () => {
  const { peak } = mockAuctions(20, { 7: 2 });
  const entries = new Map((await fetchAuctions()).entries);
  assert.equal(entries.get('TERMINATOR'), 1000);
  assert.ok(peak() <= 6, `peak ${peak()}`);
});

test('a page that keeps failing fails the whole refresh (no partial auction house)', async () => {
  mockAuctions(5, { 3: 99 });
  await assert.rejects(fetchAuctions(), /HTTP 500/);
});

test('an unchanged auction house is not downloaded again', async () => {
  const mock = mockAuctions(20, {}, 42);
  assert.equal(await fetchAuctions(42), null);
  assert.equal(mock.calls(), 1);
  assert.equal((await fetchAuctions(41)).lastUpdated, 42);
});

test('auction prices are the median over recent downloads, for items still listed', () => {
  const history = [new Map([['A', 100], ['B', 7]]), new Map([['A', 10]]), new Map([['A', 120], ['C', 5]])];
  assert.deepEqual(new Map(smoothLowest(history)), new Map([['A', 100], ['C', 5]]));
  assert.deepEqual(smoothLowest([new Map([['A', 100]]), new Map([['A', 200]])]), [['A', 150]]);
});

test('price lookups read the current tables, so a refresh that swaps them in is seen', () => {
  const tables = { bazaar: new Map(), auctions: new Map() };
  const lookup = priceLookup(tables);
  tables.bazaar = new Map([['X', 5]]);
  assert.deepEqual(lookup.get('X'), { price: 5, source: 'BZ' });
});
