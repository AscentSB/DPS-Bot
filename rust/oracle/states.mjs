// testdata/states/<caseId>.jsonl: every planner state of a coin plan, in the order greedyPlan first scores them,
// from the instrumented JS copy (build it with instrument.mjs). Run:
//   JS_SRC=$PWD/testdata/js-instrumented/src oracle/run.sh states.mjs [--no-verify] [--list] [caseId ...]
// (states-all.sh runs the default cases a few containers at a time). Without ids it runs DEFAULT_CASES below.
//
// The plan is runDps exactly as golden.mjs runs it (members/<name>.json, in-thread, no pool); the hook in
// candidatesFor reports each state once (candidatesFor runs once per distinct state; rules.states reuses it in
// later re-plan rounds). One line per state:
//   { round, loadout: L, current: R, banned, candidates, scored, bundles }
//   round      : planUpgrades re-plan round that first scored the state (0 = the first greedyPlan, k = the k-th
//                re-plan, i.e. loop variable round + 1).
//   loadout    : the state's loadout, serializeLoadout (after selectPower(selectPet(apply)), as the planner keeps it).
//   current    : evaluate(loadout, ctx, weights), serializeEval.
//   banned     : [...rules.banned] at scoring time, the labels scoreUpgrades skipped (extra key; [] in round 0).
//   candidates : availableUpgrades(loadout, ctx, prices), serializeCandidate each, in list order.
//   scored     : scoreUpgrades(ups, ...) output in order: { index, gain, shortlistOnly?, changes: [{ key, prevKey,
//                kind }] } (changes = pieceChanges without the item; the item id is the part of key after ':').
//   bundles    : every armorBundles(...) bundle in order: { label, cost, source, unverified, repick, gain?,
//                changes? }; gain and changes only on bundles scoreUpgrades kept (gain > 1e-9).
// Also writes states/<caseId>.ctx.json: the ctx (serializeCtx) the plan used, as in ctx/ (without reforgeCache,
// the bestReforge cache availableUpgrades adds to ctx).
//
// Checks (exit 1 if any fail): the plan equals golden/<caseId>.json (minus "ms"); ctx is the same at every state
// (and equals ctx/<caseId>.json when that exists); and unless --no-verify, every line is rebuilt from its JSON
// (deserializeLoadout(L), ctx from the ctx JSON + resource) and evaluate, availableUpgrades and scoreUpgrades on
// the rebuilt state must give the same current, candidates and scored JSON (so L and ctx hold everything the
// planner reads for a state).
import fs from 'node:fs';
import path from 'node:path';
import { CTX_DIR, MEMBERS_DIR, TD, readJson, writeJson } from './common.mjs';
import { loadBot } from './harness.mjs';
import { deserializeLoadout, nonFinitePaths, serializeCandidate, serializeCtx, serializeEval, serializeLoadout } from './serialize.mjs';

export const DEFAULT_CASES = [
  'recticc__arch-lcm-master-all-coins',
  'recticc__mage-lcm-master-all-coins',
  'recticc__mage-lcm-normal-all-coins',
  'recticc__mage-rcm-master-all-coins',
  'recticc__mage-lcm-master-max-coins',
  'recticc__arch-lcm-master-all-anychest-coins',
  'recticc__arch-lcm-master-all-wither-coins',
  'recticc__arch-lcm-master-all-b19.12.0-coins',
  'recticc__bers-lcm-master-all-coins',
  'Scraamss__arch-lcm-master-all-coins',
  'Scraamss__mage-lcm-master-all-coins',
  'tolv__mage-lcm-master-all-coins',
  'Ben_MC__arch-lcm-master-all-dragon-coins',
  'Ben_MC__mage-rcm-master-all-coins',
];

const args = process.argv.slice(2);
const flag = (name) => {
  const i = args.indexOf(name);
  if (i >= 0) args.splice(i, 1);
  return i >= 0;
};
const VERIFY = !flag('--no-verify');
// --list: print DEFAULT_CASES (one per line) and exit; states-all.sh reads it.
if (flag('--list')) {
  console.log(DEFAULT_CASES.join('\n'));
  process.exit(0);
}
const ids = args.length ? args : DEFAULT_CASES;

const bot = await loadBot();
const { evaluate } = bot.stats;
const { availableUpgrades, scoreUpgrades } = bot.upgrades;
const cases = new Map(readJson(path.join(TD, 'cases.json')).map((c) => [c.id, c]));
const STATES_DIR = path.join(TD, 'states');
fs.mkdirSync(STATES_DIR, { recursive: true });

const changesOut = (changes) => changes.map(({ key, prevKey, kind }) => ({ key, prevKey, kind }));
const scoredOut = (scored) => scored.map(({ index, gain, shortlistOnly, changes }) => ({ index, gain, ...(shortlistOnly ? { shortlistOnly } : {}), changes: changesOut(changes) }));

