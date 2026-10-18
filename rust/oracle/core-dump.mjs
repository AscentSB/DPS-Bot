// Reference data for dps-core: every config table, the data files as the JS code sees them, pet stats, item
// requirements, floor drops, cleanName and the formulas. Writes /td/core.json.
//
//   docker run --rm -v /home/andrew/Downloads/DPS-Bot/src:/app/src:ro \
//     -v /home/andrew/Downloads/DPS-Bot/rust/oracle:/oracle:ro \
//     -v /home/andrew/Downloads/DPS-Bot/rust/testdata:/td -w /oracle dps-bot:latest node core-dump.mjs
//
// Numbers that JSON cannot hold are written as strings: "NaN", "Infinity", "-Infinity", "-0".
import { readFileSync, writeFileSync } from 'node:fs';
import * as constants from '/app/src/config/constants.js';
import * as enchants from '/app/src/config/enchants.js';
import * as accessories from '/app/src/config/accessories.js';
import * as pets from '/app/src/config/pets.js';
import * as shards from '/app/src/config/shards.js';
import * as essence from '/app/src/config/essence.js';
import * as gemstones from '/app/src/config/gemstones.js';
import * as armorSets from '/app/src/config/armorSets.js';
import * as itemStats from '/app/src/config/itemStats.js';
import * as itemUpgrades from '/app/src/config/itemUpgrades.js';
import * as profile from '/app/src/config/profile.js';
import * as upgrades from '/app/src/config/upgrades.js';
import * as F from '/app/src/calc/formulas.js';
import * as P from '/app/src/lib/pets.js';
import * as D from '/app/src/lib/drops.js';
import { meetsRequirements } from '/app/src/lib/requirements.js';
import { cleanName } from '/app/src/lib/nbt.js';
import { allReforges, getReforge, reforgeAppliesTo, accessoryFamily, accessoryChainPosition, gemStat, isHatcessory, itemRarity, statRarity, isWeaponCategory } from '/app/src/calc/stats.js';
import FRAGGED from '/app/src/data/fragged.json' with { type: 'json' };
import ACCESSORY_DATA from '/app/src/data/accessories.json' with { type: 'json' };

