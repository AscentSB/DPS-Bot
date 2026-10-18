// Golden data for the dps-calc tests (part 1 of the calc port): testdata/picks/<caseId>.json for every ctx case,
// plus testdata/picks/global.json. Run through calc-picks.sh (it needs a copy of src that exports the internals).
//
// Per case (start loadout = the first line of evals/<case>.jsonl, rebuilt here from the member):
//   items:     contribution / raw stats / accessory stats / accessory power of every distinct gear item and the first
//              accessories, for secret digit 0 and for the medallion digit
//   bag:       accessoryStats(start, ctx, extra) for a few powers and extra tuning points, counted accessories, AP
//   shards:    shardStats(start)
//   tuning:    tuningAllocation over a grid
//   picks:     activeExtras, dpsPets, selectPet (from the start loadout and from no active pets), selectPower,
//              bestTuningSplit
//   ops:       loadout edits (replaceItem, setSlot, setDragonSlot) with the resulting loadout
//   misc:      slotName of every item, upgradableItems, gearView(dragon), loadoutKey equalities
import fs from 'node:fs';
import path from 'node:path';
import { MEMBERS_DIR, TD, readJson, writeJson } from './common.mjs';
import { loadBot, prepareCase } from './harness.mjs';
import { plain, serializeLoadout } from './serialize.mjs';

const bot = await loadBot();
const S = bot.stats;
const U = bot.upgrades;
const resource = bot.snap.resource;

const cases = readJson(path.join(TD, 'cases.json'));
const byId = new Map(cases.map((c) => [c.id, c]));
const OUT = path.join(TD, 'picks');
fs.mkdirSync(OUT, { recursive: true });

const caseIds = fs.readdirSync(path.join(TD, 'ctx')).filter((f) => f.endsWith('.json')).map((f) => f.slice(0, -5)).sort();

const triple = (t) => ({ global: { ...t.global }, wither: { ...t.wither }, dragon: { ...t.dragon } });
const obj = (o) => ({ ...o });
const sdOf = (ctx) => {
  // secretDigits(ctx.secrets)
  const s = ctx.secrets;
  if (!(s > 0)) return 0;
  return Math.min(String(Math.floor(s)).length, 5);
};

