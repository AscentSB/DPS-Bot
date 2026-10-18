// Randomised differential data for dps-calc: testdata/fuzz/<caseId>.jsonl, one { loadout: L, result: R } per line,
// where L is the case's start loadout with random edits (pets, perks, shards, extras, powers, enchants, reforges,
// gems, set pieces, dragon gear, bank, ...) and R = evaluate(L, ctx, weights) as the JS computes it. It covers the
// rarely hit paths the planner samples in evals/ miss. Seeded: the same output every run.
//   oracle/calc-picks.sh does the src copy; run this one through calc-fuzz.sh.
import fs from 'node:fs';
import path from 'node:path';
import { MEMBERS_DIR, TD, readJson } from './common.mjs';
import { loadBot, prepareCase } from './harness.mjs';
import { nonFinitePaths, serializeEval, serializeLoadout } from './serialize.mjs';
import { SRC } from './common.mjs';

const bot = await loadBot();
const S = bot.stats;
const U = bot.upgrades;
const resource = bot.snap.resource;
const { ENCHANTS } = await import(`${SRC}/config/enchants.js`);
const { POWERS } = await import(`${SRC}/config/accessories.js`);
const { SHARDS } = await import(`${SRC}/config/shards.js`);
const { ESSENCE_PERKS } = await import(`${SRC}/config/essence.js`);
const { EXTRA_UPGRADES } = await import(`${SRC}/config/upgrades.js`);
const { ARMOR_SETS } = await import(`${SRC}/config/armorSets.js`);
const { PET_ITEMS } = await import(`${SRC}/config/pets.js`);
const { allPetTypes } = await import(`${SRC}/lib/pets.js`);
const { ENRICHMENTS } = await import(`${SRC}/config/accessories.js`);

const PER_CASE = Number(process.argv[2] ?? 40);
const OUT = path.join(TD, 'fuzz');
fs.mkdirSync(OUT, { recursive: true });

function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const hash = (s) => [...s].reduce((h, c) => (Math.imul(h, 31) + c.charCodeAt(0)) >>> 0, 7);

const caseIds = fs.readdirSync(path.join(TD, 'ctx')).filter((f) => f.endsWith('.json')).map((f) => f.slice(0, -5)).sort();
const cases = new Map(readJson(path.join(TD, 'cases.json')).map((c) => [c.id, c]));
const tiers = ['COMMON', 'UNCOMMON', 'RARE', 'EPIC', 'LEGENDARY', 'MYTHIC'];
const petTypes = [...allPetTypes()];
const heldItems = [null, ...Object.keys(PET_ITEMS), 'PET_ITEM_TIER_BOOST', 'MINOS_RELIC'];
const reforgeKeys = Object.keys(S.allReforges());
const gemNames = ['JASPER', 'ONYX', 'SAPPHIRE', 'RUBY'];
const qualities = ['ROUGH', 'FLAWED', 'FINE', 'FLAWLESS', 'PERFECT'];
const enchantKeys = Object.keys(ENCHANTS);
const setPieces = ARMOR_SETS.flatMap((s) => s.pieces);
const armorSlotOf = (id) => resource.get(id)?.category;

