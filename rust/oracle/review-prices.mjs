// Adversarial auction vectors for crates/skyblock prices.rs / price_nbt.rs.
//   oracle/run.sh review-prices.mjs
// Writes TD/review-skyblock/prices.json = { downloads: [{ lastUpdated, pages: [page], lowest: [[k, v]], smooth: [[k, v]] }] }
// Each download is run through the bot's fetchAuctions (stubbed fetch) and the price worker's history/smoothing.
import fs from 'node:fs';
import zlib from 'node:zlib';
import path from 'node:path';
import { createRequire } from 'node:module';
import { TD } from './common.mjs';
const nbt = createRequire('/app/src/lib/nbt.js')('prismarine-nbt');
const { fetchAuctions, smoothLowest } = await import('/app/src/api/priceFetch.js');

const T = nbt;
const S = T.string, I = T.int, D = T.double;
let seed = 12345;
const rnd = () => { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return seed / 0x7fffffff; };
const pick = (a) => a[Math.floor(rnd() * a.length)];
const list = (type, arr) => ({ type: 'list', value: { type, value: arr } });
const blob = (items) => zlib.gzipSync(nbt.writeUncompressed(T.comp({ i: list('compound', items.map((c) => c.value)) }), 'big')).toString('base64');
const item = (ea) => T.comp({ Count: T.byte(1), id: T.short(5), tag: T.comp({ display: T.comp({ Name: S('x') }), ...(ea ? { ExtraAttributes: T.comp(ea) } : {}) }) });

const EXPS = [0, 1000, 25353229, 25353230, 30000000, 1e9, 210255385, 210255384];
const TYPES = ['ENDER_DRAGON', 'GOLDEN_DRAGON', 'BLUE_WHALE', 'SHEEP', 'NOT_A_PET'];
const TIERS = ['COMMON', 'LEGENDARY', 'EPIC', 'MYTHIC'];
const ENCH = ['ultimate_wise', 'sharpness', 'ULTIMATE_JERRY', 'one_for_all'];
function randomItem() {
  const r = rnd();
  if (r < 0.15) return blob([item({ id: S(pick(['HYPERION', 'NECRON_HELMET', 'TERMINATOR', 'ASPECT_OF_THE_END'])), upgrade_level: I(5) })]);
  if (r < 0.25) return blob([item({ id: S('ENCHANTED_BOOK'), enchantments: T.comp({ [pick(ENCH)]: I(1 + Math.floor(rnd() * 5)) }) })]);
  if (r < 0.28) return blob([item({ id: S('ENCHANTED_BOOK'), enchantments: T.comp({ sharpness: I(5), one_for_all: I(1) }) })]);
  if (r < 0.45) {
    const info = { type: pick(TYPES), tier: pick(TIERS), exp: pick(EXPS), heldItem: null, candyUsed: 0 };
    if (rnd() < 0.1) delete info.tier;
    return blob([item({ id: S('PET'), petInfo: S(JSON.stringify(info)) })]);
  }
  if (r < 0.50) return blob([item({ id: S('PET'), petInfo: T.comp({ type: S(pick(TYPES)), tier: S(pick(TIERS)), exp: D(pick(EXPS)) }) })]);
  if (r < 0.52) return blob([item({ id: S('PET'), petInfo: S(pick(['', 'null', '{bad', '5', '[1]'])) })]);
  if (r < 0.75) {
    const tier = Math.floor(rnd() * 8);
    const boost = pick([50, 50, 50, 49, 0, 30]);
    const ea = { id: S(pick(['STORM_CHESTPLATE', 'NECRON_CHESTPLATE', 'SHADOW_ASSASSIN_HELMET'])) };
    if (rnd() < 0.9) { ea.item_tier = I(tier); if (rnd() < 0.9) ea.baseStatBoostPercentage = I(boost); }
    return blob([item(ea)]);
  }
  if (r < 0.78) return blob([item(null)]);
  if (r < 0.80) return blob([item({ id: S('') })]);
  if (r < 0.82) return blob([T.comp({})]);
  if (r < 0.84) return 'not base64 at all!!';
  if (r < 0.85) return '';
  if (r < 0.86) return undefined;
  if (r < 0.87) return zlib.gzipSync(Buffer.from('xx')).toString('base64');
  return blob([item({ id: S(pick(['ASPECT_OF_THE_DRAGONS', 'LIVID_DAGGER', 'JUJU_SHORTBOW', 'DUNGEON_STONE', 'CLEAN_CHESTPLATE'])) })]);
}
const hex = () => Array.from({ length: 32 }, () => '0123456789abcdef'[Math.floor(rnd() * 16)]).join('');
const universe = Array.from({ length: 700 }, () => ({ uuid: hex(), bytes: randomItem(), bin: rnd() < 0.85, base: pick([100, 1000, 12345, 5e6, 1.5e9, 0.5, 100]) }));
const downloads = [];
const history = [];
for (let d = 0; d < 5; d++) {
  // each download lists a random 75% of the universe with jittered prices; some prices tie
  const auctions = [];
  for (const u of universe) {
    if (rnd() < 0.25) continue;
    const price = rnd() < 0.3 ? u.base : Math.round(u.base * (0.8 + rnd() * 0.6) * 100) / 100;
    const a = { uuid: u.uuid, bin: u.bin, starting_bid: price, item_bytes: u.bytes };
    if (a.item_bytes === undefined) delete a.item_bytes;
    auctions.push(a);
  }
  const perPage = 150;
  const pages = [];
  for (let p = 0; p * perPage < auctions.length; p++) pages.push({ success: true, page: p, totalPages: Math.ceil(auctions.length / perPage), lastUpdated: 1000 + d, auctions: auctions.slice(p * perPage, (p + 1) * perPage) });
  globalThis.fetch = async (url) => {
    const page = Number(new URL(url).searchParams.get('page'));
    return { ok: true, status: 200, json: async () => structuredClone(pages[page]) };
  };
  const r = await fetchAuctions(null);
  history.push(new Map(r.entries));
  if (history.length > 3) history.shift();
  const smooth = smoothLowest(history);
  const byKey = (a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
  downloads.push({ lastUpdated: 1000 + d, pages, lowest: [...r.entries].sort(byKey), smooth: smooth.sort(byKey) });
}
fs.mkdirSync(`${TD}/review-skyblock`, { recursive: true });
fs.writeFileSync(`${TD}/review-skyblock/prices.json`, JSON.stringify({ downloads }));
console.log(downloads.map((d) => `${d.pages.length} pages, ${d.lowest.length} lowest, ${d.smooth.length} smooth`).join('\n'));