for (const id of caseIds) {
  const c = byId.get(id);
  const member = readJson(path.join(MEMBERS_DIR, `${c.profile}.json`));
  const { ctx, weights, loadout: start } = prepareCase(bot, member, c.job);
  const ctx1 = { ...ctx, secretDigit: sdOf(ctx) };
  const out = { id };

  // ---- items
  const seen = new Set();
  const itemList = [];
  const addItem = (it) => {
    if (!it) return;
    const k = JSON.stringify(it);
    if (seen.has(k)) return;
    seen.add(k);
    itemList.push(it);
  };
  addItem(start.weapon); addItem(start.rag); addItem(start.clearBoots);
  for (const v of Object.values(start.armor)) addItem(v);
  for (const v of Object.values(start.equipment)) addItem(v);
  if (start.dragonGear) { addItem(start.dragonGear.weapon); for (const v of Object.values(start.dragonGear.armor)) addItem(v); for (const v of Object.values(start.dragonGear.equipment)) addItem(v); }
  for (const b of start.blades) addItem(b);
  for (const b of start.spareBlades) addItem(b);
  [...start.accessories.values()].slice(0, 60).forEach(addItem);
  out.items = itemList.map((item) => ({
    item,
    contrib0: triple(S.itemContribution(item, ctx)),
    contrib1: triple(S.itemContribution(item, ctx1)),
    raw0: triple(S.itemRawStats(item, ctx)),
    raw1: triple(S.itemRawStats(item, ctx1)),
    acc0: obj(S.accessoryItemStats(item, ctx)),
    acc1: obj(S.accessoryItemStats(item, ctx1)),
    ap: S.accessoryPower(item, resource),
    isDungeon: S.isDungeon(item, resource),
    canRecomb: S.canRecomb(item, resource),
    rarity: S.itemRarity(item),
    chimera: item.enchants?.ultimate_chimera ? triple(S.itemRawStats(item, ctx, { chimeraPetStrength: 123.456 })) : null,
  }));

  // ---- bag
  out.bag = [];
  for (const [power, extra] of [[undefined, 0], ['silky', 0], ['sighted', 7], ['bubba', 3], ['nope', 0]]) {
    for (const [tag, c2] of [[0, ctx], [1, ctx1]]) {
      const l = { ...start, power: power ?? start.power };
      const r = S.accessoryStats(l, c2, extra);
      out.bag.push({ power: power ?? null, extra, sd: tag, stats: obj(r.stats), ap: r.ap, tuningPoints: r.tuningPoints });
    }
  }
  out.counted = S.countedAccessories(start).map((i) => i.id);
  out.totalAp = S.totalAccessoryPower(start, resource);

  // ---- shards
  out.shards = triple(S.shardStats(start, ctx));
  out.shards.tuningPoints = S.shardStats(start, ctx).tuningPoints;
  out.shardTests = [{ JORMUNG: 10 }, { JORMUNG: 10, MOLTHORN: 10 }, { JORMUNG: 10, MOLTHORN: 10, TIAMAT: 10 }, { SKELETOR: 10, BARBARIAN_DUKE_X: 10, HIDEONBOX: 10, CUBOA: 9, BRUISER: 3 }].map((shards) => {
    const r = S.shardStats({ shards }, ctx);
    return { shards, result: { ...triple(r), tuningPoints: r.tuningPoints } };
  });

  // ---- tuning
  out.tuning = [];
  for (const points of [0, 5, 37, 200]) for (const speed of [300, 430, 499, 520]) for (const as of [40, 99, 100, 120]) for (const mult of [1, 1.2]) for (const capBonus of [0, 20]) {
    const r = S.tuningAllocation(points, speed, { ...ctx, speedCapBonus: capBonus }, as, mult);
    out.tuning.push({ points, speed, as, mult, capBonus, stats: obj(r.stats), keys: Object.keys(r.stats), attackSpeedPoints: r.attackSpeedPoints, speedPoints: r.speedPoints, dpsPoints: r.dpsPoints });
  }

  // ---- misc scalar functions
  out.scalars = {
    volleys: [0, 20, 99, 100, 139.4, 150].map((a) => [a, S.volleysPerSecond(start.weapon, a, ctx)]),
    arrows: [0, 0.2].map((e) => [e, S.arrowsPerVolley(start.weapon, e, ctx)]),
    saCloak: [0.5, 1, 3, 4, 10].map((h) => [h, S.saCloakMultiplier(h, ctx)]),
    classAdditive: S.classAdditiveDamage(ctx),
    berserkMelee: S.berserkMeleeMultiplier(ctx),
    archerMult: S.archerMultiplier(ctx),
    book: S.bookMultiplier(start, ctx),
    maxed: S.maxedCollections(ctx),
    profileTotals: obj(S.profileStatTotals({ ...ctx, secretDigit: 0 })),
    hasDragonSet: U.hasDragonSet(ctx),
  };

  // ---- picks
  const noPets = { ...start, activePets: { wither: null, dragon: null } };
  const act = (r) => ({ wither: r.activePets.wither ?? null, dragon: r.activePets.dragon ?? null });
  out.picks = {
    activeExtras: S.activeExtras(start, ctx, weights).map((e) => e.key),
    dpsPets: S.dpsPets(start.pets, ctx),
    selectPet: act(S.selectPet(start, ctx, weights)),
    selectPetFromNone: act(S.selectPet(noPets, ctx, weights)),
    selectPower: U.selectPower({ ...start, power: ctx.power ?? start.power }, ctx, weights).power,
    selectPowerFromStart: U.selectPower(start, ctx, weights).power,
    bestTuningSplit: U.bestTuningSplit(start, ctx, weights),
    bestTuningSplitNoPets: U.bestTuningSplit(noPets, ctx, weights),
    score: U.score(start, ctx, weights),
  };

  // ---- ops
  const L = (l) => serializeLoadout(l);
  const ops = [];
  const clone = (l) => U.cloneLoadout(l);
  ops.push({ op: 'clone', result: L(clone(start)) });
  const gear = [];
  if (start.weapon) gear.push(['weapon', start.weapon]);
  for (const [k, v] of Object.entries(start.armor)) gear.push([k, v]);
  for (const [k, v] of Object.entries(start.equipment)) gear.push([k, v]);
  for (const [slot, item] of gear.slice(0, 6)) {
    const next = { ...item, recomb: !item.recomb, name: item.name };
    ops.push({ op: 'replaceItem', old: item, next, result: L(U.replaceItem(start, item, next)) });
    const trial = clone(start);
    const other = { ...item, name: `${item.name} (alt)`, reforge: 'fabled' };
    U.setSlot(trial, slot, other);
    ops.push({ op: 'setSlot', slot, item: other, result: L(trial) });
  }
  if (start.rag) {
    const next = { ...start.rag, hpb: 3 };
    ops.push({ op: 'replaceItem', old: start.rag, next, result: L(U.replaceItem(start, start.rag, next)) });
    const trial = clone(start); U.setSlot(trial, 'rag', next);
    ops.push({ op: 'setSlot', slot: 'rag', item: next, result: L(trial) });
  }
  if (start.clearBoots) {
    const next = { ...start.clearBoots, recomb: !start.clearBoots.recomb };
    ops.push({ op: 'replaceItem', old: start.clearBoots, next, result: L(U.replaceItem(start, start.clearBoots, next)) });
  }
  const accs = [...start.accessories.values()];
  for (const a of accs.slice(0, 3).concat(accs.slice(-2))) {
    const same = { ...a, recomb: !a.recomb };
    ops.push({ op: 'replaceItem', old: a, next: same, result: L(U.replaceItem(start, a, same)) });
    const renamed = { ...a, id: `X_${a.id}` };
    ops.push({ op: 'replaceItem', old: a, next: renamed, result: L(U.replaceItem(start, a, renamed)) });
  }
  if (U.hasDragonSet(ctx)) {
    for (const [slot, item] of gear.slice(0, 5)) {
      const alt = { ...item, name: `${item.name} (dragon)`, reforge: 'fabled' };
      const t1 = clone(start); U.setDragonSlot(t1, slot, alt);
      ops.push({ op: 'setDragonSlot', slot, item: alt, result: L(t1) });
      const t2 = clone(t1); U.setDragonSlot(t2, slot, null);
      ops.push({ op: 'setDragonSlot', base: 'prev', slot, item: null, result: L(t2), from: L(t1) });
      const t3 = clone(t1); U.setSlot(t3, slot, alt);
      ops.push({ op: 'setSlot', base: 'prev', slot, item: alt, result: L(t3), from: L(t1) });
      const t4 = clone(t1);
      U.setDragonSlot(t4, slot, item);
      ops.push({ op: 'setDragonSlot', base: 'prev', slot, item, result: L(t4), from: L(t1) });
      const t5 = U.replaceItem(t1, alt, { ...alt, hpb: 2 });
      ops.push({ op: 'replaceItem', base: 'prev', old: alt, next: { ...alt, hpb: 2 }, result: L(t5), from: L(t1) });
    }
  }
  out.ops = ops;

  // ---- misc
  out.slotNames = [];
  for (const it of itemList) out.slotNames.push([it, U.slotName(start, it)]);
  out.slotNames.push([{ id: 'NOPE', name: 'x' }, U.slotName(start, { id: 'NOPE', name: 'x' })]);
  out.upgradable = U.upgradableItems(start).map((i) => i.id);
  const view = S.gearView(start, 'dragon');
  out.gearView = { weapon: view.weapon?.id ?? null, armor: Object.fromEntries(Object.entries(view.armor).map(([k, v]) => [k, v?.id ?? null])), equipment: Object.fromEntries(Object.entries(view.equipment).map(([k, v]) => [k, v?.id ?? null])), same: view === start };
  out.loadoutKey = S.loadoutKey(start);
  out.excluded = [...resource.keys()].filter((i) => U.excludedItem(i, ctx));
  out.recombableItems = itemList.map((i) => S.canRecomb(i, resource));
  writeJson(path.join(OUT, `${id}.json`), out);
  console.log(id, 'ok');
}

