// Rough JS timings of the same helpers as crates/core/tests/bench.rs (petStats with fresh objects, so its cache misses).
import * as P from '/app/src/lib/pets.js';
import { powerMultiplier } from '/app/src/calc/formulas.js';
import { getReforge } from '/app/src/calc/stats.js';
const types = P.allPetTypes().flatMap((t) => P.petTiers(t).map((tier) => [t, tier]));
const items = [null, 'HEPHAESTUS_RELIC', 'GOLD_CLAWS', 'PET_ITEM_TIER_BOOST'];
let t = performance.now(); let calls = 0;
for (let r = 0; r < 200; r++) for (const [type, tier] of types) for (const heldItem of items) { P.petStats({ type, tier, heldItem }, { bank: 5e8 }); calls++; }
console.log(`petStats (uncached): ${((performance.now() - t) * 1e6 / calls).toFixed(1)} ns/call`);
t = performance.now(); let acc = 0;
for (let rep = 0; rep < 100; rep++) for (let ap = 0; ap < 10000; ap++) acc += powerMultiplier(ap + rep);
console.log(`powerMultiplier: ${((performance.now() - t) * 1e6 / 1e6).toFixed(1)} ns/call`);
const xs = Array.from({ length: 100000 }, (_, i) => Math.abs(Math.sin(i * 7919.123457)) * 10 ** ((i % 12) - 3));
let sink = 0;
t = performance.now(); for (const x of xs) sink += x.toFixed(2).length; console.log(`toFixed(2): ${((performance.now() - t) * 1e6 / xs.length).toFixed(1)} ns/call`);
t = performance.now(); for (const x of xs) sink += String(x).length; console.log(`String(x): ${((performance.now() - t) * 1e6 / xs.length).toFixed(1)} ns/call`);
t = performance.now(); for (const x of xs) sink += x.toLocaleString('en-US').length; console.log(`toLocaleString: ${((performance.now() - t) * 1e6 / xs.length).toFixed(1)} ns/call`);
const names = ['Fabled', 'ancient', 'Withered', 'giant', "Jerry's"];
t = performance.now(); for (let i = 0; i < 200000; i++) for (const n of names) sink += getReforge(n) ? 1 : 0;
console.log(`getReforge: ${((performance.now() - t) * 1e6 / 1e6).toFixed(1)} ns/call`, sink > 0 ? '' : '');
