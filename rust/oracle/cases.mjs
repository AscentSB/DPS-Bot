// testdata/cases.json: [{ id, profile, job }] for every profile with a readable member (testdata/members). `job` is
// model::DpsJob. Run after members.mjs: oracle/run.sh cases.mjs
//
// Every profile: coin plans (limit 40, default blessings, upgradeMode 'all') for arch, bers, mage LCM Master Mode,
// mage LCM Normal Mode and mage RCM, plus the profile menu for arch and mage LCM Master Mode.
// Variant profiles (the seed profiles and 4 others spread over Catacombs levels) also get: upgradeMode 'max' (arch,
// bers, mage LCM), anyChest (arch / bers, Catacombs 36+ only), target wither / dragon (arch) and blessings
// power 19 / wisdom 12 / time 0 (arch, mage LCM).
// Case id: <profile>__<cls>-<mode>-<floorMode>-<upgradeMode>[-anychest][-<target>][-b<power>.<wisdom>.<time>]-<menu>
import path from 'node:path';
import { DEFAULT_BLESSINGS, MEMBERS_DIR, PLAN_STEPS, TD, profileNames, readJson, writeJson } from './common.mjs';

const SEEDS = ['recticc', 'Scraamss'];
const OTHER_VARIANT_PROFILES = 4;
const MAX_COIN_CASES = 220;
const ANY_CHEST_CATA = 36;
const CUSTOM_BLESSINGS = { power: 19, wisdom: 12, time: 0 };
const BASE = [['arch', 'lcm', 'master'], ['bers', 'lcm', 'master'], ['mage', 'lcm', 'master'], ['mage', 'lcm', 'normal'], ['mage', 'rcm', 'master']];
const PROFILE_MENUS = [['arch', 'lcm', 'master'], ['mage', 'lcm', 'master']];

const sameBlessings = (a, b) => a.power === b.power && a.wisdom === b.wisdom && a.time === b.time;
function makeCase(profile, cls, mode, floorMode, { upgradeMode = 'all', anyChest = false, target = null, blessings = DEFAULT_BLESSINGS, menu = 'coins' } = {}) {
  const id = `${profile}__${cls}-${mode}-${floorMode}-${upgradeMode}${anyChest ? '-anychest' : ''}${target ? `-${target}` : ''}`
    + `${sameBlessings(blessings, DEFAULT_BLESSINGS) ? '' : `-b${blessings.power}.${blessings.wisdom}.${blessings.time}`}-${menu}`;
  return { id, profile, job: { cls, target, mode, floorMode, upgradeMode, menu, limit: PLAN_STEPS, blessings: { ...blessings }, anyChest } };
}

const members = new Map();
for (const name of profileNames()) {
  let m;
  try { m = readJson(path.join(MEMBERS_DIR, `${name}.json`)); } catch { continue; }
  if (!m.error) members.set(name, m);
}

// Variant profiles: the seeds, then the lowest and highest Catacombs level and two evenly spaced between them (over
// the distinct levels, so many maxed profiles do not crowd the picks); the first profile by name at each level.
const others = [...members.keys()].filter((n) => !SEEDS.includes(n));
const levels = [...new Set(others.map((n) => members.get(n).cataLevel))].sort((a, b) => a - b);
const picks = new Set();
for (let i = 0; i < OTHER_VARIANT_PROFILES && levels.length; i++) {
  const level = levels[Math.round((i * (levels.length - 1)) / (OTHER_VARIANT_PROFILES - 1))];
  const name = others.find((n) => members.get(n).cataLevel === level && !picks.has(n)) ?? others.find((n) => !picks.has(n));
  if (name) picks.add(name);
}
const variantProfiles = [...SEEDS.filter((n) => members.has(n)), ...picks];

function variants(name) {
  const cata = members.get(name).cataLevel;
  const out = [
    makeCase(name, 'arch', 'lcm', 'master', { upgradeMode: 'max' }),
    makeCase(name, 'bers', 'lcm', 'master', { upgradeMode: 'max' }),
    makeCase(name, 'mage', 'lcm', 'master', { upgradeMode: 'max' }),
  ];
  if (cata >= ANY_CHEST_CATA) out.push(makeCase(name, 'arch', 'lcm', 'master', { anyChest: true }), makeCase(name, 'bers', 'lcm', 'master', { anyChest: true }));
  out.push(
    makeCase(name, 'arch', 'lcm', 'master', { target: 'wither' }),
    makeCase(name, 'arch', 'lcm', 'master', { target: 'dragon' }),
    makeCase(name, 'arch', 'lcm', 'master', { blessings: CUSTOM_BLESSINGS }),
    makeCase(name, 'mage', 'lcm', 'master', { blessings: CUSTOM_BLESSINGS }),
  );
  return out;
}

const cases = [];
for (const name of members.keys()) {
  for (const [cls, mode, floorMode] of BASE) cases.push(makeCase(name, cls, mode, floorMode));
  if (variantProfiles.includes(name)) cases.push(...variants(name));
  for (const [cls, mode, floorMode] of PROFILE_MENUS) cases.push(makeCase(name, cls, mode, floorMode, { menu: 'profile' }));
}
// Over budget: drop the variant profiles' Berserker max-level plans, then their other max-level plans.
const coinCount = () => cases.filter((c) => c.job.menu === 'coins').length;
for (const drop of [(c) => c.job.upgradeMode === 'max' && c.job.cls === 'bers', (c) => c.job.upgradeMode === 'max']) {
  if (coinCount() <= MAX_COIN_CASES) break;
  for (let i = cases.length - 1; i >= 0 && coinCount() > MAX_COIN_CASES; i--) if (drop(cases[i])) cases.splice(i, 1);
}

if (new Set(cases.map((c) => c.id)).size !== cases.length) throw new Error('Duplicate case ids.');
writeJson(path.join(TD, 'cases.json'), cases);
console.log(`cases.json: ${cases.length} cases (${coinCount()} coin plans, ${cases.length - coinCount()} profile menus) over ${members.size} profiles`);
console.log(`variant profiles: ${variantProfiles.map((n) => `${n} (cata ${members.get(n).cataLevel})`).join(', ')}`);