// ---- global
const g = {};
g.weightsFor = [];
for (const cls of ['arch', 'bers', 'mage']) for (const target of [null, 'wither', 'dragon']) for (const mode of ['lcm', 'rcm']) for (const floorMode of ['master', 'normal']) {
  g.weightsFor.push({ cls, target, mode, floorMode, result: U.weightsFor(cls, target, mode, floorMode) });
}
g.skeleton = [];
for (const cls of ['arch', 'bers', 'mage']) for (const cata of [0, 35, 36, 50]) g.skeleton.push({ cls, cata, result: U.skeletonMasterChestRule(cls, cata) });
g.newItem = {};
for (const id of resource.keys()) g.newItem[id] = U.newItem(id, resource);
g.newItem.NOT_IN_RESOURCE = U.newItem('NOT_IN_RESOURCE', resource);
const ids = [...resource.keys()];
const pairs = [];
const fraggedFrom = ids.filter((i) => U.upgradesInto(i, 'x') === false);
for (const a of ids) for (const b of ['POWER_WITHER_BOOTS', 'DIAMOND_NECRON_HEAD', 'STARRED_' + a, 'SPEED_WITHER_LEGGINGS', a]) if (U.upgradesInto(a, b)) pairs.push([a, b]);
g.upgradesInto = pairs;
g.upgradesIntoFalse = [['HYPERION', 'POWER_WITHER_BOOTS'], ['GOLD_NECRON_HEAD', 'DIAMOND_BONZO_HEAD'], ['WITHER_BOOTS', 'POWER_WITHER_HELMET'], ['X', 'Y']];
g.superseded = [];
const accIds = ids.filter((i) => resource.get(i).category === 'ACCESSORY').slice(0, 400);
for (const id of accIds.slice(0, 120)) {
  g.superseded.push([id, S.accessorySuperseded(id, accIds.slice(0, 200), resource), S.accessorySuperseded(id, accIds.slice(0, 200))]);
}
g.canRecomb = ids.map((id) => [id, S.canRecomb({ id, baseRarity: S.itemRarity({ baseRarity: resource.get(id).tier ?? 'COMMON', id }), recomb: false }, resource)]);
g.baseStats = {};
for (const cls of ['arch', 'mage']) for (const [cata, fm] of [[50, 'master'], [20, 'normal'], [36, 'master']]) {
  const m = {};
  for (const id of ids.slice(0, 6000)) {
    const b = S.baseItemStats(id, resource, cls, cata, fm);
    if (Object.keys(b.global).length || Object.keys(b.wither).length || Object.keys(b.dragon).length) m[id] = triple(b);
  }
  g.baseStats[`${cls}|${cata}|${fm}`] = m;
}
writeJson(path.join(OUT, 'global.json'), g);
console.log('global ok');
