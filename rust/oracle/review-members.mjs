// Adversarial review vectors for the planner: mutated members / jobs / price tables run through the JS runDps.
//   oracle/run.sh review-members.mjs [--shard i/n] [--limit N]
// Writes TD/review/<id>.json = { id, member, job, prices: 'full'|'empty'|'half', result?, error? }.
import fs from 'node:fs';
import path from 'node:path';
import { TD, readJson } from './common.mjs';
import { loadBot } from './harness.mjs';
import { priceLookup } from '/app/src/api/prices.js';

const args = process.argv.slice(2);
const opt = (n, d) => { const i = args.indexOf(n); return i >= 0 ? args.splice(i, 2)[1] : d; };
const shard = (opt('--shard', '0/1')).split('/').map(Number);
const LIMIT = Number(opt('--limit', '6'));
const bot = await loadBot();
const clone = (x) => JSON.parse(JSON.stringify(x));
const BL = { power: 29, wisdom: 14, time: 5 };
const baseJob = (cls) => ({ cls, target: null, mode: 'lcm', floorMode: 'master', upgradeMode: 'all', menu: 'coins', limit: LIMIT, blessings: BL, anyChest: false });

const MUT = {
  base: (m) => m,
  noPets: (m) => { m.pets = []; },
  noItems: (m) => { m.items = []; },
  noSkills: (m) => { m.skills = null; m.slayers = null; },
  noCollections: (m) => { m.collections = null; },
  noTuning: (m) => { m.tuning = null; m.power = null; m.unlockedPowers = []; },
  noShards: (m) => { m.shards = {}; m.perks = {}; },
  cata0: (m) => { m.cataLevel = 0; m.secrets = 0; },
  cata12: (m) => { m.cataLevel = 12; },
  cata24: (m) => { m.cataLevel = 24; },
  cata30: (m) => { m.cataLevel = 30; },
  cata36: (m) => { m.cataLevel = 36; },
  cata50: (m) => { m.cataLevel = 50; m.secrets = 123456; },
  lowLevels: (m) => { m.archerLevel = 1; m.berserkLevel = 1; m.mageLevel = 1; m.combatLevel = 3; m.profileSpeed = 0; },
  dupItems: (m) => { m.items = [...m.items, ...clone(m.items)]; },
  dupWeapons: (m) => { const w = m.items.filter((x) => ['SWORD', 'BOW', 'WAND', 'LONGSWORD'].includes(x.category)); m.items = [...m.items, ...clone(w)]; },
  dupArmor: (m) => { const w = m.items.filter((x) => ['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'].includes(x.category)); m.items = [...m.items, ...clone(w)]; },
  dupEquip: (m) => { const w = m.items.filter((x) => ['NECKLACE', 'CLOAK', 'BELT', 'GLOVES', 'GAUNTLET', 'BRACELET'].includes(x.category)); m.items = [...m.items, ...clone(w)]; },
  dupAcc: (m) => { const w = m.items.filter((x) => x.category === 'ACCESSORY'); m.items = [...m.items, ...clone(w)]; },
  bank0: (m) => { m.petProfile = { ...m.petProfile, bank: 0 }; if (!m.pets.some((p) => p.type === 'GOLDEN_DRAGON')) m.pets.push({ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: null }); },
  bankMid: (m) => { m.petProfile = { ...m.petProfile, bank: 5.5e8 + 0.3 }; if (!m.pets.some((p) => p.type === 'GOLDEN_DRAGON')) m.pets.push({ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: null }); },
  gold0: (m) => { m.petProfile = { ...m.petProfile, goldCollection: 0 }; if (!m.pets.some((p) => p.type === 'GOLDEN_DRAGON')) m.pets.push({ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: null }); },
  gold9: (m) => { m.petProfile = { ...m.petProfile, goldCollection: 99999999 }; },
  noGold: (m) => { m.petProfile = { bank: m.petProfile.bank }; },
  theOne: (m) => { for (const i of m.items) if (i.category === 'NECKLACE') i.enchants = { ...i.enchants, ultimate_the_one: 4 }; },
  mythicPets: (m) => { for (const p of m.pets) { p.tier = 'MYTHIC'; p.heldItem = null; } },
  epicPets: (m) => { for (const p of m.pets) { p.tier = 'EPIC'; } },
  prismA: (m) => { m.highestAp = null; m.riftPrism = true; },
  prismB: (m) => { m.highestAp = null; m.riftPrism = false; },
  apHigh: (m) => { m.highestAp = 99999; },
  apLow: (m) => { m.highestAp = 0; },
  blades: (m) => { const W = ['HYPERION', 'ASTRAEA', 'VALKYRIE', 'SCYLLA', 'NECRON_BLADE']; const extra = []; for (const i of m.items) if (W.includes(i.id)) { const a = clone(i); a.enchants = { ...a.enchants, ultimate_wise: 5, giant_killer: 6, ender_slayer: 0 }; delete a.enchants.ender_slayer; a.reforge = 'heroic'; extra.push(a); const b = clone(i); b.enchants = { ultimate_swarm: 1, titan_killer: 6 }; b.stars = 0; extra.push(b); } m.items.push(...extra); },
  bladesOnly: (m) => { const W = ['HYPERION', 'ASTRAEA', 'VALKYRIE', 'SCYLLA', 'NECRON_BLADE', 'DARK_CLAYMORE']; const w = m.items.filter((i) => W.includes(i.id)); const a = clone(w[0] ?? m.items[0]); a.enchants = { ultimate_wise: 5 }; a.stars = 2; m.items = [...w, a]; },
  maxorBoots: (m) => { const b = m.items.find((i) => i.category === 'BOOTS'); if (b) { const x = clone(b); x.id = 'SPEED_WITHER_BOOTS'; x.name = 'Maxor Boots'; m.items.push(x); const y = clone(b); y.id = 'POWER_WITHER_BOOTS'; m.items.push(y); } },
  noBoots: (m) => { m.items = m.items.filter((i) => i.category !== 'BOOTS'); },
  noArmor: (m) => { m.items = m.items.filter((i) => !['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'].includes(i.category)); },
  noWeapons: (m) => { m.items = m.items.filter((i) => !['SWORD', 'BOW', 'WAND', 'LONGSWORD'].includes(i.category)); },
  noEquip: (m) => { m.items = m.items.filter((i) => !['NECKLACE', 'CLOAK', 'BELT', 'GLOVES', 'GAUNTLET', 'BRACELET'].includes(i.category)); },
  noAcc: (m) => { m.items = m.items.filter((i) => i.category !== 'ACCESSORY'); },
  rag: (m) => { const r = clone(m.items.find((i) => i.category === 'AXE') ?? m.items[0]); r.id = 'RAGNAROCK_AXE'; r.category = 'AXE'; r.enchants = { chimera: 5, ultimate_wise: 5 }; m.items.push(r); },
  dupPets: (m) => { m.pets = [...m.pets, ...clone(m.pets)]; },
  fakeItem: (m) => { if (m.items[0]) { const x = clone(m.items[0]); x.id = 'TOTALLY_FAKE_ITEM'; m.items.push(x); } },
  noPetProfile: (m) => { m.petProfile = {}; },
  tinyBank: (m) => { m.petProfile = { ...m.petProfile, goldCollection: 5, bank: 0 }; },
  nothing: (m) => { m.items = []; m.pets = []; m.skills = null; m.slayers = null; m.collections = null; m.tuning = null; m.power = null; m.unlockedPowers = []; m.shards = {}; m.perks = {}; },
  fewItems: (m) => { m.items = m.items.slice(0, 5); },
  weaponsOnly: (m) => { m.items = m.items.filter((x) => /SWORD|BOW|STAFF|SCEPTER|HYPERION|TERMINATOR|LIVID|DAGGER|CLAYMORE|BLADE/i.test(x.id)); },
};
const JOBV = {
  def: (j) => j,
  wither: (j) => { j.target = 'wither'; },
  dragon: (j) => { j.target = 'dragon'; },
  bogusTarget: (j) => { j.target = 'bogus'; },
  rcm: (j) => { j.mode = 'rcm'; },
  normal: (j) => { j.floorMode = 'normal'; },
  max: (j) => { j.upgradeMode = 'max'; },
  anyChest: (j) => { j.anyChest = true; },
  limit0: (j) => { j.limit = 0; },
  limit1: (j) => { j.limit = 1; },
  limit15: (j) => { j.limit = 15; },
  noBless: (j) => { j.blessings = { power: 0, wisdom: 0, time: 0 }; },
  profile: (j) => { j.menu = 'profile'; },
};
const PRICES = ['full', 'empty', 'half'];

