// Checks testdata/golden/ against testdata/cases.json and merges the per-container timing lines
// (testdata/golden-timing/<worker>.jsonl, written by golden.mjs) into testdata/golden-timing.json:
// { cases, missing, totalMs, totalCpuMs, kinds: { <kind>: { n, ms, cpuMs } }, slowest: [{ id, ms, cpuMs }],
//   perCase: { <id>: { ms, cpuMs, seq, steps | rows, bytes } } }.
// "ms" is the wall time golden.mjs measured around runDps (also in golden/<id>.json); cpuMs is process CPU time
// (all Node threads). Both were measured with ~10 containers running at once, so they carry contention.
//   oracle/run.sh golden-summary.mjs
import fs from 'node:fs';
import path from 'node:path';
import { TD, readJson, writeJson } from './common.mjs';

const cases = readJson(path.join(TD, 'cases.json'));
const timingDir = path.join(TD, 'golden-timing');
const timing = new Map();
for (const f of fs.existsSync(timingDir) ? fs.readdirSync(timingDir).sort() : []) {
  if (!f.endsWith('.jsonl') || f.startsWith('rerun-')) continue;
  for (const line of fs.readFileSync(path.join(timingDir, f), 'utf8').split('\n')) if (line) {
    const t = JSON.parse(line);
    timing.set(t.id, { ...t, worker: f.slice(0, -6) });
  }
}

const kindOf = (c) => {
  const j = c.job;
  if (j.menu === 'profile') return `profile-${j.cls}`;
  return `${j.cls}-${j.mode}-${j.floorMode}${j.upgradeMode === 'max' ? '-max' : ''}${j.anyChest ? '-anychest' : ''}`;
};
const perCase = {};
const missing = [];
const kinds = {};
let totalMs = 0;
let totalCpuMs = 0;
let bytes = 0;
for (const c of cases) {
  const file = path.join(TD, 'golden', `${c.id}.json`);
  if (!fs.existsSync(file)) { missing.push(c.id); continue; }
  const size = fs.statSync(file).size;
  bytes += size;
  const g = readJson(file);
  const t = timing.get(c.id);
  const ms = g.ms;
  const cpuMs = t?.cpuMs ?? null;
  totalMs += ms;
  totalCpuMs += cpuMs ?? 0;
  const k = (kinds[kindOf(c)] ??= { n: 0, ms: 0, cpuMs: 0 });
  k.n++;
  k.ms += ms;
  k.cpuMs += cpuMs ?? 0;
  perCase[c.id] = { ms, cpuMs, seq: t?.seq ?? null, ...(g.plan ? { steps: g.plan.steps.length } : { rows: g.profile.rows.length }), bytes: size };
}
const slowest = Object.entries(perCase).sort((a, b) => b[1].ms - a[1].ms).slice(0, 15).map(([id, t]) => ({ id, ms: t.ms, cpuMs: t.cpuMs }));
const out = { cases: cases.length, missing, totalMs, totalCpuMs, bytes, kinds, slowest, perCase };
writeJson(path.join(TD, 'golden-timing.json'), out);

console.log(`golden: ${cases.length - missing.length}/${cases.length} cases, ${(bytes / 1024).toFixed(0)} KB`);
if (missing.length) console.log(`missing: ${missing.join(', ')}`);
console.log(`total wall ${(totalMs / 1000).toFixed(1)} s, total cpu ${(totalCpuMs / 1000).toFixed(1)} s`);
for (const [k, v] of Object.entries(kinds)) console.log(`  ${k}: ${v.n} cases, wall ${(v.ms / 1000).toFixed(1)} s (avg ${(v.ms / v.n / 1000).toFixed(2)} s), cpu ${(v.cpuMs / 1000).toFixed(1)} s`);
console.log('slowest:');
for (const s of slowest) console.log(`  ${s.id}: ${(s.ms / 1000).toFixed(2)} s wall, ${((s.cpuMs ?? 0) / 1000).toFixed(2)} s cpu`);
if (missing.length) process.exit(1);