const problems = [];
for (const id of ids) {
  const c = cases.get(id);
  if (!c) throw new Error(`No case ${id}`);
  if (c.job.menu !== 'coins') throw new Error(`${id} is not a coin plan`);
  const t0 = performance.now();
  const member = readJson(path.join(MEMBERS_DIR, `${c.profile}.json`));
  const file = path.join(STATES_DIR, `${id}.jsonl`);
  const tmp = `${file}.tmp`;
  const fd = fs.openSync(tmp, 'w');
  let ctxJson = null;
  let ctxBack = null;
  let lines = 0;
  let bytes = 0;
  const rounds = new Map();
  const bad = (msg) => { if (problems.length < 200) problems.push(`${id}: ${msg}`); };

  globalThis.__oracleState = ({ round, loadout, current, ups, banned, scored, bundles, bundleScores, ctx, weights, prices }) => {
    // ctx.reforgeCache is bestReforge's cache (a Map availableUpgrades adds to ctx), not an input: left out.
    const { reforgeCache, ...ctxIn } = ctx;
    const ctxNow = JSON.stringify(serializeCtx(ctxIn));
    if (ctxJson === null) {
      ctxJson = ctxNow;
      ctxBack = { ...JSON.parse(ctxJson), resource: bot.snap.resource };
    } else if (ctxNow !== ctxJson) bad(`ctx changed at state ${lines}`);
    const kept = new Map(bundleScores.map((x) => [x.index, x]));
    const line = {
      round,
      loadout: serializeLoadout(loadout),
      current: serializeEval(current),
      banned: [...banned],
      candidates: ups.map(serializeCandidate),
      scored: scoredOut(scored),
      bundles: bundles.map((b, i) => {
        const k = kept.get(i);
        return { ...serializeCandidate(b), ...(k ? { gain: k.gain, changes: changesOut(k.changes) } : {}) };
      }),
    };
    const text = JSON.stringify(line);
    const nf = nonFinitePaths(line);
    if (nf.length) bad(`state ${lines}: non-finite ${nf.slice(0, 3).join(', ')}`);
    if (VERIFY) {
      // The state as a reader of this line (and the ctx file) sees it.
      const back = JSON.parse(text);
      const lb = deserializeLoadout(back.loadout);
      const cur = evaluate(lb, ctxBack, weights);
      if (JSON.stringify(serializeEval(cur)) !== JSON.stringify(line.current)) bad(`state ${lines}: current differs when rebuilt from JSON`);
      const ups2 = availableUpgrades(lb, ctxBack, prices);
      if (JSON.stringify(ups2.map(serializeCandidate)) !== JSON.stringify(line.candidates)) {
        const a = ups2.map(serializeCandidate);
        let k = 0;
        while (k < a.length && k < line.candidates.length && JSON.stringify(a[k]) === JSON.stringify(line.candidates[k])) k++;
        bad(`state ${lines}: candidates differ when rebuilt from JSON (first at ${k} of ${line.candidates.length}/${a.length}: orig ${JSON.stringify(line.candidates[k])} vs rebuilt ${JSON.stringify(a[k])})`);
      }
      else if (JSON.stringify(scoredOut(scoreUpgrades(ups2, lb, cur, ctxBack, weights, back.banned))) !== JSON.stringify(line.scored)) bad(`state ${lines}: scored differs when rebuilt from JSON`);
    }
    fs.writeSync(fd, `${text}\n`);
    lines++;
    bytes += text.length + 1;
    rounds.set(round, (rounds.get(round) ?? 0) + 1);
  };

  const { cls, target, mode, floorMode, upgradeMode, menu, limit, blessings, anyChest } = c.job;
  const result = await bot.runDps({ member, cls, target, mode, floorMode, upgradeMode, menu, limit, blessings, anyChest, resource: bot.snap.resource, prices: bot.prices });
  globalThis.__oracleState = undefined;
  fs.closeSync(fd);
  fs.renameSync(tmp, file);
  writeJson(path.join(STATES_DIR, `${id}.ctx.json`), JSON.parse(ctxJson));

  const goldenFile = path.join(TD, 'golden', `${id}.json`);
  if (!fs.existsSync(goldenFile)) bad('no golden file to compare with');
  else {
    const { ms, ...golden } = readJson(goldenFile);
    if (JSON.stringify(golden) !== JSON.stringify(result)) bad('plan differs from golden');
  }
  const ctxFile = path.join(CTX_DIR, `${id}.json`);
  if (fs.existsSync(ctxFile) && JSON.stringify(readJson(ctxFile)) !== ctxJson) bad('ctx differs from ctx/');
  const perRound = [...rounds].map(([r, n]) => `r${r}:${n}`).join(' ');
  console.log(`${id}: ${lines} states (${perRound}), ${(bytes / 1e6).toFixed(1)} MB, ${result.plan.steps.length} steps, ${((performance.now() - t0) / 1000).toFixed(1)} s`);
}
if (problems.length) {
  console.log(`PROBLEMS (${problems.length}):\n${problems.join('\n')}`);
  process.exit(1);
}
console.log(`states: ${ids.length} cases ok${VERIFY ? ' (verified)' : ''}`);