let total = 0;
for (const id of caseIds) {
  const c = cases.get(id);
  const member = readJson(path.join(MEMBERS_DIR, `${c.profile}.json`));
  const { ctx, weights, loadout: start } = prepareCase(bot, member, c.job);
  const rand = rng(hash(id));
  const pick = (arr) => arr[Math.floor(rand() * arr.length)];
  const chance = (p) => rand() < p;
  const randPet = () => ({ type: pick(petTypes), tier: pick(tiers), heldItem: pick(heldItems) });
  const lines = [];
  for (let n = 0; n < PER_CASE; n++) {
    const l = U.cloneLoadout(start);
    const edits = [];
    // pets
    if (chance(0.7)) {
      const pets = Array.from({ length: Math.floor(rand() * 6) }, randPet);
      if (chance(0.5)) pets.push({ type: 'JELLYFISH', tier: pick(tiers), heldItem: null });
      l.pets = pets;
    }
    if (chance(0.8)) {
      const pool = [...l.pets, randPet()];
      l.activePets = { wither: chance(0.8) ? pick(pool) : null, dragon: chance(0.8) ? pick(pool) : null };
    }
    // bank
    if (chance(0.5)) l.bank = pick([0, 1e6, 123456789, 5e8, 1e9, 1.2e9, 0.37]);
    // perks / shards / extras / power
    if (chance(0.6)) { l.perks = { ...l.perks }; for (const k of Object.keys(ESSENCE_PERKS)) if (chance(0.7)) l.perks[k] = Math.floor(rand() * 6); }
    if (chance(0.6)) { l.shards = { ...l.shards }; for (const k of Object.keys(SHARDS)) if (chance(0.7)) l.shards[k] = Math.floor(rand() * 12) - 1; }
    if (chance(0.5)) l.extras = new Set(EXTRA_UPGRADES.filter(() => chance(0.5)).map((e) => e.key));
    if (chance(0.5)) l.power = pick([...Object.keys(POWERS), null, 'nope']);
    if (chance(0.3)) l.bopBonus = pick(['EXTRA_INFLICTION', 'OTHER', null]);
    // weapon edits
    const edit = (item) => {
      if (!item) return item;
      const it = { ...item, enchants: { ...item.enchants } };
      if (chance(0.5)) for (let i = 0; i < 4; i++) it.enchants[pick(enchantKeys)] = 1 + Math.floor(rand() * 7);
      if (chance(0.2)) it.enchants.ultimate_chimera = 1 + Math.floor(rand() * 5);
      if (chance(0.2)) it.enchants.ultimate_legion = 1 + Math.floor(rand() * 5);
      if (chance(0.2)) it.enchants.overload = 1 + Math.floor(rand() * 5);
      if (chance(0.2)) it.enchants.fire_aspect = 1 + Math.floor(rand() * 4);
      if (chance(0.2)) it.enchants.thunderlord = 1 + Math.floor(rand() * 8);
      if (chance(0.15)) it.enchants.ultimate_the_one = 1 + Math.floor(rand() * 6);
      if (chance(0.15)) it.enchants.first_strike = 1 + Math.floor(rand() * 6);
      if (chance(0.3)) it.reforge = pick(reforgeKeys);
      if (chance(0.3)) it.hpb = Math.floor(rand() * 16);
      if (chance(0.2)) it.artOfWar = !it.artOfWar;
      if (chance(0.3)) it.recomb = !it.recomb;
      if (chance(0.3)) it.masterStars = Math.floor(rand() * 6);
      if (chance(0.15)) it.dualWield = true;
      if (chance(0.2)) it.scrolls = chance(0.5) ? ['IMPLOSION_SCROLL', 'SHADOW_WARP_SCROLL', 'WITHER_SHIELD_SCROLL'] : ['ULTIMATE_WITHER_SCROLL'];
      if (chance(0.25)) it.gems = { slots: { COMBAT_0: { gem: pick(gemNames), quality: pick(qualities) }, JASPER_0: { gem: 'JASPER', quality: pick(qualities) } }, unlocked: [] };
      return it;
    };
    const replace = (old, next) => { if (old && next) { const r = U.replaceItem(l, old, next); Object.assign(l, r); } };
    if (chance(0.6) && l.weapon) { const w0 = l.weapon; replace(w0, edit(w0)); }
    for (const [slot, item] of Object.entries(l.armor)) if (chance(0.3)) replace(item, edit(item));
    for (const [slot, item] of Object.entries(l.equipment)) if (chance(0.3)) replace(item, edit(item));
    if (l.rag && chance(0.5)) replace(l.rag, edit(l.rag));
    // set pieces (bonuses and the Legion / pet interplay)
    if (chance(0.5)) {
      const set = pick(ARMOR_SETS);
      for (const pid of set.pieces.slice(0, 4)) {
        const cat = armorSlotOf(pid);
        if (!cat || !['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'].includes(cat)) continue;
        if (chance(0.85)) l.armor[cat] = edit(U.newItem(pid, resource));
      }
    }
    // dragon gear
    if (U.hasDragonSet(ctx) && chance(0.6)) {
      for (const [slot, item] of [['weapon', l.weapon], ...Object.entries(l.armor), ...Object.entries(l.equipment)]) {
        if (item && chance(0.4)) U.setDragonSlot(l, slot, edit(item));
      }
    }
    // accessories: recomb / enrichment
    const accs = [...l.accessories.values()];
    if (chance(0.7)) for (let i = 0; i < 6; i++) {
      const a = pick(accs);
      const next = { ...a, recomb: !a.recomb, enrichment: chance(0.6) ? pick(Object.keys(ENRICHMENTS)) : a.enrichment };
      Object.assign(l, U.replaceItem(l, a, next));
    }
    // medallion
    if (chance(0.15)) l.owned = new Set([...l.owned, 'GENERAL_MEDALLION']);
    if (chance(0.1)) { l.accessories = new Map(l.accessories); l.accessories.set('GENERAL_MEDALLION', U.newItem('GENERAL_MEDALLION', resource)); }
    let r;
    try { r = S.evaluate(l, ctx, weights); } catch (e) { console.error(id, n, 'JS threw', e.message); continue; }
    const ser = serializeEval(r);
    if (nonFinitePaths(ser).length) continue;
    lines.push(JSON.stringify({ loadout: serializeLoadout(l), result: ser }));
  }
  fs.writeFileSync(path.join(OUT, `${id}.jsonl`), lines.join('\n') + '\n');
  total += lines.length;
  console.log(id, lines.length);
}
console.log('total', total);
