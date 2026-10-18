// Reference vectors for dps-core's jsfmt module (JS number formatting) and the one transcendental formula.
// Writes /td/jsfmt.json as [{ fn, args, out }].
//
//   docker run --rm -v /home/andrew/Downloads/DPS-Bot/src:/app/src:ro \
//     -v /home/andrew/Downloads/DPS-Bot/rust/oracle:/oracle:ro \
//     -v /home/andrew/Downloads/DPS-Bot/rust/testdata:/td -w /oracle dps-bot:latest node jsfmt.mjs
//
// Numbers that JSON cannot hold are written as strings: "NaN", "Infinity", "-Infinity", "-0".
import { writeFileSync } from 'node:fs';
import { formatCoins } from '/app/src/commands/dps.js';
import { powerMultiplier } from '/app/src/calc/formulas.js';

// Deterministic PRNG (mulberry32), so the vectors are the same on every run.
let seed = 0x9e3779b9;
function rand() {
  seed |= 0;
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const randInt = (n) => Math.floor(rand() * n);

const enc = (x) => {
  if (typeof x !== 'number') return x;
  if (Number.isNaN(x)) return 'NaN';
  if (x === Infinity) return 'Infinity';
  if (x === -Infinity) return '-Infinity';
  if (Object.is(x, -0)) return '-0';
  return x;
};

const values = new Set();
const add = (x) => { if (!values.has(x) && !(Object.is(x, -0) && values.has('-0'))) values.add(Object.is(x, -0) ? '-0' : x); };
const num = (v) => (v === '-0' ? -0 : v);

// Special values and boundaries.
for (const x of [0, -0, NaN, Infinity, -Infinity, 5e-324, -5e-324, 2.2250738585072014e-308, Number.MAX_VALUE, -Number.MAX_VALUE,
  Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER + 1, 2 ** 53 + 2, 2 ** 64, 2 ** 70, 1e21, -1e21, 999999999999999900000, 9.999999999999999e20,
  1e20, 1e22, 1.5e21, 1.2345e25, 1e-7, -1e-7, 1e-6, 1.5e-6, 1e-5, 0.000001, 0.0000012345, 123e-20, 1e-300, 1e300,
  0.1, 0.2, 0.3, 0.1 + 0.2, 1 / 3, 2 / 3, 1.005, 1.0005, 1.0015, 2.675, 1.255, 8.345, 10.235, 1.45, 2.5, 0.5, 1.5, -0.5, -1.5, -2.5,
  0.125, 0.375, 2.125, 0.0625, 123456.7895, 1234.5675, 0.0005, 0.0015, 0.00049999999999999999, 0.49999999999999994, -0.49999999999999994,
  4503599627370495.5, 4503599627370496.5, -4503599627370495.5, 999.9995, 999999.9995, 999.95, 999999.99, 1e9 - 0.001, 1e6 - 0.01,
  1e3 - 0.05, 999.5, 1000, 1e6, 1e9, 1e12, 1e15, 1e18, -0.0001, -0.001, -1234.5, -1234.5675, 12345678.125, 0.9995, 0.99949999]) add(x);

// Exact binary ties at every digit position used by the bot: i + k / 2^m.
for (let i = 0; i < 400; i++) {
  const m = 1 + randInt(12);
  const k = 1 + 2 * randInt(2 ** (m - 1));
  const base = [0, 1, 7, 12, 999, 1000, 123456, 999999, 1e6, 12345678, 1e9, 2 ** 40][randInt(12)] + randInt(1000);
  add(base + k / 2 ** m);
  add(-(base + k / 2 ** m));
}
// Decimal literals that look like ties (x.xx5, x.xxx5, ...): binary values just above or below the tie.
for (let i = 0; i < 600; i++) {
  const int = [0, 1, 9, 99, 999, 1000, 12345, 999999, 1234567, 999999999][randInt(10)] + (rand() < 0.5 ? 0 : randInt(100000));
  const fracDigits = 1 + randInt(7);
  let frac = '';
  for (let d = 0; d < fracDigits - 1; d++) frac += String(randInt(10));
  frac += rand() < 0.8 ? '5' : String(randInt(10));
  const x = Number(`${int}.${frac}`);
  add(x);
  if (rand() < 0.2) add(-x);
}
// Random doubles across magnitudes (log scale), both signs.
for (let i = 0; i < 1500; i++) {
  const x = 10 ** (rand() * 32 - 10) * (1 + rand());
  add(rand() < 0.15 ? -x : x);
}
// Typical bot values: percent gains, per-1M gains, coin costs and their /1e9, /1e6, /1e3 quotients, stat values.
for (let i = 0; i < 800; i++) {
  add(rand() * 100);
  add(rand() * 1e-3);
  add(Math.floor(rand() * 2e10));
  add(Math.floor(rand() * 2e10) / 1e6);
  add(rand() * 20000);
}
// Random integers, small and large.
for (let i = 0; i < 300; i++) {
  add(randInt(1000));
  add(randInt(1e9) * 1000 + randInt(1000));
  add(Math.floor(rand() * 2 ** 53));
}
// Random bit patterns (finite only).
const buf = new DataView(new ArrayBuffer(8));
for (let i = 0; i < 400; i++) {
  buf.setUint32(0, Math.floor(rand() * 2 ** 32));
  buf.setUint32(4, Math.floor(rand() * 2 ** 32));
  const x = buf.getFloat64(0);
  if (Number.isFinite(x)) add(x);
}

const out = [];
const push = (fn, args, value) => out.push({ fn, args: args.map(enc), out: enc(value) });
const all = [...values].map(num);
for (const x of all) {
  push('String', [x], String(x));
  push('toLocaleString', [x], x.toLocaleString('en-US'));
  push('Math.round', [x], Math.round(x));
  push('formatCoins', [x], formatCoins(x));
  for (const d of [0, 1, 2, 3, 5, 6]) push('toFixed', [x, d], x.toFixed(d));
  if (rand() < 0.15) for (const d of [4, 8, 10, 15, 17, 20, 21, 25, 30, 50, 100]) push('toFixed', [x, d], x.toFixed(d));
}
// Math.round(x).toLocaleString('en-US') (stat numbers in embeds) and integer counts.
for (const x of all) push('toLocaleString', [Math.round(x)], Math.round(x).toLocaleString('en-US'));
// Accessory Power multiplier: every integer AP the bot can reach, plus random reals; and the two steps of it.
for (let ap = 0; ap <= 10000; ap++) push('powerMultiplier', [ap], powerMultiplier(ap));
for (let i = 0; i < 2000; i++) {
  const ap = rand() * 20000;
  push('powerMultiplier', [ap], powerMultiplier(ap));
}
for (let i = 0; i < 3000; i++) {
  const x = rand() < 0.5 ? 1 + rand() * 40 : 10 ** (rand() * 40 - 20);
  push('Math.log', [x], Math.log(x));
  const y = rand() * 4;
  push('pow', [y, 1.2], y ** 1.2);
}

writeFileSync('/td/jsfmt.json', JSON.stringify(out));
console.log(`wrote ${out.length} vectors (${all.length} distinct values)`);
process.exit(0);