let seed = 0x1234567;
function rand() {
  seed |= 0;
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const pick = (a) => a[Math.floor(rand() * a.length)];

// JSON.stringify replacer that keeps -0, NaN and the infinities.
const replacer = (k, v) => {
  if (typeof v !== 'number') return v;
  if (Number.isNaN(v)) return 'NaN';
  if (v === Infinity) return 'Infinity';
  if (v === -Infinity) return '-Infinity';
  if (Object.is(v, -0)) return '-0';
  return v;
};
const plain = (mod) => Object.fromEntries(Object.entries(mod).filter(([, v]) => typeof v !== 'function' && !(v instanceof RegExp)));

const out = {};

// ---- config tables (the module exports as JSON; functions and regexes are tested through calls below) ----
out.config = {
  constants: plain(constants), enchants: plain(enchants), accessories: plain(accessories), pets: plain(pets), shards: plain(shards),
  essence: plain(essence), gemstones: plain(gemstones), armorSets: plain(armorSets), itemStats: plain(itemStats),
  itemUpgrades: plain(itemUpgrades), profile: plain(profile), upgrades: plain(upgrades),
};

// ---- function-valued config ----
const raw = JSON.parse(readFileSync('/td/snapshot.raw.json', 'utf8'));
const resource = new Map(raw.items);
const ids = [...resource.keys()];
const categories = [...new Set([...resource.values()].map((i) => i.category))];
out.calls = {
  blessingsWith: [
    {}, { power: 10, wisdom: 3, time: 0 }, { power: 4, wisdom: 15, time: 3 }, { power: 10.5 }, { power: 29, wisdom: 0, time: 5 },
    { power: 5, wisdom: 14 }, { power: '10' }, { time: 0 },
  ].map((levels) => ({ levels, out: Object.fromEntries(Object.entries(constants.blessingsWith(levels)).map(([k, b]) => [k, b.level])) })),
  speedCap: ['arch', 'bers', 'mage'].flatMap((cls) => ['lcm', 'rcm'].map((mode) => ({ cls, mode, out: constants.speedCap(cls, mode) }))),
  equipmentSlot: categories.map((c) => ({ category: c ?? null, out: constants.equipmentSlot(c) })),
  maxReforgeItem: ids.filter((id) => constants.MAX_REFORGE_ITEMS.test(id)),
  classExcluded: ['arch', 'bers', 'mage'].map((cls) => ({ cls, ids: ids.filter((id) => constants.CLASS_EXCLUDED_ITEMS[cls]?.test(id)) })),
  enchantMaxLevel: Object.fromEntries(Object.entries(enchants.ENCHANTS).map(([k, e]) => [k, enchants.enchantMaxLevel(e)])),
  tuningStats: ['arch', 'bers', 'mage'].flatMap((cls) => ['lcm', 'rcm'].map((mode) => ({ cls, mode, out: accessories.tuningStats({ cls, mode }) }))),
  enrichmentKinds: ['arch', 'bers', 'mage'].flatMap((cls) => ['lcm', 'rcm'].map((mode) => ({ cls, mode, out: accessories.enrichmentKinds({ cls, mode }) }))),
  hatcessory: ids.filter((id) => accessories.HATCESSORY.test(id)),
  itemRarity: [...constants.RARITIES, 'SPECIAL', 'VERY_SPECIAL', 'ADMIN', 'UNOBTAINABLE'].flatMap((baseRarity) => [false, true].flatMap((recomb) =>
    ['HYPERION', 'DIAMOND_NECRON_HEAD', 'GOLD_BONZO_HEAD', 'GOLD_HEAD', 'X'].map((id) => ({ baseRarity, recomb, id, out: itemRarity({ id, baseRarity, recomb }) })))),
  statRarity: [...constants.RARITIES, 'SPECIAL', 'VERY_SPECIAL', 'ADMIN'].map((r) => [r, statRarity(r)]),
  isWeaponCategory: categories.map((c) => [c ?? null, isWeaponCategory(c)]),
  gemStat: Object.keys(gemstones.GEMSTONES).concat(['RUBY']).flatMap((gem) => [...gemstones.GEM_QUALITIES, 'WEIRD'].flatMap((q) =>
    [...constants.RARITIES, 'SPECIAL', 'VERY_SPECIAL', 'NONE'].map((r) => ({ gem, quality: q, rarity: r, out: gemStat(gem, q, r) })))),
};

// ---- data files ----
const reforges = allReforges();
const reforgeCandidates = [...ids.filter(() => rand() < 0.02), 'ASPECT_OF_THE_END', 'HYPERION', 'TERMINATOR', 'GREAT_SPOOK_BELT']
  .map((id) => ({ id, category: resource.get(id)?.category ?? null }))
  .concat(categories.map((category) => ({ id: 'X', category: category ?? null })));
out.data = {
  reforgeOrder: Object.keys(reforges),
  reforges,
  getReforge: ['Fabled', 'fabled', 'ANCIENT', "Jerry's", 'Not A Reforge', '', 'Withered!', 'giant'].map((name) => ({ name, out: Object.entries(reforges).find(([, r]) => r === getReforge(name))?.[0] ?? null })),
  reforgeCandidates,
  reforgeAppliesTo: Object.entries(reforges).map(([key, r]) => ({
    key,
    ok: reforgeCandidates.map((it, i) => (reforgeAppliesTo(r, { id: it.id, category: it.category ?? undefined }) ? i : -1)).filter((i) => i >= 0),
  })),
  accessoryFamily: [...new Set([...Object.keys(ACCESSORY_DATA.upgrades), ...Object.values(ACCESSORY_DATA.upgrades).flat(), ...ids.filter((id) => resource.get(id)?.category === 'ACCESSORY')])]
    .map((id) => [id, accessoryFamily(id), accessoryChainPosition(id)]),
  fragged: FRAGGED,
  petTypes: P.allPetTypes(),
  petTiers: Object.fromEntries(P.allPetTypes().map((t) => [t, P.petTiers(t)])),
  maxPetLevel: Object.fromEntries([...P.allPetTypes(), 'NOT_A_PET'].map((t) => [t, P.maxPetLevel(t)])),
  petLevel: [...P.allPetTypes().filter(() => rand() < 0.3), 'GOLDEN_DRAGON', 'NOT_A_PET'].flatMap((type) => ['COMMON', 'RARE', 'LEGENDARY', 'MYTHIC', 'WEIRD'].flatMap((tier) =>
    [0, 99, 100, 5e4, 1e6, 2.5e7, 1e8, 2.5e8, 1e12].map((exp) => ({ type, tier, exp, out: P.petLevel(type, tier, exp) })))),
  effectiveTier: ['COMMON', 'UNCOMMON', 'RARE', 'EPIC', 'LEGENDARY', 'MYTHIC', 'WEIRD'].flatMap((tier) => [null, pets.TIER_BOOST_ID, 'GOLD_CLAWS']
    .map((heldItem) => ({ tier, heldItem, out: P.effectiveTier({ type: 'X', tier, heldItem }) }))),
};

// ---- pet stats ----
const petProfiles = [{}, { bank: 0 }, { bank: 5e8, goldCollection: 1234 }, { bank: 2e9, goldCollection: 1e12 }, { goldCollection: 0 }, { bank: 123456789.5, goldCollection: 99999999 }];
const heldItems = [null, ...Object.keys(pets.PET_ITEMS), pets.TIER_BOOST_ID, 'NOT_AN_ITEM'];
out.petStats = [];
for (const type of [...P.allPetTypes(), 'NOT_A_PET']) {
  for (const tier of [...P.petTiers(type), 'EPIC', 'COMMON']) {
    for (const heldItem of heldItems) {
      for (const [pi, p] of petProfiles.entries()) {
        if (pi > 0 && !(type in pets.PET_PERKS)) continue;
        out.petStats.push({ pet: { type, tier, heldItem }, profile: p, out: P.petStats({ type, tier, heldItem }, p) });
      }
    }
  }
}
// Explicit levels (pets from the profile never carry one, but computePetStats reads it).
for (const type of ['GOLDEN_DRAGON', 'ENDER_DRAGON', 'TIGER', 'JELLYFISH', 'LION', 'SKELETON']) {
  for (const level of [1, 2, 50, 99, 100, 101, 150, 200, 250, 0, -5]) {
    for (const tier of P.petTiers(type)) out.petStats.push({ pet: { type, tier, heldItem: null, level }, profile: { bank: 1e8 }, out: P.petStats({ type, tier, heldItem: null, level }, { bank: 1e8 }) });
  }
}

// ---- requirements ----
const players = [
  null,
  { cataLevel: 0, skills: null, slayers: null },
  { cataLevel: 24, skills: { COMBAT: 24, FARMING: 10, MINING: 30 }, slayers: { zombie: 5, spider: 3, wolf: 4, enderman: 2, blaze: 1, vampire: 0 } },
  { cataLevel: 36, skills: null, slayers: { zombie: 9, spider: 9, wolf: 9, enderman: 9, blaze: 9, vampire: 5 } },
  { cataLevel: 50, skills: { COMBAT: 60, FARMING: 60, MINING: 60, FORAGING: 54, FISHING: 50, ENCHANTING: 60, ALCHEMY: 50, TAMING: 60, CARPENTRY: 50, RUNECRAFTING: 25, SOCIAL: 25, HUNTING: 25 }, slayers: null },
];
const withReqs = ids.filter((id) => resource.get(id)?.requirements || resource.get(id)?.catacombs_requirements);
out.requirements = players.map((player) => ({ player, failing: withReqs.filter((id) => !meetsRequirements(resource.get(id), player)) }));
out.requirementsChecked = withReqs.length;

// ---- drops ----
out.drops = {
  floorDrops: ids.filter((id) => D.isFloorDrop(id, resource)),
  baseRarity: ids.filter((id) => D.dropBaseRarity(id, resource) !== (resource.get(id)?.tier ?? 'COMMON')).map((id) => [id, D.dropBaseRarity(id, resource)]),
  rarityMissing: D.dropBaseRarity('NOT_AN_ITEM', resource),
  highestFloor: Array.from({ length: 52 }, (_, c) => D.highestFloor(c)).concat([D.highestFloor(Infinity), D.highestFloor(NaN), D.highestFloor(undefined)]),
  floorTierIndex: [0, 1, 3, 14, 24, 33, 36, 50, Infinity].flatMap((c) => [1, 3, 7, 10].map((len) => [c, len, D.floorTierIndex(c, len)])),
  buyPriceId: ids.filter((id) => D.isFloorDrop(id, resource)).flatMap((id) => [0, 5, 20, 34, 36].map((c) => [id, c, D.buyPriceId(id, resource, c)])),
};

// ---- cleanName ----
const names = [...resource.values()].map((i) => i.name).filter((n) => typeof n === 'string');
out.cleanName = [...names.filter((n) => /[%§]|^\s|\s$/.test(n)), ...names.filter(() => rand() < 0.05),
  '%%light_purple%%Hyperion', '§dHyperion §r', '  Plain  ', '%%%abc%%x', '%%A_B%%y%%z', '§', 'a§\nb', '﻿x\u0085', '%%RED%%§l§c Big ', '§§a', '%%_%%', '%%%%', '%%a%b%%']
  .map((n) => [n, cleanName(n)]);

// ---- formulas ----
const r = (lo, hi) => lo + rand() * (hi - lo);
const fm = [];
const push = (fn, args, value) => fm.push({ fn, args, out: value });
const blessingSets = [undefined, constants.blessingsWith({ power: 10, wisdom: 3, time: 0 }), constants.blessingsWith({ power: 29, wisdom: 14, time: 5 }), constants.blessingsWith({ power: 5, wisdom: 0, time: 0 })];
for (let i = 0; i < 400; i++) {
  const b = pick(blessingSets);
  const levels = b ? Object.fromEntries(Object.entries(b).map(([k, v]) => [k, v.level])) : null;
  const effect = F.blessingEffect(pick([0, 0.01 * Math.floor(r(0, 11))]), pick([0, 0.01 * Math.floor(r(0, 11))]));
  const v = r(0, 20000);
  const a1 = r(0, 1); const a2 = r(0, 0.5); const m = r(1, 1.3);
  push('calcStrength', [v, a1, a2, m, effect, levels], F.calcStrength(v, a1, a2, m, effect, b));
  push('calcCritDamage', [v, a1, a2, m, effect, levels], F.calcCritDamage(v, a1, a2, m, effect, b));
  push('calcIntelligence', [v, a1, a2, m, effect, levels], F.calcIntelligence(v, a1, a2, m, effect, b));
  push('calcCritChance', [v / 20, a1, m], F.calcCritChance(v / 20, a1, m));
  push('blessingEffect', [a1, a2], F.blessingEffect(a1, a2));
  const s = { damage: r(5, 3000), strength: r(0, 15000), critDamage: r(0, 12000), additiveDamage: r(0, 1500), critChance: pick([r(0, 100), r(100, 200), r(200, 300), 100, 200]), intelligence: r(0, 20000), abilityDamage: r(0, 300) };
  const def = pick([0, 9000, 12500]);
  push('expectedHit', [s, def, true], F.expectedHit(s, def, true));
  push('expectedHit', [s, def, false], F.expectedHit(s, def, false));
  const cm = r(1, 2);
  push('archerDamage', [s, def, cm, i % 2 === 0], F.archerDamage(s, def, cm, i % 2 === 0));
  const bad = r(0, 200); const bm = r(1, 2);
  push('berserkerDamage', [s, def, bad, bm, i % 3 === 0], F.berserkerDamage(s, def, bad, bm, i % 3 === 0));
  push('mageDamage', [s, def, bm], F.mageDamage(s, def, bm));
  const cb = F.cataBoost(Math.floor(r(0, 55)));
  push('rcmDamage', [s, def, cb, bad, bm], F.rcmDamage(s, def, cb, bad, bm));
  const lvl = Math.floor(r(0, 60));
  const stars = Math.floor(r(0, 7)); const ms = Math.floor(r(0, 6)); const sd = Math.floor(r(0, 7));
  push('dungeonMultiplier', [lvl, stars, ms, sd], F.dungeonMultiplier(lvl, stars, ms, sd));
  push('dungeonCritMultiplier', [stars, ms, sd], F.dungeonCritMultiplier(stars, ms, sd));
  push('dungeonStat', [v, ms, lvl, sd, stars], F.dungeonStat(v, ms, lvl, sd, stars));
  push('dungeonCritChance', [v, ms, sd, stars], F.dungeonCritChance(v, ms, sd, stars));
  const xp = pick([r(0, 1000), r(0, 1e6), r(0, 6e8), r(0, 1e9), Math.floor(r(0, 6e8))]);
  push('cataLevelFromXp', [xp], F.cataLevelFromXp(xp));
  push('skillLevelFromXp', [xp / 5, 60], F.skillLevelFromXp(xp / 5, 60));
  push('skillLevelFromXp', [xp / 5, 50], F.skillLevelFromXp(xp / 5, 50));
  push('secretDigits', [pick([0, -1, r(0, 10), r(0, 1e3), r(0, 1e6), Math.floor(r(0, 1e5))])], null);
  fm.at(-1).out = F.secretDigits(fm.at(-1).args[0]);
  push('powerMultiplier', [Math.floor(r(0, 3000))], null);
  fm.at(-1).out = F.powerMultiplier(fm.at(-1).args[0]);
  push('defenseMultiplier', [def], F.defenseMultiplier(def));
  push('calcDamage', [v], F.calcDamage(v));
}
for (let l = -2; l <= 60; l++) push('cataBoost', [l], F.cataBoost(l));
for (const l of [2.5, NaN, Infinity, -Infinity]) push('cataBoost', [l], F.cataBoost(l));
for (const x of [0, 49, 50, 124, 125, 569_809_640, 569_809_639, 1e12, -5, NaN]) {
  push('cataLevelFromXp', [x], F.cataLevelFromXp(x));
  push('skillLevelFromXp', [x, 60], F.skillLevelFromXp(x, 60));
  push('skillLevelFromXp', [x, -10], F.skillLevelFromXp(x, -10));
  push('skillLevelFromXp', [x, 100], F.skillLevelFromXp(x, 100));
}
out.formulas = fm;
// applyBlessings with explicit blessing objects (the wiki example in formulas.test.js).
out.applyBlessings = [
  { value: 6000, stat: 'strength', effect: 1.1 * 1.25 * 1.2, blessings: { power: { level: 29, stats: ['strength'] }, time: { level: 5, stats: ['strength'] } } },
  { value: 1000, stat: 'critDamage', effect: 1.2, blessings: constants.BLESSINGS },
  { value: 500, stat: 'intelligence', effect: 1.452, blessings: constants.BLESSINGS },
  { value: 500, stat: 'speed', effect: 1.452, blessings: constants.BLESSINGS },
].map((c) => ({ ...c, out: F.applyBlessings(c.value, c.stat, c.effect, c.blessings) }));

writeFileSync('/td/core.json', JSON.stringify(out, replacer));
console.log(`core.json: ${out.petStats.length} pet stats, ${out.formulas.length} formula vectors, ${withReqs.length} items with requirements`);
process.exit(0);
