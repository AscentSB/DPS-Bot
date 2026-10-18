// testdata/ctx/<caseId>.json and testdata/evals/<caseId>.jsonl for a diverse subset of the coin cases in
// testdata/cases.json. Run after cases.mjs:
//   oracle/run.sh evals.mjs [--count 30] [--lines 200] [--min-sample 50] [--dry] [caseId ...]
// --dry only prints the cases it would pick.
//
// For each case: ctx, weights and the start loadout exactly as runDps builds them (prepareCase), ctx.weights set as
// planUpgrades sets it. ctx/<id>.json is the context without the items resource. evals/<id>.jsonl has one line per
// evaluated loadout: { kind, label, loadout: L, result: R } where R = evaluate(L, ctx, weights):
//   kind 'start'  : the start loadout (label 'start').
//   kind 'apply'  : next = up.apply(start) for a candidate of availableUpgrades(start, ctx, prices) (label = its label).
//   kind 'repick' : for a candidate with repick whose 'apply' line gains > 1e-9 (as scoreUpgrades does):
//                   selectPower(selectPet(next)), right after its 'apply' line.
// Candidates (lines in candidate-list order): every one in group 'pet', every repick candidate that gains, then an
// even sample of the repick candidates that do not gain (a third of what is left of --lines) and of all the other
// candidates, about --lines lines per case and at least --min-sample sampled lines. (Every repick candidate would be over 1,000 lines on a Mage profile with
// ~70 KB per line, so the losing ones are sampled.) Candidates with cost <= 0 are skipped (scoreUpgrades never
// scores them).
// Every line is checked before writing: L and the ctx JSON are read back into JS objects and evaluated again, and
// the result must be the same JSON as R (so L and ctx hold everything evaluate() reads). The start (ctx, loadout,
// evaluation) must also be the same when built from readMember's live output instead of members/<name>.json.
import fs from 'node:fs';
import path from 'node:path';
import { CTX_DIR, DEFAULT_BLESSINGS, EVALS_DIR, MEMBERS_DIR, PROFILES_DIR, TD, readJson, writeJson } from './common.mjs';
import { loadBot, prepareCase } from './harness.mjs';
import { deserializeLoadout, nonFinitePaths, serializeCtx, serializeEval, serializeLoadout } from './serialize.mjs';

const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : def;
};
const LINES = Number(opt('--lines', 200));
// Sampled lines (beyond the pet and gaining repick ones) every case gets, even when pets alone fill --lines.
const MIN_SAMPLE = Number(opt('--min-sample', 50));
const COUNT = Number(opt('--count', 30));
const DRY = args.includes('--dry') && Boolean(args.splice(args.indexOf('--dry'), 1));
const explicit = args;

const bot = await loadBot();
const { evaluate, selectPet, weightedGain } = bot.stats;
const { availableUpgrades, selectPower } = bot.upgrades;

const cases = readJson(path.join(TD, 'cases.json'));
const memberCache = new Map();
const memberOf = (name) => {
  if (!memberCache.has(name)) memberCache.set(name, readJson(path.join(MEMBERS_DIR, `${name}.json`)));
  return memberCache.get(name);
};

// --- Case selection: greedy over weighted features (class / set / floor, that crossed with a Catacombs band, each
// variant kind, the profile), each feature worth less every time a picked case already has it (w / (1 + n)^2), so
// the picks cover every kind first and then spread over classes and profiles. Ties keep cases.json order, so the
// pick is deterministic.
const band = (cata) => (cata < 35 ? 'low' : cata < 48 ? 'mid' : 'high');
const sameBlessings = (a, b) => a.power === b.power && a.wisdom === b.wisdom && a.time === b.time;
function features(c) {
  const j = c.job;
  const combo = `${j.cls}-${j.mode}-${j.floorMode}`;
  const f = [[`combo:${combo}`, 5], [`band:${combo}:${band(memberOf(c.profile).cataLevel)}`, 3], [`profile:${c.profile}`, 2]];
  if (j.upgradeMode === 'max') f.push([`max:${combo}`, 4]);
  if (j.anyChest) f.push([`anychest:${j.cls}`, 4]);
  if (j.target) f.push([`target:${j.target}`, 4]);
  if (!sameBlessings(j.blessings, DEFAULT_BLESSINGS)) f.push([`blessings:${combo}`, 4]);
  return f;
}
function pickCases() {
  const pool = cases.filter((c) => c.job.menu === 'coins' && !memberOf(c.profile).error);
  const seen = new Map();
  const picked = [];
  while (picked.length < COUNT && picked.length < pool.length) {
    let best = null;
    let bestScore = -1;
    for (const c of pool) {
      if (picked.includes(c)) continue;
      const s = features(c).reduce((sum, [k, w]) => sum + w / (1 + (seen.get(k) ?? 0)) ** 2, 0);
      if (s > bestScore) { bestScore = s; best = c; }
    }
    picked.push(best);
    for (const [k] of features(best)) seen.set(k, (seen.get(k) ?? 0) + 1);
  }
  return picked;
}

const selected = explicit.length ? explicit.map((id) => cases.find((c) => c.id === id) ?? (() => { throw new Error(`No case ${id}`); })()) : pickCases();

if (DRY) {
  for (const c of selected) console.log(`${c.id} (cata ${memberOf(c.profile).cataLevel})`);
  process.exit(0);
}

