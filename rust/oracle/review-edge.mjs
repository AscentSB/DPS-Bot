// Adversarial differential data for the damage-model review: degenerate contexts and loadouts (no pets, empty bags,
// unknown items, missing fields, odd levels). Writes testdata/review-edge.jsonl, one
// { ctx, loadout, result?, threw?, nonFinite? } per line. Run through review-edge.sh.
import fs from 'node:fs';
import path from 'node:path';
import { MEMBERS_DIR, TD, readJson } from './common.mjs';
import { loadBot } from './harness.mjs';
import { nonFinitePaths, serializeEval, serializeLoadout, serializeCtx } from './serialize.mjs';

const bot = await loadBot();
const S = bot.stats;
const U = bot.upgrades;
const resource = bot.snap.resource;
const cases = readJson(path.join(TD, 'cases.json'));
const PER = Number(process.argv[2] ?? 24);
const STEP = Number(process.argv[3] ?? 9);

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

function mkCtx(member, job, over) {
  const m = { ...member, ...over };
  const { cls, target = null, upgradeMode = 'all', mode = 'lcm', floorMode = 'master', blessings, anyChest = false } = job;
  const ctx = U.makeContext({
    cls, cataLevel: m.cataLevel, secrets: m.secrets, power: m.power, tuning: m.tuning, profileSpeed: m.profileSpeed,
    petProfile: m.petProfile, archerLevel: m.archerLevel, berserkLevel: m.berserkLevel, mageLevel: m.mageLevel,
    combatLevel: m.combatLevel, skills: m.skills, slayers: m.slayers, collections: m.collections,
    profileStats: m.profileStats, upgradeMode, mode, floorMode, blessings, anyChest, resource,
  });
  const weights = U.weightsFor(cls, target, mode, ctx.floorMode);
  ctx.weights = weights;
  return { ctx, weights };
}