const WIDE = args.includes('--wide');
const EXTRA = args.includes('--extra');
const BUILD = args.includes('--build');
const ONLY = opt('--only', null);
const cases = [];
const add = (profile, cls, mut, jv, prices) => (!ONLY || `${profile}__${cls}__${mut}__${jv}__${prices}`.includes(ONLY)) && cases.push({ id: `${profile}__${cls}__${mut}__${jv}__${prices}`, profile, cls, mut, jv, prices });
const ALL = fs.readdirSync(path.join(TD, 'members')).filter((f) => f.endsWith('.json')).map((f) => f.slice(0, -5)).sort();
if (WIDE) {
  for (const p of ALL) for (const cls of ['arch', 'bers', 'mage']) {
    for (const jv of ['wither', 'dragon', 'rcm', 'normal', 'anyChest', 'max']) add(p, cls, 'base', jv, 'full');
    for (const mut of ['cata24', 'cata30', 'cata36', 'noPets', 'noTuning', 'noShards']) add(p, cls, mut, 'def', 'half');
  }
}
if (BUILD) {
  for (const p of ['5reh', 'Ben_MC', 'PokeQc', 'Rwob']) for (const cls of ['arch', 'bers', 'mage']) {
    for (const mut of ['prismA', 'prismB', 'apHigh', 'apLow', 'blades', 'bladesOnly', 'maxorBoots', 'noBoots', 'noArmor', 'noWeapons', 'noEquip', 'noAcc', 'rag']) { add(p, cls, mut, 'def', 'full'); }
    add(p, cls, 'blades', 'rcm', 'full'); add(p, cls, 'maxorBoots', 'rcm', 'full'); add(p, cls, 'noArmor', 'normal', 'full');
  }
}
if (EXTRA) {
  for (const p of ['5reh', 'Ben_MC', 'PokeQc', 'Rwob']) for (const cls of ['arch', 'bers', 'mage']) {
    for (const mut of ['bank0', 'bankMid', 'gold0', 'gold9', 'noGold', 'theOne', 'mythicPets', 'epicPets']) { add(p, cls, mut, 'def', 'full'); add(p, cls, mut, 'profile', 'full'); }
  }
}
const PROFILES = WIDE || EXTRA || BUILD ? [] : ['5reh', 'Ben_MC', 'PokeQc'];
for (const p of PROFILES) for (const cls of ['arch', 'bers', 'mage']) {
  for (const mut of Object.keys(MUT)) add(p, cls, mut, 'def', 'full');
  for (const jv of Object.keys(JOBV)) if (jv !== 'def') add(p, cls, 'base', jv, 'full');
  for (const pr of ['empty', 'half']) add(p, cls, 'base', 'def', pr);
  add(p, cls, 'base', 'profile', 'empty');
  add(p, cls, 'noSkills', 'profile', 'full');
  add(p, cls, 'noCollections', 'profile', 'full');
  add(p, cls, 'noPets', 'profile', 'full');
  add(p, cls, 'cata0', 'profile', 'full');
  add(p, cls, 'cata36', 'rcm', 'full');
  add(p, cls, 'cata24', 'anyChest', 'full');
  add(p, cls, 'noPets', 'max', 'full');
  add(p, cls, 'noItems', 'rcm', 'full');
  add(p, cls, 'cata30', 'max', 'half');
}
const outDir = path.join(TD, 'review-plan');
fs.mkdirSync(outDir, { recursive: true });
const tablesFor = (kind) => {
  const s = bot.snap.tables;
  if (kind === 'full') return s;
  if (kind === 'empty') return { bazaar: new Map(), auctions: new Map() };
  return { bazaar: s.bazaar, auctions: new Map([...s.auctions].filter((_, i) => i % 2 === 0)) };
};
let n = 0;
for (let i = 0; i < cases.length; i++) {
  if (i % shard[1] !== shard[0]) continue;
  const c = cases[i];
  const member = clone(readJson(path.join(TD, 'members', `${c.profile}.json`)));
  MUT[c.mut](member);
  const job = baseJob(c.cls);
  JOBV[c.jv](job);
  const tables = tablesFor(c.prices);
  const prices = { ...priceLookup(tables), tables };
  let result; let error;
  try {
    result = await bot.runDps({ member, ...job, resource: bot.snap.resource, prices });
  } catch (e) { error = String(e?.message ?? e); }
  fs.writeFileSync(path.join(outDir, `${c.id}.json`), JSON.stringify({ id: c.id, member, job, prices: c.prices, result, error }));
  n++;
}
console.log(`shard ${shard}: ${n} of ${cases.length} cases`);
