// Times readMember (JS) over every testdata profile: cold first pass and best of 5 warm passes.
//   oracle/run.sh skyblock-member-bench.mjs
import path from 'node:path';
import { PROFILES_DIR, SRC, freezeNow, loadSnapshot, profileNames, readJson } from './common.mjs';

const snap = loadSnapshot();
freezeNow(snap.now);
const { readMember } = await import(`${SRC}/api/hypixel.js`);
const profiles = profileNames().map((n) => readJson(path.join(PROFILES_DIR, `${n}.json`)));
const pass = () => {
  const t = process.hrtime.bigint();
  for (const { uuid, profile } of profiles) readMember(profile, uuid, snap.resource, snap.collections);
  return Number(process.hrtime.bigint() - t) / 1e6;
};
const cold = pass();
let best = Infinity;
for (let i = 0; i < 5; i++) best = Math.min(best, pass());
console.log(`readMember x${profiles.length}: cold ${cold.toFixed(1)} ms (${(cold / profiles.length).toFixed(2)} ms each), warm best ${best.toFixed(1)} ms (${(best / profiles.length).toFixed(2)} ms each)`);
