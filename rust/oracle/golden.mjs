// testdata/golden/<caseId>.json: the JS runDps output for every case in testdata/cases.json (in-thread, no
// evaluation pool, Date.now frozen at snapshot.now), plus "ms": the JS wall time of the runDps call.
//   oracle/run.sh golden.mjs [--claims DIR] [--out DIR] [--live] [--force] [--compare] [--worker N] [caseId ...]
// Normally started through golden-all.sh, which runs ~10 containers of this script at once over one shared queue.
//
// --claims DIR : work queue shared by parallel containers. A case is run by the container that first creates
//                DIR/<caseId> (O_EXCL), so every case runs once; the order is slowest kind first (Mage coin plans,
//                then the other coin plans, then profile menus) so the long ones do not end up last.
// --out DIR    : write there instead of TD/golden (determinism reruns write to a scratch dir and compare).
// --live       : build the member with readMember from profiles/<name>.json instead of reading members/<name>.json.
// --force      : rerun cases that already have an output file (default: skip them).
// --compare    : check each result against TD/golden/<caseId>.json (minus "ms"); exit 1 on any difference. With
//                --out, --force and a few ids this is the determinism check (a fresh process, another order).
// --worker N   : a tag written into the timing lines (which container ran the case).
//
// Besides the golden file, each case appends one line to TD/golden-timing/<worker>.jsonl:
// { id, ms, cpuMs, seq } (cpuMs = process.cpuUsage() user + system over the runDps call, all Node threads, so it
// includes GC helper threads; seq = position of the case in this container's run, 0 = cold JIT).
// golden-summary.mjs merges those into TD/golden-timing.json.
import fs from 'node:fs';
import path from 'node:path';
import { MEMBERS_DIR, PROFILES_DIR, TD, readJson } from './common.mjs';
import { loadBot } from './harness.mjs';
import { nonFinitePaths } from './serialize.mjs';

const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : def;
};
const flag = (name) => {
  const i = args.indexOf(name);
  if (i >= 0) args.splice(i, 1);
  return i >= 0;
};
const CLAIMS = opt('--claims', null);
const OUT = opt('--out', path.join(TD, 'golden'));
const WORKER = opt('--worker', '0');
const LIVE = flag('--live');
const FORCE = flag('--force');
const COMPARE = flag('--compare');
const explicit = args;

const bot = await loadBot();
const cases = readJson(path.join(TD, 'cases.json'));
const byId = new Map(cases.map((c) => [c.id, c]));
const selected = explicit.length ? explicit.map((id) => byId.get(id) ?? (() => { throw new Error(`No case ${id}`); })()) : cases;

// Slowest kind first (stable within a kind, so the order is deterministic).
const weight = (c) => (c.job.menu === 'profile' ? 2 : c.job.cls === 'mage' ? 0 : 1);
const queue = CLAIMS ? [...selected].sort((a, b) => weight(a) - weight(b)) : selected;

const memberOf = (name) => {
  if (!LIVE) return readJson(path.join(MEMBERS_DIR, `${name}.json`));
  const { uuid, profile } = readJson(path.join(PROFILES_DIR, `${name}.json`));
  return bot.readMember(profile, uuid, bot.snap.resource, bot.snap.collections);
};

fs.mkdirSync(OUT, { recursive: true });
if (CLAIMS) fs.mkdirSync(CLAIMS, { recursive: true });
const timingDir = path.join(TD, 'golden-timing');
fs.mkdirSync(timingDir, { recursive: true });
const timingFile = path.join(timingDir, `${OUT === path.join(TD, 'golden') ? '' : 'rerun-'}${WORKER}.jsonl`);

const claim = (id) => {
  if (!CLAIMS) return true;
  try {
    fs.closeSync(fs.openSync(path.join(CLAIMS, id), 'wx'));
    return true;
  } catch (err) {
    if (err.code === 'EEXIST') return false;
    throw err;
  }
};

let seq = 0;
let failed = 0;
let differ = 0;
for (const c of queue) {
  const file = path.join(OUT, `${c.id}.json`);
  if (!FORCE && fs.existsSync(file)) continue;
  if (!claim(c.id)) continue;
  const member = memberOf(c.profile);
  if (member.error) {
    console.log(`${c.id}: member has an error (${member.error}), skipped`);
    continue;
  }
  const { cls, target, mode, floorMode, upgradeMode, menu, limit, blessings, anyChest } = c.job;
  // What runInWorker sends (src/commands/dps.js) plus what the plan worker adds: resource and frozen prices, no pool.
  const job = { member, cls, target, mode, floorMode, upgradeMode, menu, limit, blessings, anyChest, resource: bot.snap.resource, prices: bot.prices };
  let result;
  const cpu0 = process.cpuUsage();
  const t0 = performance.now();
  try {
    result = await bot.runDps(job);
  } catch (err) {
    failed++;
    console.log(`${c.id}: runDps threw: ${err?.stack ?? err}`);
    continue;
  }
  const ms = performance.now() - t0;
  const cpu = process.cpuUsage(cpu0);
  const cpuMs = (cpu.user + cpu.system) / 1000;
  const bad = nonFinitePaths(result);
  if (bad.length) console.log(`${c.id}: WARNING non-finite numbers (JSON writes null): ${bad.slice(0, 5).join(', ')}`);
  fs.writeFileSync(file, JSON.stringify({ ...result, ms }));
  if (COMPARE) {
    const { ms: _ms, ...golden } = readJson(path.join(TD, 'golden', `${c.id}.json`));
    const same = JSON.stringify(golden) === JSON.stringify(result);
    if (!same) differ++;
    console.log(`${c.id}: ${same ? 'identical to golden' : 'DIFFERS from golden'}`);
  }
  fs.appendFileSync(timingFile, `${JSON.stringify({ id: c.id, ms, cpuMs, seq })}\n`);
  const steps = result.plan?.steps.length ?? result.profile?.rows.length;
  console.log(`[w${WORKER}] ${c.id}: ${menu === 'profile' ? `${steps} rows` : `${steps} steps`}, ${ms.toFixed(0)} ms (cpu ${cpuMs.toFixed(0)} ms)`);
  seq++;
}
console.log(`[w${WORKER}] done: ${seq} cases run, ${failed} failed${COMPARE ? `, ${differ} differ from golden` : ''}`);
if (failed || differ) process.exit(1);
