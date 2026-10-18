// testdata/members/<name>.json for every testdata/profiles/<name>.json: readMember(profile, uuid, resource,
// collections) from src/api/hypixel.js with Date.now() fixed at snapshot.now. A profile readMember rejects
// (inventory API off) gets { "error": message }.
//   oracle/run.sh members.mjs [name ...]
import path from 'node:path';
import { MEMBERS_DIR, PROFILES_DIR, SRC, freezeNow, loadSnapshot, profileNames, readJson, writeJson } from './common.mjs';
import { nonFinitePaths } from './serialize.mjs';

const snap = loadSnapshot();
freezeNow(snap.now);
const { readMember } = await import(`${SRC}/api/hypixel.js`);

const names = process.argv.length > 2 ? process.argv.slice(2) : profileNames();
let ok = 0;
let failed = 0;
for (const name of names) {
  const { uuid, profile } = readJson(path.join(PROFILES_DIR, `${name}.json`));
  let out;
  try {
    out = readMember(profile, uuid, snap.resource, snap.collections);
    ok++;
    const bad = nonFinitePaths(out);
    if (bad.length) console.log(`${name}: WARNING non-finite numbers (JSON writes null): ${bad.slice(0, 5).join(', ')}`);
    console.log(`${name}: cata ${out.cataLevel}, ${out.items.length} items, ${out.pets.length} pets`);
  } catch (err) {
    out = { error: err.message };
    failed++;
    console.log(`${name}: error: ${err.message}`);
  }
  writeJson(path.join(MEMBERS_DIR, `${name}.json`), out);
}
console.log(`members: ${ok} read, ${failed} errors`);