const out = [];
let threw = 0;
const sample = cases.filter((_, i) => i % STEP === 0);
for (const c of sample) {
  const member = readJson(path.join(MEMBERS_DIR, `${c.profile}.json`));
  const rand = rng(hash(c.id));
  const pick = (a) => a[Math.floor(rand() * a.length)];
  const chance = (p) => rand() < p;
  for (let n = 0; n < PER; n++) {
    const over = {};
    if (chance(0.3)) over.collections = null;
    if (chance(0.3)) { over.skills = undefined; over.slayers = undefined; }
    if (chance(0.2)) over.skills = {};
    if (chance(0.2)) over.slayers = {};
    if (chance(0.2)) over.tuning = null;
    if (chance(0.2)) over.power = null;
    if (chance(0.3)) over.petProfile = pick([{}, { bank: 0 }, { goldCollection: 0 }, { bank: 1.5e9, goldCollection: 1e6 }]);
    if (chance(0.25)) over.cataLevel = pick([0, 1, 5, 24, 50, 60]);
    if (chance(0.2)) over.secrets = pick([0, 1, 9, 10, 1000, 99999]);
    if (chance(0.2)) { over.archerLevel = pick([0, 50]); over.berserkLevel = pick([0, 50]); over.mageLevel = pick([0, 50]); over.combatLevel = pick([0, 60]); }
    if (chance(0.2)) over.profileSpeed = pick([0, 400]);
    if (chance(0.15)) over.profileStats = pick([null, { sbLevel: 0, harpSongs: [], cakes: [] }, { cakes: [{ stat: 'strength', amount: 5 }, { stat: 'bogus', amount: 3 }] }, {}]);
    const { ctx, weights } = mkCtx(member, c.job, over);
    // start loadout from the same ctx (buildLoadout reads ctx), may throw on degenerate ctx
    let l;
    try { l = U.buildLoadout(member, ctx, weights); ctx.weights = weights; } catch (e) { continue; }
    l = U.cloneLoadout(l);
    const bad = (item) => item && { ...item };
    if (chance(0.25)) l.pets = [];
    if (chance(0.25)) l.activePets = pick([{}, { wither: null, dragon: null }, { wither: { type: 'NOPE', tier: 'LEGENDARY', heldItem: null }, dragon: { type: 'WITHER', tier: 'WEIRD', heldItem: 'FAKE' } }]);
    if (chance(0.3)) { l.pets = [{ type: 'NOPE', tier: 'EPIC', heldItem: null }, { type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: null }]; }
    if (chance(0.25)) l.accessories = new Map();
    if (chance(0.2)) l.owned = new Set();
    if (chance(0.15)) l.weapon = null;
    if (chance(0.1)) l.armor = {};
    if (chance(0.1)) l.equipment = {};
    if (chance(0.1)) l.rag = null;
    if (chance(0.15) && l.weapon) l.weapon = { ...l.weapon, id: 'TOTALLY_UNKNOWN_ITEM' };
    if (chance(0.15) && l.weapon) l.weapon = { ...l.weapon, category: undefined };
    if (chance(0.15) && l.weapon) l.weapon = { ...l.weapon, category: pick(['SWORD', 'BOW', 'LONGSWORD', 'WAND', 'HELMET', 'WEIRD']) };
    if (chance(0.15) && l.weapon) l.weapon = { ...l.weapon, reforge: pick(['nonexistent', 'Fabled', 'fabled', 'Ancient ', 'SPIRITUAL', '']) };
    if (chance(0.15) && l.weapon) l.weapon = { ...l.weapon, enchants: { ...l.weapon.enchants, sharpness: pick([0, -1, 7, 100]), ultimate_wise: 3, bogus: 5 } };
    if (chance(0.1) && l.weapon) l.weapon = { ...l.weapon, hpb: pick([0, 40, 1000]), masterStars: pick([0, 5, 9]), stars: pick([0, 5, 10]) };
    if (chance(0.1) && l.weapon) l.weapon = { ...l.weapon, gems: { slots: { A: { gem: 'NOPE', quality: 'PERFECT' }, B: { gem: 'JASPER', quality: 'NOPE' }, C: { gem: 'JASPER', quality: 'PERFECT' } }, unlocked: [] } };
    if (chance(0.1) && l.weapon) l.weapon = { ...l.weapon, baseRarity: pick(['SPECIAL', 'VERY_SPECIAL', 'ULTIMATE', 'SUPREME', 'WEIRD']), recomb: chance(0.5) };
    if (chance(0.1) && l.weapon) l.weapon = { ...l.weapon, dungeon: pick([true, undefined]) };
    if (chance(0.1)) l.bank = pick([0, -5, 1e12, 0.5]);
    if (chance(0.1)) l.power = pick(['nope', null, 'ominous', 'sighted']);
    if (chance(0.1)) l.perks = pick([{}, { whatever: 3 }]);
    if (chance(0.1)) l.shards = pick([{}, { whatever: 3 }]);
    if (chance(0.1)) l.extras = new Set(['bogus_extra']);
    if (chance(0.1)) l.witherImpact = true;
    if (chance(0.1)) l.bopBonus = pick(['EXTRA_INFLICTION', 'weird']);
    // unknown accessory
    if (chance(0.15)) { l.accessories = new Map(l.accessories); l.accessories.set('UNKNOWN_TALI', { id: 'UNKNOWN_TALI', name: 'x', baseRarity: 'COMMON', recomb: true, stars: 0, masterStars: 0, enchants: {}, hpb: 0, artOfWar: false, reforge: 'bogus', gems: { slots: {}, unlocked: [] }, enrichment: 'NOPE', scrolls: [] }); }
    let r;
    try { r = S.evaluate(l, ctx, weights); } catch (e) { threw++; out.push(JSON.stringify({ ctx: serializeCtx(ctx), loadout: serializeLoadout(l), threw: String(e.message) })); continue; }
    const ser = serializeEval(r);
    const nf = nonFinitePaths(ser);
    // pet / power / tuning picks, bag helpers
    let picks = null;
    try {
      const pk = (p) => (p ? `${p.type}:${p.tier}:${p.heldItem ?? ''}` : null);
      const sp = S.selectPet(l, ctx, weights);
      const spw = U.selectPower(l, ctx, weights);
      const bts = U.bestTuningSplit(l, ctx, weights);
      picks = {
        dpsPets: S.dpsPets(l.pets, ctx).map(pk),
        selectPet: [pk(sp.activePets.wither), pk(sp.activePets.dragon)],
        selectPower: spw.power ?? null,
        split: bts,
        counted: S.countedAccessories(l).map((i) => i.id),
        totalAp: S.totalAccessoryPower(l, resource),
        acc: ((a) => ({ stats: { ...a.stats }, ap: a.ap, tp: a.tuningPoints }))(S.accessoryStats(l, ctx, 3)),
        sa: S.saCloakMultiplier(2.5, ctx),
        gear: S.gearView(l, 'dragon') === l,
      };
    } catch (e) { picks = { threw: String(e.message) }; }
    out.push(JSON.stringify({ ctx: serializeCtx(ctx), loadout: serializeLoadout(l), picks, ...(nf.length ? { nonFinite: nf } : { result: ser }) }));
  }
}
fs.writeFileSync(path.join(TD, 'review-edge.jsonl'), out.join('\n') + '\n');
console.log('lines', out.length, 'threw', threw);
