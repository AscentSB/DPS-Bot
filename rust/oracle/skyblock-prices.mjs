// skyblock-prices.mjs: golden data for the Rust price code (crates/skyblock prices.rs / price_nbt.rs).
// Serves testdata/auction-pages/{page0..N,bazaar}.json through a stubbed fetch, runs the bot's fetchAuctions /
// fetchBazaar / smoothLowest over them, and writes testdata/auction-pages/expected.json:
//   { pages, lowest: [[key, price]] (sorted by key), bazaar: [[id, price]] (sorted),
//     smooth: [[key, price]] (sorted; median over runs with 3, 4 and all pages), bench: { auctionMs, bazaarMs } }
// Run: oracle/run.sh skyblock-prices.mjs
import fs from 'node:fs';
import { TD } from './common.mjs';

const { fetchAuctions, fetchBazaar, smoothLowest } = await import('/app/src/api/priceFetch.js');
const DIR = `${TD}/auction-pages`;
const pageFiles = fs.readdirSync(DIR).filter((f) => /^page\d+\.json$/.test(f));
const N = pageFiles.length;
const data = Array.from({ length: N }, (_, i) => JSON.parse(fs.readFileSync(`${DIR}/page${i}.json`, 'utf8')));
const bazaarRaw = fs.readFileSync(`${DIR}/bazaar.json`, 'utf8');

let totalPages = N;
globalThis.fetch = async (url) => {
  const u = new URL(url);
  if (u.pathname.endsWith('/bazaar')) return { ok: true, status: 200, json: async () => JSON.parse(bazaarRaw) };
  const page = Number(u.searchParams.get('page'));
  return { ok: true, status: 200, json: async () => ({ ...data[page], totalPages }) };
};

const byKey = (a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
const runs = [];
let lowest;
let auctionMs;
for (const n of [Math.min(3, N), Math.min(4, N), N]) {
  totalPages = n;
  const t = performance.now();
  const r = await fetchAuctions();
  auctionMs = performance.now() - t;
  runs.push(new Map(r.entries));
  lowest = r.entries;
}
const t0 = performance.now();
const bazaar = await fetchBazaar();
const bazaarMs = performance.now() - t0;
const out = {
  pages: N,
  lowest: [...lowest].sort(byKey),
  bazaar: [...bazaar].sort(byKey),
  smooth: smoothLowest(runs).sort(byKey),
  bench: { auctionMs, bazaarMs },
};
fs.writeFileSync(`${DIR}/expected.json`, JSON.stringify(out));
console.log(`pages ${N}: ${out.lowest.length} lowest keys, ${out.bazaar.length} bazaar, ${out.smooth.length} smooth; auction ${auctionMs.toFixed(0)} ms, bazaar parse ${bazaarMs.toFixed(0)} ms (JSON.parse of file inside)`);