// --- Dump ---
fs.mkdirSync(CTX_DIR, { recursive: true });
fs.mkdirSync(EVALS_DIR, { recursive: true });
const stableJson = (v) => JSON.stringify(v);
let totalLines = 0;
let totalBytes = 0;
const problems = [];
for (const c of selected) {
  const t0 = performance.now();
  const member = memberOf(c.profile);
  const { ctx, weights, loadout } = prepareCase(bot, member, c.job);
  // The same case built from readMember's live output (not its JSON) must give the same start: the member files
  // lose nothing the planner reads.
  {
    const { uuid, profile } = readJson(path.join(PROFILES_DIR, `${c.profile}.json`));
    const live = prepareCase(bot, bot.readMember(profile, uuid, bot.snap.resource, bot.snap.collections), c.job);
    const a = JSON.stringify([serializeCtx(ctx), serializeLoadout(loadout), serializeEval(evaluate(loadout, ctx, weights))]);
    const b = JSON.stringify([serializeCtx(live.ctx), serializeLoadout(live.loadout), serializeEval(evaluate(live.loadout, live.ctx, live.weights))]);
    if (a !== b) problems.push(`${c.id}: start built from the live readMember output differs from the member file's`);
  }
  const ctxOut = serializeCtx(ctx);
  for (const p of nonFinitePaths(ctxOut)) problems.push(`${c.id} ctx ${p}`);
  writeJson(path.join(CTX_DIR, `${c.id}.json`), ctxOut);
  // The context as a reader of ctx/<id>.json sees it (plus the resource), for the read-back check.
  const ctxBack = { ...JSON.parse(JSON.stringify(ctxOut)), resource: bot.snap.resource };

  const lines = [];
  const add = (kind, label, l) => {
    const r = evaluate(l, ctx, weights);
    const L = serializeLoadout(l);
    const R = serializeEval(r);
    for (const p of nonFinitePaths(R)) problems.push(`${c.id} "${label}" result ${p}`);
    const text = stableJson({ kind, label, loadout: L, result: R });
    const back = JSON.parse(text);
    const again = stableJson(serializeEval(evaluate(deserializeLoadout(back.loadout), ctxBack, weights)));
    if (again !== stableJson(back.result)) problems.push(`${c.id} "${label}": read-back evaluate differs`);
    for (const a of l.accessories.entries()) if (a[0] !== a[1].id) problems.push(`${c.id} "${label}": accessory key ${a[0]} holds ${a[1].id}`);
    lines.push(text);
    return r;
  };

  const current = add('start', 'start', loadout);
  const ups = availableUpgrades(loadout, ctx, bot.prices).filter((up) => up.cost > 0);
  // Which repick candidates gain on their own (scoreUpgrades re-picks only those).
  const gains = new Map();
  ups.forEach((up, i) => { if (up.repick) gains.set(i, weightedGain(current, evaluate(up.apply(loadout), ctx, weights), weights)); });
  const gaining = (i) => gains.get(i) > 1e-9;
  const must = [];
  const repickOther = [];
  const others = [];
  ups.forEach((up, i) => {
    if (up.group === 'pet' || gaining(i)) must.push(i);
    else if (up.repick) repickOther.push(i);
    else others.push(i);
  });
  // Fill the line budget: a third (or what the others leave) with repick candidates that lose damage, the rest with
  // the other candidates, each an even sample over the candidate list.
  const room = Math.max(MIN_SAMPLE, LINES - 1 - must.length - must.filter(gaining).length);
  const nRepick = Math.min(repickOther.length, Math.max(Math.floor(room / 3), room - others.length));
  const nOther = Math.min(others.length, room - nRepick);
  const even = (list, n) => Array.from({ length: n }, (_, k) => list[Math.floor(((k + 0.5) * list.length) / n)]);
  const chosen = new Set([...must, ...even(repickOther, nRepick), ...even(others, nOther)]);
  let repicks = 0;
  ups.forEach((up, i) => {
    if (!chosen.has(i)) return;
    const next = up.apply(loadout);
    const r = add('apply', up.label, next);
    if (up.repick && weightedGain(current, r, weights) > 1e-9) {
      add('repick', up.label, selectPower(selectPet(next, ctx, weights), ctx, weights));
      repicks++;
    }
  });
  const pets = ups.filter((up) => up.group === 'pet').length;

  const body = `${lines.join('\n')}\n`;
  fs.writeFileSync(path.join(EVALS_DIR, `${c.id}.jsonl`), body);
  totalLines += lines.length;
  totalBytes += Buffer.byteLength(body);
  console.log(`${c.id}: cata ${member.cataLevel}, ${ups.length} candidates (${pets} pet, ${gains.size} repick of which ${must.filter(gaining).length} gain), ${lines.length} lines (${repicks} repicks), ${(Buffer.byteLength(body) / 1e6).toFixed(1)} MB, ${Math.round(performance.now() - t0)} ms`);
}
console.log(`\nevals: ${selected.length} cases, ${totalLines} lines, ${(totalBytes / 1e6).toFixed(1)} MB`);
if (problems.length) {
  console.log(`PROBLEMS (${problems.length}):\n${problems.slice(0, 40).join('\n')}`);
  process.exitCode = 1;
}
