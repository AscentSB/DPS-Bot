// Prints the start loadout (JS) of a review case: node review-load.mjs <caseId>
import path from 'node:path';
import { TD, readJson } from './common.mjs';
import { loadBot, prepareCase } from './harness.mjs';
const b = await loadBot();
const c = readJson(path.join(TD, 'review-plan', `${process.argv[2]}.json`));
const { loadout, weights } = prepareCase(b, c.member, c.job);
const brief = (i) => i ? `${i.id}[${i.stars ?? ''}${i.reforge ?? ''}${JSON.stringify(i.enchants)}]` : null;
console.log('weapon', brief(loadout.weapon));
console.log('armor', Object.fromEntries(Object.entries(loadout.armor).map(([k, v]) => [k, brief(v)])));
console.log('equip', Object.fromEntries(Object.entries(loadout.equipment).map(([k, v]) => [k, brief(v)])));
const d = loadout.dragonGear;
console.log('dragon', d && { weapon: brief(d.weapon), armor: Object.fromEntries(Object.entries(d.armor).map(([k, v]) => [k, brief(v)])), equipment: Object.fromEntries(Object.entries(d.equipment).map(([k, v]) => [k, brief(v)])) });
