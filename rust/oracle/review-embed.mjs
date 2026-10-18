// Review vectors for the /dps embed: edge-case sessions through the JS buildEmbed / buttons / pageCount.
// Output: testdata/review/embed-edge.json = { entries: [{ id, session, embed, components }] } (same shape as embeds/).
//   JS_SRC=$PWD/testdata/js-instrumented/src oracle/run.sh review-embed.mjs
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const SRC = process.env.JS_SRC_IN_CONTAINER ?? '/app/src';
const dps = await import(`${SRC}/commands/dps.js`);
const { buildEmbed, buttons } = dps;
const base0 = JSON.parse(fs.readFileSync('/td/embeds/Ben_MC__arch-lcm-master-all-coins.json', 'utf8')).entries[0].session;
const mage0 = JSON.parse(fs.readFileSync('/td/embeds/Ben_MC__mage-rcm-master-all-coins.json', 'utf8')).entries[0].session;
const clone = (o) => JSON.parse(JSON.stringify(o));

const out = [];
const add = (id, s) => {
  let embed = null; let components = null; let error = null;
  try { embed = buildEmbed(s).toJSON(); } catch (e) { error = String(e.message).slice(0, 80); }
  try { components = buttons(id, s).map((r) => r.toJSON()); } catch (e) { error ??= String(e.message).slice(0, 80); }
  out.push({ id, session: s, embed, components, error });
};
const withPlan = (b, f) => { const s = clone(b); f(s, s.coins[Object.keys(s.coins)[0]].plan); return s; };

