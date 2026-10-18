// Review data for dps-calc helpers: newItem / baseItemStats / canRecomb / isDungeon / itemRarity / hasWitherImpact for
// every id in the items resource, plus upgradesInto pairs. Writes testdata/review-helpers.jsonl.
import fs from 'node:fs';
import path from 'node:path';
import { TD } from './common.mjs';
import { loadBot } from './harness.mjs';
import { dropStatMultiplier } from '/app/src/lib/drops.js';
import { ITEM_STATS } from '/app/src/config/itemStats.js';

const bot = await loadBot();
const S = bot.stats;
const U = bot.upgrades;
const resource = bot.snap.resource;
const lines = [];
const pairs = (b) => Object.entries(b).map(([k, v]) => [k, v]);
const fin = (x) => (Number.isFinite(x) ? x : String(x));
const ser = (t) => ({ global: pairs(t.global).map(([k, v]) => [k, fin(v)]), wither: pairs(t.wither).map(([k, v]) => [k, fin(v)]), dragon: pairs(t.dragon).map(([k, v]) => [k, fin(v)]) });
const catas = [0, 24, 50, Infinity];
for (const [id, info] of resource) {
  const it = U.newItem(id, resource);
  lines.push({ k: 'new', id, item: it });
  const special = ITEM_STATS[id] || info.tiered_stats || dropStatMultiplier(id, resource) !== 1;
  const combos = special ? ['arch', 'bers', 'mage'].flatMap((c) => catas.flatMap((a) => ['master', 'normal'].map((f) => [c, a, f]))) : [['mage', 50, 'master']];
  for (const [cls, cata, floor] of combos) {
    lines.push({ k: 'base', id, cls, cata: fin(cata), floor, out: ser(S.baseItemStats(id, resource, cls, cata, floor)) });
  }
  for (const recomb of [false, true]) {
    const x = { ...it, recomb };
    lines.push({ k: 'misc', id, recomb, rarity: S.itemRarity(x), canRecomb: S.canRecomb(x, resource), dungeon: S.isDungeon(x, resource), wi: S.hasWitherImpact({ ...x, scrolls: ['ULTIMATE_WITHER_SCROLL'] }), wi3: S.hasWitherImpact({ ...x, scrolls: ['IMPLOSION_SCROLL', 'SHADOW_WARP_SCROLL', 'WITHER_SHIELD_SCROLL'] }) });
  }
  for (const r of ['COMMON', 'UNCOMMON', 'RARE', 'EPIC', 'LEGENDARY', 'MYTHIC', 'DIVINE', 'SPECIAL', 'VERY_SPECIAL', 'ULTIMATE', 'SUPREME', 'ODD']) {
    lines.push({ k: 'rar', id, base: r, out: [S.itemRarity({ id, baseRarity: r, recomb: true }), S.itemRarity({ id, baseRarity: r, recomb: false }), S.statRarity(r)] });
  }
}
fs.writeFileSync(path.join(TD, 'review-helpers.jsonl'), lines.map((l) => JSON.stringify(l)).join('\n') + '\n');
console.log(lines.length);