// money formatting edges
const costs = [0, 1, 999, 999.4999, 999.5, 1000, 1000.04, 1500, 99999.95, 999949.9, 999950, 999999, 999999.5, 1e6, 1004999, 1005000, 123456789, 999999999, 1e9, 1.5e12, 12345678901234];
const pms = [0, 1e-7, 4.9e-7, 5e-7, 0.0000015, 0.99999949, 0.9999995, 0.99999999, 1, 1.000001, 10.123455, 2.5e-6, 1e21, 123456.789];
const gains = [0, 0.005, 0.015, 0.025, 0.045, 1.005, 2.675, -0.001, -0.005, 0.994999, 99.995, 1e21, 123456.789, 5e-7];
add('money', withPlan(base0, (s, p) => {
  p.steps = costs.map((cost, i) => ({ label: `Cost ${i}`, cost, source: 'AH', unverified: i % 3 === 0, gain: gains[i % gains.length], perMillion: pms[i % pms.length], witherGain: 1, dragonGain: 1 }));
}));
for (let page = 0; page < 3; page++) add(`money-p${page}`, { ...withPlan(base0, (s, p) => { p.steps = costs.map((cost, i) => ({ label: `Cost ${i}`, cost, source: 'AH', gain: gains[(i + 3) % gains.length], perMillion: pms[(i + 5) % pms.length], witherGain: 1, dragonGain: 1 })); }), page });
// page beyond the end, empty plan, exactly 8, 9, 16, 17 steps
for (const n of [0, 1, 8, 9, 16, 17, 40]) {
  for (const page of [0, 1, 2, 5]) {
    add(`n${n}-p${page}`, { ...withPlan(base0, (s, p) => { const t = p.steps[0]; p.steps = Array.from({ length: n }, (_, i) => ({ ...t, label: `S${i}`, unverified: i === 7 })); }), page });
  }
}
// stats formatting: -0.4 rounds to -0, big numbers, .5 ties, undefined fields, equal/different dragons
const statVariants = [
  { critChance: -0.4, attackSpeed: 0.5, speed: 1.5, strength: 2.5, critDamage: -0.5, intelligence: 1234567.5, abilityDamage: -0.6 },
  { critChance: 1e21, attackSpeed: 999.5, speed: 0.49999999999999994, strength: 12345.4999, critDamage: 1e-9, intelligence: 99999.5, abilityDamage: 1 },
];
const mkStats = (a, b) => ({ wither: a, dragon: b });
for (const [wi, w] of [{ wither: 50, dragon: 50 }, { wither: 100, dragon: 0 }, { wither: 0, dragon: 100 }, { wither: 0, dragon: 0 }, { wither: 0.5, dragon: 1e-300 }].entries()) {
  for (const [vi, [a, b]] of [[0, 0], [0, 1], [1, 1], [1, 0]].entries()) {
    for (const base of [base0, mage0]) {
      add(`stats-w${wi}-v${vi}-${base.cls}-${base.mode}`, withPlan(base, (s, p) => {
        s.coins[Object.keys(s.coins)[0]].weights = w;
        p.startDamage.stats = mkStats(statVariants[a], statVariants[b]);
        p.endDamage.stats = mkStats(statVariants[b], statVariants[b]);
      }));
    }
  }
}
// tuning text
const tunings = [
  [{ attackSpeed: 0, speed: 0, dps: 0 }, { strength: 1 }], [{ attackSpeed: 12, speed: 0, dps: 181 }, null], [{ attackSpeed: 0.5, speed: 3.25, dps: 7 }, { strength: 0.5, critDamage: 0.5 }],
  [{ attackSpeed: 1e21, speed: 1e-7, dps: 181 }, { strength: 0.0027, critDamage: 0.0028, critChance: 0.2, intelligence: 0.5, weird: 0.3 }],
  [{ attackSpeed: 0, speed: 50, dps: 3 }, { strength: 0.5, critChance: 0.5 }], [{ attackSpeed: 0, speed: 0, dps: 2.5 }, { strength: 0.5, critChance: 0.1, critDamage: 0.2 }], [{ attackSpeed: 0, speed: 0, dps: 1 }, {}],
  [{ attackSpeed: -3, speed: 0, dps: 100 }, { strength: -0.5, critDamage: 1.005 }],
];
tunings.forEach(([t, sp], i) => add(`tuning-${i}`, withPlan(base0, (s, p) => { p.startDamage.tuning = t; s.coins[Object.keys(s.coins)[0]].tuningSplit = sp; })));
// names
['a_b', '__x', 'x_', '_', 'a__b', 'ab_cd_ef', '___', '____', 'x__', 'A_B_C_D_E', 'z___z', '0_0', 'a_', '_a'].forEach((name, i) => add(`name-${i}`, { ...clone(base0), name }));
// class / mode / floor / cata / chest / blessings / target
for (const cls of ['arch', 'bers', 'mage']) for (const cata of [0, 35, 36, 39, 40, 41]) for (const anyChest of [false, true]) for (const view of ['coins', 'profile']) for (const um of ['all', 'max']) {
  const key = `${um}${anyChest ? ':anyChest' : ''}`;
  const s = clone(cls === 'mage' ? mage0 : base0);
  s.cls = cls; s.member = { cataLevel: cata }; s.anyChest = anyChest; s.view = view; s.upgradeMode = um;
  const first = s.coins[Object.keys(s.coins)[0]]; s.coins = { [key]: first };
  add(`cls-${cls}-${cata}-${anyChest}-${view}-${um}`, s);
}
for (const mode of ['lcm', 'rcm']) for (const floorMode of ['master', 'normal']) for (const view of ['coins', 'profile']) {
  const s = clone(mage0); s.mode = mode; s.floorMode = floorMode; s.view = view; add(`mage-${mode}-${floorMode}-${view}`, s);
}
add('blessings-null', { ...clone(base0), blessings: null });
add('blessings-zero', { ...clone(base0), blessings: { power: 5, wisdom: 0, time: 0 } });
add('member-none', { ...clone(base0), member: undefined });
// profile view: empty rows, long rows
add('profile-empty', { ...clone(base0), view: 'profile', profile: { rows: [], damage: base0.profile.damage } });
add('profile-gains', { ...clone(base0), view: 'profile', profile: { rows: [0, 0.005, 0.015, 1.005, 1e-9, 12.3456, 100, -0.004].map((gain, i) => ({ label: `R${i}`, gain })), damage: base0.profile.damage } });
add('profile-rcm', { ...clone(mage0), view: 'profile' });
// damage text of big/small numbers
for (const [i, w] of [0, 1, 999.5, 1000, 999950, 999999.5, 1e9, 123456789012, 0.4, 0.5, 1.5, 2.5].entries()) {
  add(`damage-${i}`, withPlan(base0, (s, p) => { p.startDamage.wither = w; p.startDamage.dragon = w * 3; p.endDamage.wither = w + 1e6; p.endDamage.dragon = w + 1e9; }));
}
const dir = '/td/review';
fs.mkdirSync(dir, { recursive: true });
fs.writeFileSync(path.join(dir, 'embed-edge.json'), JSON.stringify({ entries: out }));
console.log(out.length, 'entries;', out.filter((e) => e.error).map((e) => `${e.id}: ${e.error}`));
