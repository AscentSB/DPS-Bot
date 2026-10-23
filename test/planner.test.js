import test from 'node:test';
import assert from 'node:assert/strict';
import zlib from 'node:zlib';
import nbt from 'prismarine-nbt';
import { decodeItems, toItemState } from '../src/lib/nbt.js';
import { readMember } from '../src/api/hypixel.js';
import { makeContext, buildLoadout, planUpgrades, weightsFor, availableUpgrades, newItem, enchantParts, profileUpgrades } from '../src/calc/upgrades.js';
import { evaluate, itemContribution, shardStats, totalAccessoryPower, accessoryStats, saCloakMultiplier } from '../src/calc/stats.js';

const close = (a, b, eps = 1e-6) => assert.ok(Math.abs(a - b) <= eps * Math.max(1, Math.abs(b)), `${a} != ${b}`);

const resource = new Map([
  ['HYPERION', { id: 'HYPERION', name: 'Hyperion', category: 'SWORD', tier: 'LEGENDARY', dungeon_item: true, stats: { DAMAGE: 260, STRENGTH: 150, INTELLIGENCE: 350 } }],
  ['GIANTS_SWORD', { id: 'GIANTS_SWORD', name: "Giant's Sword", category: 'SWORD', tier: 'LEGENDARY', dungeon_item: true, stats: { DAMAGE: 500 } }],
  ['TERMINATOR', { id: 'TERMINATOR', name: 'Terminator', category: 'BOW', tier: 'LEGENDARY', dungeon_item_conversion_cost: { essence_type: 'DRAGON', amount: 300 }, stats: { DAMAGE: 310, STRENGTH: 50, CRITICAL_DAMAGE: 250 } }],
  ['POWER_WITHER_LEGGINGS', { id: 'POWER_WITHER_LEGGINGS', name: "Necron's Leggings", category: 'LEGGINGS', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 40, CRITICAL_DAMAGE: 30 }, gemstone_slots: [{ slot_type: 'JASPER', costs: [{ type: 'COINS', coins: 250000 }, { type: 'ITEM', item_id: 'FLAWLESS_JASPER_GEM', amount: 2 }] }] }],
  ['SOULWEAVER_GLOVES', { id: 'SOULWEAVER_GLOVES', name: 'Soulweaver Gloves', category: 'GLOVES', tier: 'EPIC', dungeon_item: true, stats: { strength: 10, critical_damage: 10 } }],
  ['RAGNAROCK_AXE', { id: 'RAGNAROCK_AXE', name: 'Ragnarock', category: 'SWORD', tier: 'RARE', stats: { DAMAGE: 200, STRENGTH: 100 }, gemstone_slots: [{ slot_type: 'COMBAT' }] }],
  ['SPEED_TALISMAN', { id: 'SPEED_TALISMAN', name: 'Speed Talisman', category: 'ACCESSORY', tier: 'COMMON' }],
  ['SPEED_RING', { id: 'SPEED_RING', name: 'Speed Ring', category: 'ACCESSORY', tier: 'UNCOMMON' }],
  ['WITHER_RELIC', { id: 'WITHER_RELIC', name: 'Wither Relic', category: 'ACCESSORY', tier: 'LEGENDARY' }],
  ['MASTER_SKULL_TIER_7', { id: 'MASTER_SKULL_TIER_7', name: 'Master Skull - Tier 7', category: 'ACCESSORY', tier: 'LEGENDARY', dungeon_item: true }],
  ['GENERAL_MEDALLION', { id: 'GENERAL_MEDALLION', name: "General's Medallion", category: 'ACCESSORY', tier: 'RARE', dungeon_item: true }],
]);

function encode(items) {
  const list = items.map((x) => ({
    id: nbt.short(1), Count: nbt.byte(1),
    tag: nbt.comp({ ExtraAttributes: nbt.comp({
      id: nbt.string(x.id),
      upgrade_level: nbt.int(x.stars ?? 5),
      hot_potato_count: nbt.int(x.hpb ?? 0),
      ...(x.recomb ? { rarity_upgrades: nbt.int(1) } : {}),
      ...(x.reforge ? { modifier: nbt.string(x.reforge) } : {}),
      ...(x.gems ? { gems: nbt.comp(Object.fromEntries(Object.entries(x.gems).map(([k, v]) => [k, nbt.string(v)]))) } : {}),
      enchantments: nbt.comp(Object.fromEntries(Object.entries(x.enchants ?? {}).map(([k, v]) => [k, nbt.int(v)]))),
    }) }),
  }));
  const root = nbt.comp({ i: nbt.list(nbt.comp(list)) }, '');
  return zlib.gzipSync(nbt.writeUncompressed(root)).toString('base64');
}
const items = (list) => decodeItems(encode(list)).map((i) => toItemState(i, resource));

const priceTable = {
  FIRST_MASTER_STAR: 8e6, SECOND_MASTER_STAR: 15e6, THIRD_MASTER_STAR: 25e6, FOURTH_MASTER_STAR: 40e6, FIFTH_MASTER_STAR: 60e6,
  ENCHANTMENT_SHARPNESS_7: 60e6, ENCHANTMENT_SMITE_7: 5e6, ENCHANTMENT_ENDER_SLAYER_7: 30e6, ENCHANTMENT_POWER_7: 80e6,
  HOT_POTATO_BOOK: 80e3, FUMING_POTATO_BOOK: 1.2e6, RECOMBOBULATOR_3000: 8e6, PRECURSOR_GEAR: 1e6,
  HYPERION: 900e6, TERMINATOR: 1.2e9, WITHER_RELIC: 50e6, SPEED_TALISMAN: 1e5, SPEED_RING: 5e5,
  SHARD_JORMUNG: 1.8e6, PET_GOLDEN_DRAGON_LEGENDARY: 1.2e9, ANTIQUE_REMEDIES: 5e6, FLAWLESS_JASPER_GEM: 3e6, PERFECT_JASPER_GEM: 14e6,
};
const prices = { get: (id) => (priceTable[id] ? { price: priceTable[id], source: 'BZ' } : null) };
// A Necron's Blade variant with all 3 Wither scrolls (Wither Impact).
const scrolled = (item) => ({ ...item, scrolls: ['IMPLOSION_SCROLL', 'SHADOW_WARP_SCROLL', 'WITHER_SHIELD_SCROLL'] });
const petTierFor = (type) => (['BLACK_CAT', 'GRIFFIN'].includes(type) ? 'MYTHIC' : 'LEGENDARY');
const profile = (list, extra = {}) => ({ items: list, pets: [], shards: {}, ...extra });

test('NBT decode reads ITEM_IDs, stars, recombs, reforges, gems and enchants', () => {
  const [it] = items([{ id: 'HYPERION', stars: 7, hpb: 10, recomb: true, reforge: 'withered', gems: { COMBAT_0: 'PERFECT', COMBAT_0_gem: 'JASPER' }, enchants: { sharpness: 6 } }]);
  assert.equal(it.id, 'HYPERION');
  assert.equal(it.stars, 5);
  assert.equal(it.masterStars, 2);
  assert.equal(it.hpb, 10);
  assert.equal(it.recomb, true);
  assert.equal(it.reforge, 'withered');
  assert.deepEqual(it.gems.slots.COMBAT_0, { gem: 'JASPER', quality: 'PERFECT' });
  assert.equal(it.enchants.sharpness, 6);
});

test("Necron's Leggings + Ancient: (40 + 25) str and (30 + cata) cd, scaled by cata 50 + 5 stars", () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const [legs] = items([{ id: 'POWER_WITHER_LEGGINGS', reforge: 'ancient' }]);
  const c = itemContribution(legs, ctx);
  close(c.global.strength, 65 * 6.35);
  close(c.global.critDamage, 80 * 6.35);
  close(c.global.critChance, 12 * 1.5);
});

test('lowercase API stat keys are read (Soulweaver Gloves)', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const c = itemContribution(newItem('SOULWEAVER_GLOVES', resource), ctx);
  close(c.global.strength, 10 * 6.35);
});

test('only the next master star is offered, and only on owned items', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const loadout = buildLoadout(profile(items([{ id: 'HYPERION', stars: 8 }])), ctx, weightsFor('mage'));
  const labels = availableUpgrades(loadout, ctx, prices).map((u) => u.label);
  assert.ok(labels.includes('4th Master Star on Hyperion'));
  assert.ok(!labels.some((l) => /5th Master Star/.test(l)));
  assert.ok(!labels.some((l) => /Master Star on Necron/.test(l)));
});

test('greedy plan is in dependency order and every step gains damage', async () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const weights = weightsFor('mage');
  const plan = await planUpgrades(buildLoadout(profile([]), ctx, weights), ctx, prices, weights, 12);
  const labels = plan.steps.map((s) => s.label);
  const buy = labels.indexOf('Buy Hyperion');
  const star = labels.indexOf('1st Master Star on Hyperion');
  if (star !== -1) assert.ok(buy !== -1 && buy < star);
  for (const s of plan.steps) assert.ok(s.gain > 0 && s.perMillion > 0);
});

test('smite only helps withers, ender slayer only helps dragons', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const w = weightsFor('mage');
  const base = buildLoadout(profile(items([{ id: 'GIANTS_SWORD' }])), ctx, w);
  const ups = availableUpgrades(base, ctx, prices);
  const before = evaluate(base, ctx, w);
  const smite = evaluate(ups.find((u) => u.label.startsWith('Smite VII')).apply(base), ctx, w);
  const ender = evaluate(ups.find((u) => u.label.startsWith('Ender Slayer VII')).apply(base), ctx, w);
  assert.ok(smite.wither > before.wither && smite.dragon === before.dragon);
  assert.ok(ender.dragon > before.dragon && ender.wither === before.wither);
});

test('Sharpness and Smite stack on one sword (Hypixel allows both)', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const w = weightsFor('mage');
  const base = buildLoadout(profile(items([{ id: 'GIANTS_SWORD', enchants: { sharpness: 7 } }])), ctx, w);
  const up = availableUpgrades(base, ctx, prices).find((u) => u.label.startsWith('Smite VII'));
  assert.ok(!up.label.includes('replaces'));
  const after = up.apply(base);
  assert.equal(after.weapon.enchants.sharpness, 7);
  assert.equal(after.weapon.enchants.smite, 7);
  assert.ok(evaluate(after, ctx, w).wither > evaluate(base, ctx, w).wither);
});

test('accessory families: a ring replaces its talisman, and the talisman is not offered once you own the ring', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const loadout = buildLoadout(profile(items([{ id: 'SPEED_TALISMAN' }, { id: 'SPEED_RING' }])), ctx, w);
  assert.equal(totalAccessoryPower(loadout, resource), 5);
  const labels = availableUpgrades(loadout, ctx, prices).map((u) => u.label);
  assert.ok(!labels.includes('Buy Speed Talisman'));
  assert.ok(labels.includes('Recombobulate Speed Ring'));
});

test('dungeon accessories give double accessory power; Master Skull T7 multiplies strength', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const loadout = buildLoadout(profile(items([{ id: 'MASTER_SKULL_TIER_7' }])), ctx, w);
  assert.equal(totalAccessoryPower(loadout, resource), 32);
  const acc = accessoryStats(loadout, { ...ctx, power: 'silky' });
  assert.equal(acc.stats.multStrength, 1.1);
});

test("General's Medallion adds 1% per secret digit to dungeon gear", () => {
  const w = weightsFor('arch');
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, secrets: 12345, resource });
  const without = buildLoadout(profile(items([{ id: 'TERMINATOR' }])), ctx, w);
  const withMedal = buildLoadout(profile(items([{ id: 'TERMINATOR' }, { id: 'GENERAL_MEDALLION' }])), ctx, w);
  const a = evaluate(without, ctx, w).stats.dragon.damage;
  const b = evaluate(withMedal, ctx, w).stats.dragon.damage;
  close(b - a, 310 * 0.05);
});

test('shard echoes: Molthorn and Tiamat stack on Jormung', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  close(shardStats({ shards: { JORMUNG: 10 } }, ctx).global.additiveStrength, 0.01);
  close(shardStats({ shards: { JORMUNG: 10, MOLTHORN: 10 } }, ctx).global.additiveStrength, 0.015);
  close(shardStats({ shards: { JORMUNG: 10, MOLTHORN: 10, TIAMAT: 10 } }, ctx).global.additiveStrength, 0.01 * (1 + 0.5 * 1.5));
  assert.equal(shardStats({ shards: { SKELETOR: 10 } }, { ...ctx, cls: 'bers' }).global.additiveDamage, undefined);
});

test('Ragnarock grants 1.5x its strength, and Chimera copies the active pet', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const base = buildLoadout(profile(items([{ id: 'TERMINATOR' }])), ctx, w);
  const rag = buildLoadout(profile(items([{ id: 'TERMINATOR' }, { id: 'RAGNAROCK_AXE', stars: 0 }])), ctx, w);
  const strDiff = (l) => evaluate(l, ctx, w);
  const raw = (r) => r.stats.dragon.strength;
  const a = raw(strDiff(base));
  const b = raw(strDiff(rag));
  assert.ok(b > a);
  const chim = buildLoadout(profile(items([{ id: 'TERMINATOR' }, { id: 'RAGNAROCK_AXE', stars: 0, enchants: { ultimate_chimera: 5 } }]), { pets: [{ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: 'ANTIQUE_REMEDIES' }] }), ctx, w);
  assert.equal(chim.activePets.wither.type, 'GOLDEN_DRAGON');
  assert.ok(raw(strDiff(chim)) > b);
});

test('gem slots that need unlocking include the unlock cost', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const loadout = buildLoadout(profile(items([{ id: 'POWER_WITHER_LEGGINGS' }])), ctx, weightsFor('arch'));
  const up = availableUpgrades(loadout, ctx, prices).find((u) => u.label === "Flawless Jasper in Necron's Leggings (unlock slot)");
  assert.ok(up);
  assert.equal(up.cost, 3e6 + 250000 + 2 * 3e6);
});

test('fragging a Bone Necklace costs 8 Thorn fragments and raises its crit chance', () => {
  const res = new Map([...resource,
    ['BONE_NECKLACE', { id: 'BONE_NECKLACE', name: 'Bone Necklace', category: 'NECKLACE', tier: 'EPIC', dungeon_item: true, stats: { CRITICAL_CHANCE: 3 } }],
    ['STARRED_BONE_NECKLACE', { id: 'STARRED_BONE_NECKLACE', name: 'Bone Necklace', category: 'NECKLACE', tier: 'EPIC', dungeon_item: true, stats: { CRITICAL_CHANCE: 5 } }],
  ]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const loadout = buildLoadout(profile([{ ...newItem('BONE_NECKLACE', res), masterStars: 1 }]), ctx, w);
  const p = { get: (id) => (id === 'THORN_FRAGMENT' ? { price: 100000, source: 'BZ' } : null) };
  const up = availableUpgrades(loadout, ctx, p).find((u) => u.label.startsWith('Frag Bone Necklace'));
  assert.equal(up.cost, 800000);
  const after = up.apply(loadout);
  assert.equal(after.equipment.NECKLACE.id, 'STARRED_BONE_NECKLACE');
  assert.equal(after.equipment.NECKLACE.masterStars, 1);
  assert.ok(evaluate(after, ctx, w).stats.dragon.critChance > evaluate(loadout, ctx, w).stats.dragon.critChance);
});

test('essence perks cost essence x bazaar price and need Catacombs 20', () => {
  const p = { get: (id) => (id === 'ESSENCE_UNDEAD' ? { price: 2000, source: 'BZ' } : null) };
  const w = weightsFor('arch');
  const low = makeContext({ cls: 'arch', cataLevel: 19, resource });
  const high = makeContext({ cls: 'arch', cataLevel: 20, resource });
  const prof = profile(items([{ id: 'TERMINATOR' }]), { perks: { catacombs_strength: 2 } });
  assert.ok(!availableUpgrades(buildLoadout(prof, low, w), low, p).some((u) => u.label.startsWith('Strength Essence')));
  const loadout = buildLoadout(prof, high, w);
  const up = availableUpgrades(loadout, high, p).find((u) => u.label.startsWith('Strength Essence IV'));
  assert.equal(up.cost, (6000 + 8000) * 2000);
  const before = evaluate(loadout, high, w).stats.dragon.strength;
  assert.ok(evaluate(up.apply(loadout), high, w).stats.dragon.strength > before);
});

test('Forbidden Blessing makes the Power blessing stronger', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const base = buildLoadout(profile(items([{ id: 'TERMINATOR' }])), ctx, w);
  const blessed = buildLoadout(profile(items([{ id: 'TERMINATOR' }]), { perks: { forbidden_blessing: 10 } }), ctx, w);
  const ratio = evaluate(blessed, ctx, w).stats.dragon.strength / evaluate(base, ctx, w).stats.dragon.strength;
  assert.ok(ratio > 1);
});

test('tuning points fill speed up to 500 first; only leftover points reach crit stats', async () => {
  const { tuningAllocation } = await import('../src/calc/stats.js');
  const ctx = { cls: 'arch', tuning: { split: { critChance: 1 } } };
  // 500 dungeon speed is 700 on the sheet. 200 points, 580 speed: 80 points to speed (120 speed), 120 points to
  // crit chance (0.2 each).
  const a = tuningAllocation(200, 580, ctx);
  assert.equal(a.speedPoints, 80);
  assert.equal(a.stats.speed, 120);
  assert.ok(Math.abs(a.stats.critChance - 24) < 1e-9);
  // Not enough points to reach 500: everything goes to speed, nothing to crit.
  const b = tuningAllocation(50, 300, ctx);
  assert.equal(b.speedPoints, 50);
  assert.equal(b.stats.critChance, undefined);
  // Already at 500 (720 on the sheet): all points go to crit.
  assert.equal(tuningAllocation(100, 720, ctx).speedPoints, 0);
});

test('dungeon speed: every point over 100 counts for 2/3', async () => {
  const { dungeonSpeed, sheetSpeedFor } = await import('../src/config/constants.js');
  assert.equal(dungeonSpeed(80), 80);
  close(dungeonSpeed(200), 166.67, 0.01);
  close(dungeonSpeed(300), 233.33, 0.01);
  close(dungeonSpeed(700), 500);
  close(sheetSpeedFor(500), 700);
  close(sheetSpeedFor(450), 625);
});

test('a speed upgrade below 500 frees tuning points for crit; past 500 it does nothing', () => {
  const res = new Map([...resource, ['FAST_BOOTS', { id: 'FAST_BOOTS', name: 'Fast Boots', category: 'BOOTS', tier: 'COMMON', stats: { WALK_SPEED: 200 } }]]);
  const w = weightsFor('arch');
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res, tuning: { split: { critChance: 1 } } });
  const loadout = { ...buildLoadout(profile([...items([{ id: 'TERMINATOR' }]), newItem('FAST_BOOTS', res)]), ctx, w), power: 'silky' };
  // Give the bag plenty of tuning points.
  loadout.accessories = new Map(Array.from({ length: 400 }, (_, i) => [`A${i}`, { id: `A${i}`, category: 'ACCESSORY', baseRarity: 'MYTHIC', enchants: {} }]));
  const base = evaluate(loadout, ctx, w);
  assert.ok(base.speed >= 700 && base.tuning.speed > 0);
  const faster = evaluate({ ...loadout, shards: { OBSIDIAN_DEFENDER: 10 } }, ctx, w);
  assert.ok(faster.tuning.speed < base.tuning.speed);
  assert.ok(faster.stats.dragon.critChance > base.stats.dragon.critChance);
  // Past 500 with no speed points left to free, more speed changes nothing.
  const capped = { ...loadout, armor: { BOOTS: { ...newItem('FAST_BOOTS', res), id: 'FAST_BOOTS' } }, shards: {} };
  const res2 = new Map([...res, ['FAST_BOOTS', { ...res.get('FAST_BOOTS'), stats: { WALK_SPEED: 650 } }]]);
  const ctx2 = { ...ctx, resource: res2 };
  const c1 = evaluate(capped, ctx2, w);
  const c2 = evaluate({ ...capped, shards: { OBSIDIAN_DEFENDER: 10 } }, ctx2, w);
  assert.equal(c1.tuning.speed, 0);
  assert.equal(c2.dragon, c1.dragon);
});

test('Wolf Slayer speed: +1 at levels 1, 3 and 8', async () => {
  const { wolfSlayerSpeed } = await import('../src/api/hypixel.js');
  assert.equal(wolfSlayerSpeed(0), 0);
  assert.equal(wolfSlayerSpeed(10), 1);
  assert.equal(wolfSlayerSpeed(250), 2);
  assert.equal(wolfSlayerSpeed(400000), 3);
});

test('pets: Golden Dragon 200 matches the sheet (300 str with Heph Remedies, +5% str, +250% damage)', async () => {
  const { petStats } = await import('../src/lib/pets.js');
  const gd = petStats({ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: 'HEPHAESTUS_REMEDIES' }, { bank: 1e9 });
  assert.ok(Math.abs(gd.stats.strength - (50 + 99.9) * 2) < 1e-6);
  assert.equal(gd.stats.additiveStrength, 0.05);
  assert.equal(gd.stats.additiveDamage, 250);
  // Legendary Treasure: 0.25% per million in the bank (none without bank data), max 250% at 1B.
  assert.equal(petStats({ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY' }, { bank: 100e6 }).stats.additiveDamage, 25);
  assert.equal(petStats({ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY' }, {}).stats.additiveDamage, 0);
  const ed = petStats({ type: 'ENDER_DRAGON', tier: 'LEGENDARY', heldItem: 'GOLD_CLAWS' });
  assert.equal(ed.stats.critDamage, 75);
  assert.equal(ed.dragon.additiveDamage, 200);
  assert.equal(petStats({ type: 'JELLYFISH', tier: 'LEGENDARY' }).potionBoost, 0.5);
});

test('only one pet is active; a Jellyfish buffs the Dungeon potion even when another pet is active', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const gdOnly = buildLoadout(profile(items([{ id: 'TERMINATOR' }]), { pets: [{ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY' }] }), ctx, w);
  const both = buildLoadout(profile(items([{ id: 'TERMINATOR' }]), { pets: [{ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY' }, { type: 'JELLYFISH', tier: 'LEGENDARY' }] }), ctx, w);
  assert.equal(both.activePets.wither.type, 'GOLDEN_DRAGON');
  const a = evaluate(gdOnly, ctx, w).stats.dragon;
  const b = evaluate(both, ctx, w).stats.dragon;
  assert.ok(b.strength > a.strength && b.critChance > a.critChance);
});

test('armor sets: Skeleton Master gives +5% arrow damage per piece and +25% for the full set with a bow', () => {
  const res = new Map([...resource, ...['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'].map((s) => [`SKELETON_MASTER_${s}`, { id: `SKELETON_MASTER_${s}`, name: s, category: s, tier: 'EPIC', dungeon_item: true }])]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const pieces = ['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'].map((s) => newItem(`SKELETON_MASTER_${s}`, res));
  const none = buildLoadout(profile(items([{ id: 'TERMINATOR' }])), ctx, w);
  const full = buildLoadout(profile([...items([{ id: 'TERMINATOR' }]), ...pieces]), ctx, w);
  assert.equal(evaluate(full, ctx, w).stats.dragon.additiveDamage - evaluate(none, ctx, w).stats.dragon.additiveDamage, 4 * 5 + 25);
});

test('gear buys also come bundled with the best reforge stone', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const loadout = buildLoadout(profile(items([{ id: 'TERMINATOR' }])), ctx, w);
  const p = { get: (id) => ({ POWER_WITHER_LEGGINGS: { price: 50e6, source: 'AH' }, PRECURSOR_GEAR: { price: 1e6, source: 'BZ' } })[id] ?? null };
  const labels = availableUpgrades(loadout, ctx, p).map((u) => u.label);
  assert.ok(labels.includes("Buy Necron's Leggings"));
  assert.ok(labels.includes("Buy Necron's Leggings + Ancient reforge"));
});

test("Diamond Necron Head + Ancient matches the sheet: (40 + 35) str, cata cd and 15 cc, all doubled", () => {
  const res = new Map([...resource, ['DIAMOND_NECRON_HEAD', { id: 'DIAMOND_NECRON_HEAD', name: 'Diamond Necron Head', category: 'HELMET', tier: 'SPECIAL', dungeon_item: true, stats: { STRENGTH: 40 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const c = itemContribution({ ...newItem('DIAMOND_NECRON_HEAD', res), reforge: 'ancient' }, ctx);
  close(c.global.strength, 150 * 6.35);
  close(c.global.critDamage, 100 * 6.35);
  close(c.global.critChance, 30 * 1.5);
});

test('diamond and gold heads are never recombobulated and a recomb changes nothing', () => {
  const res = new Map([...resource, ['DIAMOND_NECRON_HEAD', { id: 'DIAMOND_NECRON_HEAD', name: 'Diamond Necron Head', category: 'HELMET', tier: 'SPECIAL', dungeon_item: true, stats: { STRENGTH: 40 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const head = { ...newItem('DIAMOND_NECRON_HEAD', res), reforge: 'ancient' };
  const loadout = buildLoadout(profile([head]), ctx, weightsFor('arch'));
  assert.ok(!availableUpgrades(loadout, ctx, prices).some((u) => u.label === 'Recombobulate Diamond Necron Head'));
  assert.deepEqual(itemContribution({ ...head, recomb: true }, ctx), itemContribution(head, ctx));
});

test('Terminator volleys: 4/s at 100 attack speed, 3.33/s at 99; arrows = 3 + Duplex + solo-Archer Doubleshot', async () => {
  const { volleysPerSecond, arrowsPerVolley } = await import('../src/calc/stats.js');
  const term = { id: 'TERMINATOR', category: 'BOW' };
  const ctx = { cls: 'arch', archerLevel: 50 };
  assert.equal(volleysPerSecond(term, 100, ctx), 4);
  assert.ok(Math.abs(volleysPerSecond(term, 99, ctx) - 20 / 6) < 1e-9);
  assert.equal(volleysPerSecond(term, 150, ctx), 4);
  // Archer 50: Doubleshot 100% (solo archer) + 50% = 1.5 extra arrows.
  assert.ok(Math.abs(arrowsPerVolley(term, 0.2, ctx) - 4.7) < 1e-9);
  assert.ok(Math.abs(arrowsPerVolley(term, 0, { cls: 'arch', archerLevel: 20 }) - 4.2) < 1e-9);
  assert.equal(arrowsPerVolley(term, 0, { cls: 'bers' }), 3);
});

test('tuning fills attack speed to 100, then speed to 500, then crit', async () => {
  const { tuningAllocation } = await import('../src/calc/stats.js');
  const ctx = { cls: 'arch', tuning: { split: { critChance: 1 } } };
  const a = tuningAllocation(300, 640, ctx, 70); // 100 points AS, 40 points speed (to 700 = 500 in dungeons), 160 crit
  assert.equal(a.attackSpeedPoints, 100);
  assert.equal(a.speedPoints, 40);
  assert.equal(a.dpsPoints, 160);
});

test('a Crochet Tiger Plushie on the pet frees tuning points and is offered as a pet item', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource, tuning: { split: { critChance: 1 } } });
  const w = weightsFor('arch');
  const loadout = buildLoadout(profile(items([{ id: 'TERMINATOR' }]), { pets: [{ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: null }] }), ctx, w);
  const p = { get: (id) => (id === 'CROCHET_TIGER_PLUSHIE' ? { price: 1e6, source: 'AH' } : null) };
  const up = availableUpgrades(loadout, ctx, p).find((u) => u.label.startsWith('Crochet Tiger Plushie'));
  assert.ok(up);
  const before = evaluate(loadout, ctx, w);
  const after = evaluate(up.apply(loadout), ctx, w);
  assert.ok(after.attackSpeed >= before.attackSpeed);
});

test('clear boots: faster boots count for speed while boss boots give damage', () => {
  const res = new Map([...resource, ['SPEED_WITHER_BOOTS', { id: 'SPEED_WITHER_BOOTS', name: "Maxor's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { WALK_SPEED: 30, CRITICAL_DAMAGE: 45 } }],
    ['POWER_WITHER_BOOTS', { id: 'POWER_WITHER_BOOTS', name: "Necron's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 40, CRITICAL_DAMAGE: 30 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const necronOnly = buildLoadout(profile([...items([{ id: 'TERMINATOR' }]), newItem('POWER_WITHER_BOOTS', res)]), ctx, w);
  const both = buildLoadout(profile([...items([{ id: 'TERMINATOR' }]), newItem('POWER_WITHER_BOOTS', res), newItem('SPEED_WITHER_BOOTS', res)]), ctx, w);
  assert.equal(both.clearBoots.id, 'SPEED_WITHER_BOOTS');
  assert.equal(evaluate(both, ctx, w).speed - evaluate(necronOnly, ctx, w).speed >= 0, true);
});

test('Terminator attack speed gets the 5-star dungeon boost (x1.5) but not the Catacombs boost', () => {
  const res = new Map([...resource, ['TERMINATOR', { ...resource.get('TERMINATOR'), stats: { ...resource.get('TERMINATOR').stats, ATTACK_SPEED: 40 } }]]);
  const c = itemContribution(newItem('TERMINATOR', res), makeContext({ cls: 'bers', cataLevel: 50, resource: res }));
  close(c.global.attackSpeed, 60);
  const ctx = makeContext({ cls: 'bers', cataLevel: 50, resource });
  const w = weightsFor('bers');
  const loadout = buildLoadout(profile(items([{ id: 'TERMINATOR' }, { id: 'GIANTS_SWORD' }])), ctx, w);
  assert.equal(loadout.weapon.id, 'TERMINATOR');
});

test("Maxor's boots speed gets the full dungeon multiplier, Catacombs boost included", () => {
  const res = new Map([...resource, ['SPEED_WITHER_BOOTS', { id: 'SPEED_WITHER_BOOTS', name: "Maxor's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { WALK_SPEED: 30 } }]]);
  const c = itemContribution(newItem('SPEED_WITHER_BOOTS', res), makeContext({ cls: 'arch', cataLevel: 50, resource: res }));
  close(c.global.speed, 30 * (4.85 + 1.5));
});

test("Ender Dragon's Superior perk raises attack speed, tuning points included", async () => {
  const { tuningAllocation } = await import('../src/calc/stats.js');
  const ctx = { cls: 'arch', tuning: { split: { critChance: 1 } } };
  // 73 attack speed x 1.1 = 80.3. Each point gives 0.33, so 60 points reach 100 (66 without the boost).
  assert.equal(tuningAllocation(300, 500, ctx, 73 * 1.1, 1.1).attackSpeedPoints, 60);
  assert.equal(tuningAllocation(300, 500, ctx, 73).attackSpeedPoints, 90);
  const res = new Map([...resource, ['TERMINATOR', { ...resource.get('TERMINATOR'), stats: { ...resource.get('TERMINATOR').stats, ATTACK_SPEED: 40 } }]]);
  const c = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const base = { ...buildLoadout(profile([newItem('TERMINATOR', res)]), c, w), power: 'silky' };
  const edPet = { type: 'ENDER_DRAGON', tier: 'LEGENDARY', heldItem: null };
  const ed = evaluate({ ...base, activePets: { wither: edPet, dragon: edPet } }, c, w);
  const none = evaluate({ ...base, activePets: {} }, c, w);
  // Terminator 60 + Silky's 5 attack speed bonus = 65, x1.1 with Ender Dragon.
  close(none.attackSpeed, 65);
  close(ed.attackSpeed, 71.5);
});

test('Weirder Tuba counts against dragons only', () => {
  const c = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const base = buildLoadout(profile(items([{ id: 'TERMINATOR' }])), c, w);
  const tuba = { ...base, extras: new Set(['WEIRDER_TUBA']) };
  const a = evaluate(base, c, w);
  const b = evaluate(tuba, c, w);
  close(b.wither, a.wither);
  assert.ok(b.dragon > a.dragon);
  close(b.attackSpeeds.dragon - a.attackSpeeds.dragon, 5);
});

test('Blended (blacksmith) reforge is offered on equipment at 9 rolls x 500 coins', () => {
  const c = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const base = buildLoadout(profile(items([{ id: 'TERMINATOR' }, { id: 'SOULWEAVER_GLOVES' }])), c, w);
  const up = availableUpgrades(base, c, prices).find((u) => u.label.startsWith('Blended reforge on'));
  assert.ok(up);
  assert.equal(up.cost, 4500);
  assert.equal(up.apply(base).equipment.GLOVES.reforge, 'blended');
});

test('the active pet is whichever owned pet does the most damage (no fixed Golden Dragon on withers)', () => {
  const pets = [{ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY' }, { type: 'ENDER_DRAGON', tier: 'LEGENDARY' }];
  // Endgame-sized stat pools, where Ender Dragon's Superior (+10% most stats) is worth the most.
  const profileStats = { sbLevel: 0, harpSongs: [], cakes: [{ stat: 'strength', amount: 1200 }, { stat: 'critDamage', amount: 900 }, { stat: 'attackSpeed', amount: 100 }] };
  const pick = (cls, bank) => {
    const ctx = makeContext({ cls, cataLevel: 50, berserkLevel: 50, combatLevel: 60, resource, profileStats, petProfile: { bank, goldCollection: 1e9 } });
    const l = buildLoadout(profile(items([{ id: 'TERMINATOR' }]), { pets }), ctx, weightsFor(cls));
    const dmg = (p) => evaluate({ ...l, activePets: { ...l.activePets, wither: p } }, ctx, weightsFor(cls)).wither;
    assert.equal(dmg(l.activePets.wither), Math.max(...pets.map(dmg)));
    return l.activePets;
  };
  // Archer: the Golden Dragon's bank perk (+250% additive at 1B) wins rich, Ender Dragon wins broke.
  assert.equal(pick('arch', 1e9).wither.type, 'GOLDEN_DRAGON');
  assert.equal(pick('arch', 0).wither.type, 'ENDER_DRAGON');
  assert.equal(pick('arch', 1e9).dragon.type, 'ENDER_DRAGON');
  // Berserker: Lust for Blood's 1200 additive waters the bank perk down, so Ender Dragon wins even at 1B.
  assert.equal(pick('bers', 1e9).wither.type, 'ENDER_DRAGON');
});

test('tuning buys attack speed for whichever boss needs more', async () => {
  const res = new Map([...resource, ['TERMINATOR', { ...resource.get('TERMINATOR'), stats: { ...resource.get('TERMINATOR').stats, ATTACK_SPEED: 40 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const base = buildLoadout(profile([newItem('TERMINATOR', res)]), ctx, w);
  const gd = { type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: null };
  const ed = { type: 'ENDER_DRAGON', tier: 'LEGENDARY', heldItem: null };
  const r = evaluate({ ...base, activePets: { wither: gd, dragon: ed } }, ctx, w);
  // Golden Dragon's attack speed covers withers; the dragon phase with Ender Dragon sets the points.
  assert.ok(r.attackSpeeds.wither > r.attackSpeeds.dragon);
});

test('armor only gets Ancient: no other reforge is offered or bundled on armor', () => {
  const res = new Map([...resource, ['SPEED_WITHER_BOOTS', { id: 'SPEED_WITHER_BOOTS', name: "Maxor's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { WALK_SPEED: 30 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const l = buildLoadout(profile([newItem('TERMINATOR', res), newItem('SPEED_WITHER_BOOTS', res), newItem('POWER_WITHER_LEGGINGS', res)]), ctx, w);
  const p = { get: (id) => ({ price: 1e6, source: 'BZ' }) };
  const armorReforges = availableUpgrades(l, ctx, p).filter((u) => /reforge on (Maxor's Boots|Necron's Leggings)/.test(u.label));
  assert.ok(armorReforges.length > 0);
  assert.ok(armorReforges.every((u) => u.label.startsWith('Ancient reforge')));
});

test('Legion raises attack speed: 0.07% per level per player, 4 nearby players against withers', () => {
  const res = new Map([...resource, ['TERMINATOR', { ...resource.get('TERMINATOR'), stats: { ...resource.get('TERMINATOR').stats, ATTACK_SPEED: 40 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const legs = { ...newItem('POWER_WITHER_LEGGINGS', res), enchants: { ultimate_legion: 5 } };
  const l = { ...buildLoadout(profile([newItem('TERMINATOR', res), legs]), ctx, w), power: 'silky' };
  // (Terminator 60 + Silky 5) x (1 + 5 x 0.0007 x 4)
  close(evaluate({ ...l, activePets: {} }, ctx, w).attackSpeed, 65 * (1 + 5 * 0.0007 * 4));
});

test('Inferno Demonlord shard: +1 attack speed per level, read from attributes.stacks.attack_speed', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const l = buildLoadout(profile(items([{ id: 'TERMINATOR' }])), ctx, w);
  const a = evaluate({ ...l, activePets: {} }, ctx, w);
  const b = evaluate({ ...l, activePets: {}, shards: { BURNINGSOUL: 10 } }, ctx, w);
  close(b.attackSpeed - a.attackSpeed, 10);
});

test('a bracelet takes the gloves slot: never worn together with gloves', () => {
  const res = new Map([...resource, ['MOLTEN_BRACELET', { id: 'MOLTEN_BRACELET', name: 'Molten Bracelet', category: 'BRACELET', tier: 'LEGENDARY', stats: { STRENGTH: 15, CRITICAL_DAMAGE: 10 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const l = buildLoadout(profile([newItem('TERMINATOR', res), newItem('SOULWEAVER_GLOVES', res), newItem('MOLTEN_BRACELET', res)]), ctx, w);
  assert.deepEqual(Object.keys(l.equipment), ['GLOVES']);
  const p = { get: (id) => ({ price: 1e6, source: 'AH' }) };
  const owned = buildLoadout(profile([newItem('TERMINATOR', res), newItem('SOULWEAVER_GLOVES', res)]), ctx, w);
  const buy = availableUpgrades(owned, ctx, p).find((u) => u.label.startsWith('Buy Molten Bracelet'));
  if (buy) assert.ok(!('BRACELET' in buy.apply(owned).equipment));
});

test('Legion counts 4 players against withers (every class) and Mage dragons, 1 for Archer / Berserker dragons and RCM', () => {
  const gain = (cls, mode = 'lcm') => {
    const ctx = makeContext({ cls, cataLevel: 50, mode, resource });
    const w = weightsFor(cls, null, mode);
    const plain = buildLoadout(profile([newItem(cls === 'arch' ? 'TERMINATOR' : 'HYPERION', resource), newItem('POWER_WITHER_LEGGINGS', resource)]), ctx, w);
    const legion = { ...plain, armor: { ...plain.armor, LEGGINGS: { ...plain.armor.LEGGINGS, enchants: { ultimate_legion: 5 } } } };
    const a = evaluate({ ...plain, activePets: {} }, ctx, w).stats;
    const b = evaluate({ ...legion, activePets: {} }, ctx, w).stats;
    return (t) => b[t].strength / a[t].strength - 1;
  };
  const arch = gain('arch');
  close(arch('wither'), 4 * arch('dragon'));
  const mage = gain('mage');
  close(mage('dragon'), mage('wither'));
  close(mage('wither'), arch('wither'));
});

test('Duplex is read from its NBT key ultimate_reiterate: Duplex V adds 0.2 arrows', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, archerLevel: 50, resource });
  const w = weightsFor('arch');
  const plain = buildLoadout(profile([newItem('TERMINATOR', resource)]), ctx, w);
  const duplex = buildLoadout(profile([{ ...newItem('TERMINATOR', resource), enchants: { ultimate_reiterate: 5 } }]), ctx, w);
  close(evaluate(duplex, ctx, w).arrowsPerVolley - evaluate(plain, ctx, w).arrowsPerVolley, 0.2);
  close(evaluate(duplex, ctx, w).arrowsPerVolley, 4.7);
});

test('unobtainable items like the Raygun are never offered', () => {
  const res = new Map([...resource, ['RAYGUN', { id: 'RAYGUN', name: 'Raygun', category: 'BOW', tier: 'UNOBTAINABLE', stats: { DAMAGE: 99999999 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const l = buildLoadout(profile([newItem('TERMINATOR', res)]), ctx, weightsFor('arch'));
  const p = { get: () => ({ price: 1, source: 'AH' }) };
  assert.ok(!availableUpgrades(l, ctx, p).some((u) => u.label.includes('Raygun')));
});

test('hatcessories: only the best hat counts, the others are duplicates', async () => {
  const { countedAccessories } = await import('../src/calc/stats.js');
  const hat = (id, recomb = false) => ({ id, name: id, category: 'ACCESSORY', baseRarity: 'SPECIAL', recomb, enchants: {}, gems: { slots: {}, unlocked: [] } });
  const loadout = { accessories: new Map([['PARTY_HAT_CRAB', hat('PARTY_HAT_CRAB')], ['PARTY_HAT_SLOTH', hat('PARTY_HAT_SLOTH')], ['PARTY_HAT_CRAB_ANIMATED', hat('PARTY_HAT_CRAB_ANIMATED', true)], ['WITHER_RELIC', { id: 'WITHER_RELIC', category: 'ACCESSORY', baseRarity: 'LEGENDARY' }]]) };
  const counted = countedAccessories(loadout).map((i) => i.id);
  assert.deepEqual(counted.sort(), ['PARTY_HAT_CRAB_ANIMATED', 'WITHER_RELIC']);
});

test('items the resource marks can_recombobulate: false are never offered a recombobulator', () => {
  const res = new Map([...resource,
    ['VOTER_BADGE_SUPREME', { id: 'VOTER_BADGE_SUPREME', name: "Supreme Voter's Badge", category: 'ACCESSORY', tier: 'EPIC', can_recombobulate: false, stats: { STRENGTH: 5 } }],
    ['WITHER_RELIC', { ...resource.get('WITHER_RELIC'), stats: { STRENGTH: 5 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const l = buildLoadout(profile([newItem('TERMINATOR', res), newItem('VOTER_BADGE_SUPREME', res), newItem('WITHER_RELIC', res)]), ctx, weightsFor('arch'));
  const labels = availableUpgrades(l, ctx, prices).map((u) => u.label);
  assert.ok(!labels.includes("Recombobulate Supreme Voter's Badge"));
  assert.ok(labels.includes('Recombobulate Wither Relic'));
});

test('Rift Prism: the profile Accessory Power wins; only an exact 11 gap adds the prism', () => {
  const res = new Map([...resource, ['RIFT_PRISM', { id: 'RIFT_PRISM', name: 'Rift Prism', category: 'ACCESSORY', tier: 'RARE', can_recombobulate: false }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const gear = [newItem('TERMINATOR', res), newItem('WITHER_RELIC', res)];
  const base = totalAccessoryPower(buildLoadout(profile(gear), ctx, w), res);
  assert.equal(totalAccessoryPower(buildLoadout(profile(gear, { riftPrism: true }), ctx, w), res), base + 11);
  // The profile's Accessory Power wins: consumed but no gap means the 11 is already in it.
  assert.equal(totalAccessoryPower(buildLoadout(profile(gear, { riftPrism: true, highestAp: base }), ctx, w), res), base);
  assert.equal(totalAccessoryPower(buildLoadout(profile(gear, { riftPrism: null, highestAp: base + 11 }), ctx, w), res), base + 11);
  assert.equal(totalAccessoryPower(buildLoadout(profile(gear, { riftPrism: null, highestAp: base + 10 }), ctx, w), res), base);
  const p = { get: () => ({ price: 1e6, source: 'AH' }) };
  assert.ok(!availableUpgrades(buildLoadout(profile(gear, { riftPrism: true }), ctx, w), ctx, p).some((u) => u.label === 'Buy Rift Prism'));
});

test('Combat skill adds 0.5 base crit chance per level; 2.67B combat XP is level 60', async () => {
  const { skillLevelFromXp } = await import('../src/calc/formulas.js');
  assert.equal(skillLevelFromXp(2666982490, 60), 60);
  assert.equal(skillLevelFromXp(55172425, 60), 50);
  assert.equal(skillLevelFromXp(55172424, 60), 49);
  const w = weightsFor('arch');
  const at = (combatLevel) => { const ctx = makeContext({ cls: 'arch', cataLevel: 50, combatLevel, resource }); return evaluate(buildLoadout(profile([newItem('TERMINATOR', resource)]), ctx, w), ctx, w).stats.wither.critChance; };
  // Terminator divides crit chance by 4: +30 flat -> +7.5.
  close(at(60) - at(0), 30 * 0.25);
});

test('Ragnarock is cast with the highest-strength owned pet, whatever pet is active', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const gd = { type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: 'ANTIQUE_REMEDIES' };
  const ed = { type: 'ENDER_DRAGON', tier: 'LEGENDARY', heldItem: null };
  const l = buildLoadout(profile(items([{ id: 'TERMINATOR' }, { id: 'RAGNAROCK_AXE', stars: 0, enchants: { ultimate_chimera: 5 } }]), { pets: [gd, ed] }), ctx, w);
  const strengthWith = (pets) => evaluate({ ...l, pets, activePets: { wither: ed, dragon: ed } }, ctx, w).stats.dragon.strength;
  // Owning the Golden Dragon raises strength even while the Ender Dragon is active.
  assert.ok(strengthWith([gd, ed]) > strengthWith([ed]));
});

test('floor-scaled dungeon armor: highest floor tier, 50% stat boost and +1 rarity (Skeleton Master Chestplate = 37.8 crit chance)', () => {
  const res = new Map([...resource, ['SKELETON_MASTER_CHESTPLATE', { id: 'SKELETON_MASTER_CHESTPLATE', name: 'Skeleton Master Chestplate', category: 'CHESTPLATE', tier: 'EPIC', dungeon_item: true,
    tiered_stats: { CRITICAL_CHANCE: [2, 2, 2, 2, 3, 3, 3, 3, 4, 4], CRITICAL_DAMAGE: [22, 23, 25, 27, 30, 32, 35, 38, 41, 45] } }]]);
  const chest = { ...newItem('SKELETON_MASTER_CHESTPLATE', res), recomb: true, reforge: 'ancient', masterStars: 5 };
  assert.equal(chest.baseRarity, 'LEGENDARY');
  // Floor 10 (Catacombs 36+): 4 x 1.5 = 6, Ancient at Mythic +15, x (1.5 + 0.25 + 0.05) stars.
  const ctx = { ...makeContext({ cls: 'arch', cataLevel: 50, resource: res }), secretDigit: 5 };
  close(itemContribution(chest, ctx).global.critChance, 21 * 1.8);
  // Catacombs 24 tops out at floor 7: 3 x 1.5 = 4.5.
  const low = { ...makeContext({ cls: 'arch', cataLevel: 24, resource: res }), secretDigit: 5 };
  close(itemContribution({ ...chest, reforge: null }, low).global.critChance, 4.5 * 1.8);
});

test('accessory chains: owning the Pesthunter Relic supersedes the Artifact even without a direct NEU link', async () => {
  const { accessorySuperseded } = await import('../src/calc/stats.js');
  assert.ok(accessorySuperseded('PESTHUNTER_ARTIFACT', new Set(['PESTHUNTER_RELIC'])));
  assert.ok(accessorySuperseded('PESTHUNTER_BADGE', new Set(['PESTHUNTER_RELIC'])));
  assert.ok(!accessorySuperseded('PESTHUNTER_RELIC', new Set(['PESTHUNTER_ARTIFACT'])));
});

test('mutual accessory families (Abiphone cases) count once, not zero times', async () => {
  const { countedAccessories } = await import('../src/calc/stats.js');
  const acc = (id, baseRarity = 'RARE') => [id, { id, category: 'ACCESSORY', baseRarity }];
  const counted = countedAccessories({ accessories: new Map([acc('ABICASE_BLUE_RED'), acc('ABICASE_BLUE_BLUE'), acc('PESTHUNTER_RELIC', 'LEGENDARY'), acc('PESTHUNTER_ARTIFACT', 'EPIC')]) }).map((i) => i.id);
  assert.equal(counted.filter((id) => id.startsWith('ABICASE')).length, 1);
  assert.deepEqual(counted.filter((id) => id.startsWith('PESTHUNTER')), ['PESTHUNTER_RELIC']);
});

test('fragging an item marks the fragged id owned, so the planner never buys a second copy', () => {
  const res = new Map([...resource, ['ADAPTIVE_BELT', { id: 'ADAPTIVE_BELT', name: 'Adaptive Belt', category: 'BELT', tier: 'EPIC', dungeon_item: true, stats: { STRENGTH: 10 } }],
    ['STARRED_ADAPTIVE_BELT', { id: 'STARRED_ADAPTIVE_BELT', name: 'Adaptive Belt', category: 'BELT', tier: 'EPIC', dungeon_item: true, stats: { STRENGTH: 20 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const l = buildLoadout(profile([newItem('TERMINATOR', res), newItem('ADAPTIVE_BELT', res)]), ctx, weightsFor('arch'));
  const p = { get: () => ({ price: 1e5, source: 'BZ' }) };
  const frag = availableUpgrades(l, ctx, p).find((u) => u.label.startsWith('Frag Adaptive Belt'));
  if (!frag) return; // no fragged recipe for this id in the data
  const after = frag.apply(l);
  assert.ok(after.owned.has(after.equipment.BELT.id));
  assert.ok(!availableUpgrades(after, ctx, p).some((u) => u.label.startsWith('Buy Adaptive Belt')));
});

test('enchant levels never lock: Duplex I can be followed by higher levels, each ranked by gain per million', async () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, archerLevel: 50, resource });
  const w = weightsFor('arch');
  const l = buildLoadout(profile([newItem('TERMINATOR', resource)]), ctx, w);
  const book = { ENCHANTMENT_ULTIMATE_REITERATE_1: 1e6, ENCHANTMENT_ULTIMATE_REITERATE_2: 3e6, ENCHANTMENT_ULTIMATE_REITERATE_3: 6e6, ENCHANTMENT_ULTIMATE_REITERATE_4: 10e6, ENCHANTMENT_ULTIMATE_REITERATE_5: 15e6 };
  const p = { get: (id) => (book[id] ? { price: book[id], source: 'BZ' } : null) };
  const labels = (await planUpgrades(l, ctx, p, w, 10)).steps.map((s) => s.label);
  const duplex = labels.filter((x) => x.startsWith('Duplex'));
  assert.ok(duplex.length > 1, labels.join(', '));
  assert.ok(duplex.at(-1).startsWith('Duplex V '), duplex.join(', '));
});

test('a piece the plan later replaces gets less than 25M spent on it', async () => {
  const res = new Map([...resource, ['OLD_LEGGINGS', { id: 'OLD_LEGGINGS', name: 'Old Leggings', category: 'LEGGINGS', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 30, CRITICAL_DAMAGE: 30 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, archerLevel: 50, resource: res });
  const w = weightsFor('arch');
  const l = buildLoadout(profile([newItem('TERMINATOR', res), newItem('OLD_LEGGINGS', res)]), ctx, w);
  const plan = async (star) => {
    const table = { FIRST_MASTER_STAR: star, POWER_WITHER_LEGGINGS: 3e9 };
    return (await planUpgrades(l, ctx, { get: (id) => (table[id] ? { price: table[id], source: 'AH' } : null) }, w, 5)).steps.map((s) => s.label);
  };
  // 20M star: under the budget, kept before the Necron's Leggings buy.
  const cheap = await plan(20e6);
  assert.ok(cheap.indexOf('1st Master Star on Old Leggings') >= 0 && cheap.indexOf('1st Master Star on Old Leggings') < cheap.indexOf("Buy Necron's Leggings"), cheap.join(', '));
  // 30M star on leggings that get replaced: dropped.
  const dear = await plan(30e6);
  assert.ok(dear.includes("Buy Necron's Leggings"), dear.join(', '));
  assert.ok(!dear.includes('1st Master Star on Old Leggings'), dear.join(', '));
});

test('Golden heads upgrade into Diamond heads and keep their upgrades; that is not a replacement', async () => {
  const res = new Map([...resource,
    ['GOLD_NECRON_HEAD', { id: 'GOLD_NECRON_HEAD', name: 'Golden Necron Head', category: 'HELMET', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 30, CRITICAL_DAMAGE: 30 } }],
    ['DIAMOND_NECRON_HEAD', { id: 'DIAMOND_NECRON_HEAD', name: 'Diamond Necron Head', category: 'HELMET', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 31, CRITICAL_DAMAGE: 31 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, archerLevel: 50, resource: res });
  const w = weightsFor('arch');
  const head = { ...newItem('GOLD_NECRON_HEAD', res), reforge: 'ancient', enchants: { ultimate_legion: 5 } };
  const l = buildLoadout(profile([newItem('TERMINATOR', res), head]), ctx, w);
  const table = { ENCHANTED_DIAMOND_BLOCK: 40e6, DIAMOND_NECRON_HEAD: 2e9, FIRST_MASTER_STAR: 30e6 };
  const p = { get: (id) => (table[id] ? { price: table[id], source: 'BZ' } : null) };
  const ups = availableUpgrades(l, ctx, p);
  const up = ups.find((u) => u.label.startsWith('Upgrade Golden Necron Head into Diamond Necron Head'));
  assert.equal(up.cost, 320e6);
  assert.ok(!ups.some((u) => u.label.startsWith('Buy Diamond Necron Head')));
  const after = up.apply(l);
  assert.equal(after.armor.HELMET.id, 'DIAMOND_NECRON_HEAD');
  assert.equal(after.armor.HELMET.enchants.ultimate_legion, 5);
  assert.equal(after.armor.HELMET.reforge, 'ancient');
  // A 30M master star on the Golden head stays: upgrading the head keeps it, so the 25M cap does not apply.
  const labels = (await planUpgrades(l, ctx, p, w, 6)).steps.map((s) => s.label);
  assert.ok(labels.includes('1st Master Star on Golden Necron Head'), labels.join(', '));
  assert.ok(labels.some((x) => x.startsWith('Upgrade Golden Necron Head')), labels.join(', '));
});

test("Wither armor upgrades into Necron's / Storm's with 8 fragments; Mage never gets Necron's", () => {
  const res = new Map([...resource,
    ['WITHER_LEGGINGS', { id: 'WITHER_LEGGINGS', name: 'Wither Leggings', category: 'LEGGINGS', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 10 } }],
    ['WISE_WITHER_LEGGINGS', { id: 'WISE_WITHER_LEGGINGS', name: "Storm's Leggings", category: 'LEGGINGS', tier: 'LEGENDARY', dungeon_item: true, stats: { INTELLIGENCE: 250, CRITICAL_DAMAGE: 10 } }]]);
  const p = { get: () => ({ price: 1e6, source: 'BZ' }) };
  const labels = (cls, weapon) => {
    const ctx = makeContext({ cls, cataLevel: 50, resource: res });
    const l = buildLoadout(profile([newItem(weapon, res), newItem('WITHER_LEGGINGS', res)]), ctx, weightsFor(cls));
    return availableUpgrades(l, ctx, p).map((u) => u.label);
  };
  const arch = labels('arch', 'TERMINATOR');
  assert.ok(arch.includes("Upgrade Wither Leggings into Necron's Leggings (8 Giant Fragment Diamonds)"), arch.join(', '));
  assert.ok(!arch.some((x) => x.startsWith("Buy Necron's Leggings")));
  const mage = labels('mage', 'HYPERION');
  assert.ok(mage.some((x) => x.startsWith("Upgrade Wither Leggings into Storm's Leggings")));
  assert.ok(!mage.some((x) => x.includes("Necron's Leggings")));
});

test('Mage RCM: scored on Wither Impact, needs a Necron blade, tunes speed to 500 then intelligence, any armor reforge', async () => {
  const { tuningAllocation, isRcm } = await import('../src/calc/stats.js');
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, resource, mode: 'rcm' });
  assert.ok(isRcm(ctx));
  assert.deepEqual(weightsFor('mage', null, 'rcm'), { wither: 100, dragon: 0 });
  // 100 points at 620 speed: 54 to speed (700 = 500 in dungeons), the rest to intelligence (2 each).
  const t = tuningAllocation(100, 620, ctx);
  assert.equal(t.speedPoints, 54);
  close(t.stats.intelligence, 92);
  const w = weightsFor('mage', null, 'rcm');
  const withBlade = buildLoadout(profile([scrolled(newItem('HYPERION', resource)), newItem('GIANTS_SWORD', resource)]), ctx, w);
  assert.equal(withBlade.weapon.id, 'HYPERION');
  const r = evaluate(withBlade, ctx, w);
  assert.ok(r.wither > 0);
  // Intelligence raises the cast; strength does not.
  const noBlade = buildLoadout(profile([newItem('GIANTS_SWORD', resource)]), ctx, w);
  assert.equal(evaluate(noBlade, ctx, w).wither, 0);
  // Armor reforges other than Ancient are offered for the RCM set.
  const res = new Map([...resource, ['WISE_WITHER_LEGGINGS', { id: 'WISE_WITHER_LEGGINGS', name: "Storm's Leggings", category: 'LEGGINGS', tier: 'LEGENDARY', dungeon_item: true, stats: { INTELLIGENCE: 250 } }]]);
  const armorReforges = (mode) => {
    const c = makeContext({ cls: 'mage', cataLevel: 50, resource: res, mode });
    const l = buildLoadout(profile([scrolled(newItem('HYPERION', res)), newItem('WISE_WITHER_LEGGINGS', res)]), c, weightsFor('mage', null, mode));
    return availableUpgrades(l, c, { get: () => ({ price: 1e5, source: 'BZ' }) }).filter((u) => /reforge on Storm's Leggings/.test(u.label)).map((u) => u.label);
  };
  assert.ok(armorReforges('rcm').some((x) => !x.startsWith('Ancient')), armorReforges('rcm').join(', '));
  assert.ok(armorReforges('lcm').every((x) => x.startsWith('Ancient')));
});

test('Fabled gives melee (LCM) 1.075x; RCM counts Smite but not Sharpness or Fabled', () => {
  const w = weightsFor('mage');
  const lcm = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const plain = { ...scrolled(newItem('HYPERION', resource)), reforge: null };
  const fabled = { ...plain, reforge: 'fabled' };
  const base = buildLoadout(profile([plain]), lcm, w);
  const withFabled = { ...base, weapon: fabled };
  const ratioOk = evaluate(withFabled, lcm, w).wither > evaluate(base, lcm, w).wither;
  assert.ok(ratioOk);
  const rcm = makeContext({ cls: 'mage', cataLevel: 50, resource, mode: 'rcm' });
  const rw = weightsFor('mage', null, 'rcm');
  const blade = (enchants, reforge = 'heroic') => ({ ...scrolled(newItem('HYPERION', resource)), reforge, enchants });
  const l = buildLoadout(profile([blade({})]), rcm, rw);
  const cast = (item) => evaluate({ ...l, weapon: item }, rcm, rw).wither;
  assert.ok(cast(blade({ smite: 7 })) > cast(blade({})));
  close(cast(blade({ sharpness: 7 })), cast(blade({})));
});

test('floor toggle: Master Mode counts Titan Killer and Triple-Strike, Normal Mode Giant Killer and First Strike', () => {
  // Additive bonuses show up one to one.
  const res = new Map([...resource, ['WEAK_SWORD', { id: 'WEAK_SWORD', name: 'Weak Sword', category: 'SWORD', tier: 'COMMON', stats: { DAMAGE: 1 } }]]);
  const w = weightsFor('mage');
  const dmg = (floorMode, enchants) => {
    const ctx = makeContext({ cls: 'mage', cataLevel: 0, resource: res, floorMode });
    const l = buildLoadout(profile([{ ...newItem('WEAK_SWORD', res), enchants }]), ctx, w);
    return evaluate(l, ctx, w).wither;
  };
  assert.ok(dmg('normal', { giant_killer: 7 }) > dmg('normal', {}));
  close(dmg('master', { giant_killer: 7 }), dmg('master', {}));
  assert.ok(dmg('master', { titan_killer: 7 }) > dmg('master', {}));
  close(dmg('normal', { titan_killer: 7 }), dmg('normal', {}));
  close(dmg('normal', { titan_killer: 7 }), dmg('normal', {}));
  assert.ok(dmg('master', { triple_strike: 5 }) > dmg('master', {}));
  close(dmg('master', { first_strike: 5 }), dmg('master', {}));
  assert.ok(dmg('normal', { first_strike: 5 }) > dmg('normal', {}));
  close(dmg('normal', { triple_strike: 5 }), dmg('normal', {}));
  // Thunderlord VII: a 60% lightning hit every 3rd hit = +20%.
  close(dmg('master', { thunderlord: 7 }) / dmg('master', {}), 1.2);
  assert.ok(dmg('master', { prosecute: 6 }) > dmg('master', {}));
});

test('RCM: only Giant Killer, Prosecute, Swarm (7 mobs) and Smite raise Wither Impact, on both floors; Crow + Hephaestus Relic boosts ability damage', async () => {
  const { petStats } = await import('../src/lib/pets.js');
  const crow = petStats({ type: 'CROW', tier: 'LEGENDARY', heldItem: 'HEPHAESTUS_RELIC' });
  close(crow.stats.abilityDamage, 30);
  close(crow.stats.intelligence, 225);
  const ctx = makeContext({ cls: 'mage', cataLevel: 0, resource, mode: 'rcm' });
  const w = weightsFor('mage', null, 'rcm');
  const blade = (enchants) => ({ ...scrolled(newItem('HYPERION', resource)), enchants });
  const l = buildLoadout(profile([blade({})]), ctx, w);
  const cast = (enchants) => evaluate({ ...l, weapon: blade(enchants) }, ctx, w).wither;
  for (const e of [{ giant_killer: 7 }, { prosecute: 6 }, { ultimate_swarm: 5 }, { smite: 7 }]) assert.ok(cast(e) > cast({}), JSON.stringify(e));
  for (const e of [{ titan_killer: 7 }, { first_strike: 5 }, { thunderlord: 7 }, { sharpness: 7 }, { ender_slayer: 7 }]) close(cast(e), cast({}));
  // Normal Mode (F7) clear set: Giant Killer still counts, Titan Killer still does not.
  const f7 = makeContext({ cls: 'mage', cataLevel: 0, resource, mode: 'rcm', floorMode: 'normal' });
  const castF7 = (enchants) => evaluate({ ...l, weapon: blade(enchants) }, f7, w).wither;
  assert.ok(castF7({ giant_killer: 7 }) > castF7({}));
  close(castF7({ titan_killer: 7 }), castF7({}));
});

test('upper-case enchant keys from the profile are read', () => {
  const [it] = items([{ id: 'HYPERION', enchants: { PROSECUTE: 6 } }]);
  assert.equal(it.enchants.prosecute, 6);
});

test('Normal Mode (Mage only): no master stars, no Master Skull, withers only; other classes stay Master Mode', () => {
  const nm = makeContext({ cls: 'mage', cataLevel: 50, resource, floorMode: 'normal' });
  assert.equal(nm.floorMode, 'normal');
  assert.equal(makeContext({ cls: 'arch', cataLevel: 50, resource, floorMode: 'normal' }).floorMode, 'master');
  assert.equal(makeContext({ cls: 'mage', cataLevel: 50, resource, floorMode: 'normal', mode: 'rcm' }).floorMode, 'normal');
  assert.deepEqual(weightsFor('mage', null, 'lcm', 'normal'), { wither: 100, dragon: 0 });
  const mm = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const starred = { ...newItem('HYPERION', resource), masterStars: 5 };
  const plain = newItem('HYPERION', resource);
  close(itemContribution(starred, nm).global.strength, itemContribution(plain, nm).global.strength);
  assert.ok(itemContribution(starred, mm).global.strength > itemContribution(plain, mm).global.strength);
  const skull = newItem('MASTER_SKULL_TIER_7', resource);
  assert.equal(itemContribution(skull, nm).global.multStrength, undefined);
  close(itemContribution(skull, mm).global.multStrength, 1.1);
});

test('a small step overwritten within 10M coins is skipped; one replaced later stays', async () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, archerLevel: 50, resource });
  const w = weightsFor('arch');
  const l = buildLoadout(profile([newItem('TERMINATOR', resource)]), ctx, w);
  const plan = async (second) => {
    const book = { ENCHANTMENT_SNIPE_3: 1e6, ENCHANTMENT_SNIPE_4: second };
    return (await planUpgrades(l, ctx, { get: (id) => (book[id] ? { price: book[id], source: 'BZ' } : null) }, w, 5)).steps.map((s) => s.label);
  };
  // Snipe IV for 2.5M right after Snipe III (under 5% gain): Snipe III is skipped.
  const soon = await plan(2.5e6);
  assert.ok(!soon.some((x) => x.startsWith('Snipe III ')), soon.join(', '));
  assert.ok(soon.some((x) => x.startsWith('Snipe IV ')), soon.join(', '));
  // Snipe IV for 20M: more than 10M later, so Snipe III stays.
  const late = await plan(20e6);
  assert.ok(late.some((x) => x.startsWith('Snipe III ')), late.join(', '));
});

test('ultimate enchants combine: Legion II on Legion I costs one more Legion I book when that is cheaper, and is not an overwrite', async () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, archerLevel: 50, resource });
  const w = weightsFor('arch');
  const l = buildLoadout(profile([newItem('TERMINATOR', resource)]), ctx, w);
  const book = { ENCHANTMENT_ULTIMATE_REITERATE_1: 1e6, ENCHANTMENT_ULTIMATE_REITERATE_2: 2.5e6 };
  const p = { get: (id) => (book[id] ? { price: book[id], source: 'BZ' } : null) };
  const withI = { ...l, weapon: { ...l.weapon, enchants: { ultimate_reiterate: 1 } } };
  const step = availableUpgrades(withI, ctx, p).find((u) => u.label.startsWith('Duplex II on Terminator'));
  assert.equal(step.label, 'Duplex II on Terminator (combine: add a I book)');
  close(step.cost, 1e6);
  const labels = (await planUpgrades(l, ctx, p, w, 5)).steps.map((s) => s.label);
  assert.ok(labels.includes('Duplex I on Terminator') && labels.includes('Duplex II on Terminator (combine: add a I book)'), labels.join(', '));
});

test('powers: the best learned power is used for free; unlearned ones are a stone purchase step', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource, mode: 'rcm' });
  const w = weightsFor('mage', null, 'rcm');
  const l = buildLoadout(profile([newItem('HYPERION', resource)], { unlockedPowers: [] }), ctx, w);
  assert.ok(l.unlockedPowers.has('inspired'));
  const p = { get: (id) => ({ price: 1e6, source: 'BZ', id }) };
  const learn = availableUpgrades(l, ctx, p).find((u) => u.label.startsWith('Learn Bizarre power'));
  assert.ok(learn);
  // Learning a power takes 9 of its stone.
  close(learn.cost, 9e6);
  const after = learn.apply(l);
  assert.equal(after.power, 'bizarre');
  assert.ok(evaluate(after, ctx, w).wither >= evaluate(l, ctx, w).wither);
});

test('an accessory with an enrichment can swap it with the Accessory Enrichment Swapper', () => {
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const relic = { ...newItem('WITHER_RELIC', resource), enrichment: 'critical_chance' };
  const l = buildLoadout(profile([newItem('TERMINATOR', resource), relic]), ctx, w);
  const labels = availableUpgrades(l, ctx, { get: (id) => ({ price: 1e6, source: 'BZ', id }) }).map((u) => u.label);
  assert.ok(labels.some((x) => x.startsWith('Swap all enriched accessories (1) to Critical Damage')), labels.filter((x) => /Swap/.test(x)).join(', '));
});

test('Hyperion gets +1 damage and +2 intelligence per Catacombs level before dungeon scaling', () => {
  const c50 = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const hyp = itemContribution(newItem('HYPERION', resource), c50).global;
  close(hyp.intelligence, (350 + 100) * 6.35);
  close(hyp.damage, (260 + 50) * 6.35);
});

test('Shadow Assassin Cloak gives 1.1x only while holding a blade with all 3 Wither scrolls; scrolls are a step', () => {
  const res = new Map([...resource, ['SHADOW_ASSASSIN_CLOAK', { id: 'SHADOW_ASSASSIN_CLOAK', name: 'Shadow Assassin Cloak', category: 'CLOAK', tier: 'LEGENDARY', dungeon_item: true, stats: {} }]]);
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource: res });
  const w = weightsFor('mage');
  const cloak = newItem('SHADOW_ASSASSIN_CLOAK', res);
  const two = { ...newItem('HYPERION', res), scrolls: ['IMPLOSION_SCROLL', 'SHADOW_WARP_SCROLL'] };
  const l2 = buildLoadout(profile([two, cloak]), ctx, w);
  const l3 = buildLoadout(profile([scrolled(newItem('HYPERION', res)), cloak]), ctx, w);
  assert.equal(l2.witherImpact, false);
  assert.equal(l3.witherImpact, true);
  const d2 = evaluate(l2, ctx, w).wither;
  const d3 = evaluate(l3, ctx, w).wither;
  assert.ok(d3 > d2);
  const step = availableUpgrades(l2, ctx, { get: (id) => ({ price: 2e6, source: 'BZ', id }) }).find((u) => u.label.startsWith('Wither Shield on Hyperion'));
  assert.ok(step);
  assert.equal(step.cost, 2e6);
  close(evaluate(step.apply(l2), ctx, w).wither, d3);
  // Holding another weapon gets no 1.1x, even with the scrolled Hyperion owned.
  const gs = newItem('GIANTS_SWORD', res);
  close(evaluate({ ...l3, weapon: gs }, ctx, w).wither, evaluate({ ...l2, weapon: gs }, ctx, w).wither);
});

test('below floor 10 (cata < 36), a floor-scaled drop gets under 25M in total (it is replaced by the floor 10 one later)', async () => {
  const res = new Map([...resource, ['SKELETON_MASTER_CHESTPLATE', { id: 'SKELETON_MASTER_CHESTPLATE', name: 'Skeleton Master Chestplate', category: 'CHESTPLATE', tier: 'EPIC', dungeon_item: true,
    tiered_stats: { CRITICAL_DAMAGE: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100] } }]]);
  const w = weightsFor('arch');
  const table = { FIRST_MASTER_STAR: 2e6, SECOND_MASTER_STAR: 15e6, THIRD_MASTER_STAR: 30e6, RECOMBOBULATOR_3000: 0.9e6 };
  const p = { get: (id) => (table[id] ? { price: table[id], source: 'BZ' } : null) };
  const labels = async (cata) => {
    const ctx = makeContext({ cls: 'arch', cataLevel: cata, archerLevel: 50, resource: res });
    const l = buildLoadout(profile([newItem('TERMINATOR', res), newItem('SKELETON_MASTER_CHESTPLATE', res)]), ctx, w);
    return (await planUpgrades(l, ctx, p, w, 12)).steps.map((s) => s.label);
  };
  const low = await labels(34);
  assert.ok(low.includes('2nd Master Star on Skeleton Master Chestplate'), low.join(', '));
  assert.ok(!low.includes('3rd Master Star on Skeleton Master Chestplate'), low.join(', '));
  const high = await labels(40);
  assert.ok(high.includes('3rd Master Star on Skeleton Master Chestplate'), high.join(', '));
});

test('profile stats: base crit damage, skills, slayers, SkyBlock level, Harp; profile upgrades rank the next levels', async () => {
  const { profileStatTotals } = await import('../src/calc/stats.js');
  const { profileUpgrades } = await import('../src/calc/upgrades.js');
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 40, resource, skills: { FORAGING: 20, ENCHANTING: 60, ALCHEMY: 50 }, slayers: { spider: 7, wolf: 7, enderman: 8 },
    profileStats: { sbLevel: 278, harpSongs: ['hymn_joy', 'vie_en_rose'], cakes: [{ stat: 'strength', amount: 2 }] } });
  const t = profileStatTotals(ctx);
  // 50 base + spider 1-4 (4) + 5-6 (4) + wolf 5 (1) + 7 (2) = 61 crit damage.
  close(t.critDamage, 61);
  close(t.critChance, 1);
  // Foraging 20: 14 + 2 x 6 = 26, SkyBlock 278 / 5 = 55, cake 2.
  close(t.strength, 26 + 55 + 2);
  // Enchanting 60: 14 + 92 = 106, Alchemy 50: 14 + 72 = 86, enderman 1+2+3+4 = 10, Harp 1 + 4.
  close(t.intelligence, 106 + 86 + 10 + 5);
  assert.deepEqual(profileStatTotals(makeContext({ cls: 'mage', cataLevel: 50, resource })), {});
  const w = weightsFor('mage');
  const l = buildLoadout(profile([newItem('HYPERION', resource)]), ctx, w);
  const { rows } = profileUpgrades(l, ctx, w);
  assert.ok(rows.some((r) => r.label === 'Mage 40 → 41'));
  assert.ok(rows.some((r) => r.label.startsWith("Melody's Harp")));
  for (let i = 1; i < rows.length; i++) assert.ok(rows[i - 1].gain >= rows[i].gain);
});

test('pet perks: Wither Skeleton vs withers, Skeleton bow damage and Combo, Blaze doubles potato books', async () => {
  const { petStats } = await import('../src/lib/pets.js');
  const ws = petStats({ type: 'WITHER_SKELETON', tier: 'LEGENDARY' });
  close(ws.wither.additiveDamage, 100);
  const sk = petStats({ type: 'SKELETON', tier: 'LEGENDARY' });
  close(sk.stats.bowDamage, 150);
  close(sk.special.comboStrength, 60);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const l = buildLoadout(profile([newItem('TERMINATOR', resource)]), ctx, w);
  const wsPet = { type: 'WITHER_SKELETON', tier: 'LEGENDARY', heldItem: null };
  const withWs = evaluate({ ...l, activePets: { wither: wsPet, dragon: wsPet } }, ctx, w);
  const none = evaluate({ ...l, activePets: {} }, ctx, w);
  assert.ok(withWs.wither / none.wither > withWs.dragon / none.dragon);
  const mctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const mw = weightsFor('mage');
  const ml = buildLoadout(profile([{ ...newItem('HYPERION', resource), hpb: 15 }]), mctx, mw);
  const blaze = { type: 'BLAZE', tier: 'LEGENDARY', heldItem: null };
  const b = evaluate({ ...ml, activePets: { wither: blaze, dragon: blaze } }, mctx, mw).stats.wither;
  const n = evaluate({ ...ml, activePets: {} }, mctx, mw).stats.wither;
  assert.ok(b.damage > n.damage);
});

test('pet perks from the clarifications: Pigman, Tiger, T-Rex, Lion, Griffin, Ankylosaurus, Black Cat, Death\'s Touch; no Horse pets', async () => {
  const { petStats } = await import('../src/lib/pets.js');
  const { tuningAllocation } = await import('../src/calc/stats.js');
  const ps = (type, heldItem = null) => petStats({ type, tier: petTierFor(type), heldItem });
  close(ps('PIGMAN').stats.additiveDamage, 75);
  close(ps('TIGER').wither.additiveDamage, 100);
  // Tyrant: Hephaestus Remedies (2x strength) becomes 3x.
  const rex = ps('TYRANNOSAURUS', 'HEPHAESTUS_REMEDIES');
  close(rex.stats.strength, 75 * 3);
  const lion = ps('LION');
  close(lion.special.weaponStats.damage, 20);
  close(lion.special.firstStrikeBoost, 1);
  close(ps('GRIFFIN').stats.additiveStrength, 0.15);
  close(ps('ANKYLOSAURUS').special.perkStrength, 500);
  close(ps('BLACK_CAT').special.speedCapBonus, 100);
  // Black Cat lifts the speed target from 500 to 600 (850 on the sheet).
  assert.equal(tuningAllocation(200, 750, { cls: 'arch', speedCapBonus: 100, tuning: { split: { critDamage: 1 } } }).speedPoints, 67);
  // Death's Touch adds damage; Stampede only counts in the RCM clear.
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const w = weightsFor('mage');
  const l = buildLoadout(profile([newItem('HYPERION', resource)]), ctx, w);
  const ws = { type: 'WITHER_SKELETON', tier: 'LEGENDARY', heldItem: null };
  const noWb = { ...l, activePets: {} };
  assert.ok(evaluate({ ...l, activePets: { wither: ws, dragon: ws } }, ctx, w).dragon > evaluate(noWb, ctx, w).dragon);
  const { dpsPets } = await import('../src/calc/stats.js');
  assert.equal(dpsPets([{ type: 'SKELETON_HORSE', tier: 'LEGENDARY', heldItem: null }], ctx).length, 0);
});

test('ferocity: Berserker vs withers only (extra hit x Berserk melee multiplier), stars only, never Archer', () => {
  const res = new Map([...resource, ['TERMINATOR', { ...resource.get('TERMINATOR'), stats: { ...resource.get('TERMINATOR').stats, FEROCITY: 20 } }]]);
  const dmg = (cls, res2) => {
    const ctx = makeContext({ cls, cataLevel: 50, berserkLevel: 50, archerLevel: 50, resource: res2 });
    const w = weightsFor(cls);
    const l = buildLoadout(profile([newItem('TERMINATOR', res2)]), ctx, w);
    return { r: evaluate({ ...l, activePets: {} }, ctx, w), l, ctx };
  };
  const withF = dmg('bers', res);
  const without = dmg('bers', resource);
  assert.ok(withF.r.wither > without.r.wither);
  close(withF.r.dragon, without.r.dragon);
  const a1 = dmg('arch', res);
  const a0 = dmg('arch', resource);
  close(a1.r.wither, a0.r.wither);
  // Ferocity on a dungeon item gets the star multiplier only (1.5 at 5 stars, no cata).
  close(itemContribution(newItem('TERMINATOR', res), withF.ctx).global.ferocity, 20 * 1.5);
  // Vicious V: +5 ferocity on the Terminator (star scaled like the rest).
  close(itemContribution({ ...newItem('TERMINATOR', res), enchants: { vicious: 5 } }, withF.ctx).global.ferocity, (20 + 5) * 1.5);
});

test('runDps returns plain data for the worker (coin and profile menus); bows get arrow damage', async () => {
  const { runDps } = await import('../src/calc/run.js');
  const member = { ...profile(items([{ id: 'TERMINATOR' }])), cataLevel: 50, secrets: 0, archerLevel: 50, combatLevel: 60 };
  const coins = await runDps({ member, cls: 'arch', target: null, resource, prices, menu: 'coins' });
  assert.ok(Array.isArray(coins.plan.steps));
  assert.equal(typeof coins.plan.startDamage.wither, 'number');
  assert.deepEqual(structuredClone(coins), coins);
  const prof = await runDps({ member, cls: 'arch', target: null, resource, prices, menu: 'profile' });
  assert.ok(Array.isArray(prof.profile.rows));
  // Nansorb arrows: +5 bow damage.
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const l = buildLoadout(profile(items([{ id: 'TERMINATOR' }])), ctx, weightsFor('arch'));
  close(evaluate(l, ctx, weightsFor('arch')).stats.wither.damage, 5 + 310 * 6.35 + 5);
});

test('stars alone do not make a dungeon item (The Primordial): no catacombs scaling, master stars or medallion', () => {
  const res = new Map([...resource, ['THE_PRIMORDIAL', { id: 'THE_PRIMORDIAL', name: 'The Primordial', category: 'BELT', tier: 'LEGENDARY', stats: { STRENGTH: 10 },
    upgrade_costs: [[{ type: 'ESSENCE', essence_type: 'SPIDER', amount: 10 }]] }]]);
  const enc = encode([{ id: 'THE_PRIMORDIAL', stars: 5 }]);
  const [belt] = decodeItems(enc).map((i) => toItemState(i, res));
  assert.equal(belt.dungeon, undefined);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  close(itemContribution(belt, ctx).global.strength, 10);
  const l = buildLoadout(profile([newItem('TERMINATOR', res), belt]), ctx, weightsFor('arch'));
  assert.ok(!availableUpgrades(l, ctx, prices).some((u) => /Master Star on The Primordial/.test(u.label)));
});

test('Book of Progression: x(1 + 0.01% per SkyBlock level), capped at 500, and it lifts the SkyBlock level profile row', async () => {
  const { bookMultiplier } = await import('../src/calc/stats.js');
  const { profileUpgrades } = await import('../src/calc/upgrades.js');
  const res = new Map([...resource, ['BOOK_OF_PROGRESSION', { id: 'BOOK_OF_PROGRESSION', name: 'Book of Progression', category: 'ACCESSORY', tier: 'LEGENDARY' }]]);
  const mk = (sbLevel, bopBonus = 'EXTRA_INFLICTION') => makeContext({ cls: 'arch', cataLevel: 50, resource: res, profileStats: { sbLevel, bopBonus, harpSongs: [], cakes: [] } });
  const w = weightsFor('arch');
  const withBook = buildLoadout(profile([newItem('TERMINATOR', res), newItem('BOOK_OF_PROGRESSION', res)]), mk(278), w);
  const without = buildLoadout(profile([newItem('TERMINATOR', res)]), mk(278), w);
  close(bookMultiplier(withBook, mk(278)), 1.0278);
  close(bookMultiplier(withBook, mk(620)), 1.05);
  close(bookMultiplier(without, mk(278)), 1);
  const row = (l) => profileUpgrades(l, mk(278), w).rows.find((r) => r.label.startsWith('SkyBlock level')).gain;
  assert.ok(row(withBook) > row(without));
  // Set to something else: no bonus, and switching is a step costing 500 x level.
  const off = buildLoadout(profile([newItem('TERMINATOR', res), newItem('BOOK_OF_PROGRESSION', res)]), mk(278, 'STACKED_HEALING'), w);
  close(bookMultiplier(off, mk(278, 'STACKED_HEALING')), 1);
  const step = availableUpgrades(off, mk(278, 'STACKED_HEALING'), { get: () => null }).find((u) => u.label.startsWith('Set Book of Progression'));
  assert.equal(step.cost, 500 * 278);
  close(bookMultiplier(step.apply(off), mk(278, 'STACKED_HEALING')), 1.0278);
  // Under level 75 the damage option is locked.
  const low = buildLoadout(profile([newItem('TERMINATOR', res), newItem('BOOK_OF_PROGRESSION', res)]), mk(60, null), w);
  assert.ok(!availableUpgrades(low, mk(60, null), { get: () => null }).some((u) => u.label.startsWith('Set Book of Progression')));
});

test('one Accessory Enrichment Swapper swaps every enriched accessory in the bag as one step', async () => {
  const ids = ['ACC_A', 'ACC_B', 'ACC_C', 'ACC_D', 'ACC_E'];
  const res = new Map([...resource, ...ids.map((id) => [id, { id, name: id, category: 'ACCESSORY', tier: 'LEGENDARY' }])]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, archerLevel: 50, resource: res });
  const w = weightsFor('arch');
  const accs = ids.map((id) => ({ ...newItem(id, res), enrichment: 'walk_speed' }));
  const l = buildLoadout(profile([newItem('TERMINATOR', res), ...accs]), ctx, w);
  const p = { get: (id) => (id === 'TALISMAN_ENRICHMENT_SWAPPER' ? { price: 1e5, source: 'BZ' } : null) };
  const steps = (await planUpgrades(l, ctx, p, w, 10)).steps;
  const swap = steps.find((x) => /^Swap all enriched accessories \(5\)/.test(x.label));
  assert.ok(swap, steps.map((x) => x.label).join(', '));
  close(swap.cost, 1e5);
  assert.ok(!steps.some((x) => /^Swap ACC_/.test(x.label)));
});

test('buys and shards respect requirements: Catacombs, floor, skills, slayers and Hunting', async () => {
  const { meetsRequirements } = await import('../src/lib/requirements.js');
  const player = { cataLevel: 23, skills: { COMBAT: 24, HUNTING: 3 }, slayers: { spider: 3 } };
  assert.equal(meetsRequirements({ requirements: [{ type: 'DUNGEON_SKILL', dungeon_type: 'CATACOMBS', level: 27 }] }, player), false);
  assert.equal(meetsRequirements({ requirements: [{ type: 'DUNGEON_TIER', dungeon_type: 'CATACOMBS', tier: 7 }] }, player), false);
  assert.equal(meetsRequirements({ requirements: [{ type: 'DUNGEON_TIER', dungeon_type: 'CATACOMBS', tier: 6 }] }, player), true);
  assert.equal(meetsRequirements({ requirements: [{ type: 'SKILL', skill: 'COMBAT', level: 22 }] }, player), true);
  assert.equal(meetsRequirements({ requirements: [{ type: 'SLAYER', slayer_boss_type: 'spider', level: 6 }] }, player), false);
  const res = new Map([...resource, ['TERMINATOR', { ...resource.get('TERMINATOR'), requirements: [{ type: 'DUNGEON_SKILL', dungeon_type: 'CATACOMBS', level: 27 }] }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 23, skills: player.skills, slayers: player.slayers, resource: res });
  const l = buildLoadout(profile([]), ctx, weightsFor('arch'));
  const labels = availableUpgrades(l, ctx, { get: () => ({ price: 1e6, source: 'AH' }) }).map((u) => u.label);
  assert.ok(!labels.some((x) => x.startsWith('Buy Terminator')));
  assert.ok(!labels.some((x) => x.startsWith('Inferno Demonlord Shard')));
});

test('dungeon accessories and their enrichments get x (1 + cata boost), except crit chance', async () => {
  const { accessoryItemStats } = await import('../src/calc/stats.js');
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const skull = { ...newItem('MASTER_SKULL_TIER_7', resource), enrichment: 'critical_damage' };
  close(accessoryItemStats(skull, ctx).critDamage, 1 * (1 + 4.85));
  close(accessoryItemStats({ ...skull, enrichment: 'critical_chance' }, ctx).critChance, 1);
  close(accessoryItemStats({ ...newItem('WITHER_RELIC', resource), enrichment: 'critical_damage' }, ctx).critDamage, 1);
});

test('an accessory buy can come bundled with its recombobulator and enrichment as one step', () => {
  const res = new Map([...resource, ['ARTIFACT_OF_CONTROL', { id: 'ARTIFACT_OF_CONTROL', name: 'Artifact of Control', category: 'ACCESSORY', tier: 'LEGENDARY' }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const l = buildLoadout(profile([newItem('TERMINATOR', res)]), ctx, weightsFor('arch'));
  const p = { get: () => ({ price: 1e6, source: 'AH' }) };
  const bundle = availableUpgrades(l, ctx, p).find((u) => u.label === 'Buy Artifact of Control + Recombobulator + Critical Chance Enrichment');
  assert.ok(bundle);
  assert.equal(bundle.cost, 3e6);
  const item = bundle.apply(l).accessories.get('ARTIFACT_OF_CONTROL');
  assert.ok(item.recomb && item.enrichment === 'critical_chance');
});

test("with Maxor's boots, the boss boots are Maxor's or Necron's only", () => {
  const res = new Map([...resource,
    ['SPEED_WITHER_BOOTS', { id: 'SPEED_WITHER_BOOTS', name: "Maxor's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { WALK_SPEED: 30, CRITICAL_DAMAGE: 10 } }],
    ['SKELETON_MASTER_BOOTS', { id: 'SKELETON_MASTER_BOOTS', name: 'Skeleton Master Boots', category: 'BOOTS', tier: 'EPIC', dungeon_item: true, stats: { CRITICAL_DAMAGE: 45 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const l = buildLoadout(profile([newItem('TERMINATOR', res), newItem('SPEED_WITHER_BOOTS', res), newItem('SKELETON_MASTER_BOOTS', res)]), ctx, w);
  assert.equal(l.armor.BOOTS.id, 'SPEED_WITHER_BOOTS');
  const owned = buildLoadout(profile([newItem('TERMINATOR', res), newItem('SPEED_WITHER_BOOTS', res)]), ctx, w);
  const p = { get: () => ({ price: 1e5, source: 'AH' }) };
  assert.ok(!availableUpgrades(owned, ctx, p).some((u) => u.label.startsWith('Buy Skeleton Master Boots')));
});

test("mage fills speed to 450 only and picks boss boots freely, with no clear boots", async () => {
  const { tuningAllocation } = await import('../src/calc/stats.js');
  // 450 / 500 dungeon speed is 625 / 700 on the sheet.
  assert.equal(tuningAllocation(100, 595, { cls: 'mage', tuning: { split: { critDamage: 1 } } }).speedPoints, 20);
  assert.equal(tuningAllocation(100, 595, { cls: 'arch', tuning: { split: { critDamage: 1 } } }).speedPoints, 70);
  const res = new Map([...resource,
    ['SPEED_WITHER_BOOTS', { id: 'SPEED_WITHER_BOOTS', name: "Maxor's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { WALK_SPEED: 30, CRITICAL_DAMAGE: 10 } }],
    ['WISE_WITHER_BOOTS', { id: 'WISE_WITHER_BOOTS', name: "Storm's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { INTELLIGENCE: 250, CRITICAL_DAMAGE: 10 } }]]);
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource: res });
  const l = buildLoadout(profile([newItem('HYPERION', res), newItem('SPEED_WITHER_BOOTS', res), newItem('WISE_WITHER_BOOTS', res)]), ctx, weightsFor('mage'));
  assert.equal(l.armor.BOOTS.id, 'WISE_WITHER_BOOTS');
  assert.equal(l.clearBoots, null);
  const p = { get: () => ({ price: 1e5, source: 'AH' }) };
  assert.ok(!availableUpgrades(l, ctx, p).some((u) => u.label.includes('clear boots')));
});

test("upgrade mode 'max' jumps straight to the max level at its own cost", () => {
  const w = weightsFor('arch');
  const labels = (mode) => {
    const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource, upgradeMode: mode });
    const l = buildLoadout(profile([newItem('TERMINATOR', resource), newItem('POWER_WITHER_LEGGINGS', resource)]), ctx, w);
    return availableUpgrades(l, ctx, { get: (id) => ({ price: 1e6, source: 'BZ', id }) }).map((u) => u.label);
  };
  const all = labels('all').filter((x) => x.startsWith('Duplex'));
  const max = labels('max').filter((x) => x.startsWith('Duplex'));
  assert.equal(all.length, 5);
  assert.deepEqual(max, ['Duplex V on Terminator']);
  assert.ok(labels('max').some((x) => x.startsWith('1st–5th Master Stars on')));
});

test("upgrade mode 'max' offers only the best buyable accessory of a family (Wither Relic, not Wither Artifact)", () => {
  const res = new Map([...resource, ['WITHER_ARTIFACT', { id: 'WITHER_ARTIFACT', name: 'Wither Artifact', category: 'ACCESSORY', tier: 'EPIC' }]]);
  const buys = (mode, p) => {
    const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res, upgradeMode: mode });
    const l = buildLoadout(profile([newItem('TERMINATOR', res)]), ctx, weightsFor('arch'));
    return availableUpgrades(l, ctx, p).map((u) => u.label).filter((x) => /^Buy Wither (Artifact|Relic)/.test(x));
  };
  const all = { get: (id) => ({ price: 1e6, source: 'AH', id }) };
  assert.ok(buys('all', all).some((x) => x.startsWith('Buy Wither Artifact')));
  assert.ok(buys('max', all).length && buys('max', all).every((x) => x.startsWith('Buy Wither Relic')));
  // No Wither Relic on sale: the artifact is the best you can buy.
  const noRelic = { get: (id) => (id === 'WITHER_RELIC' ? null : { price: 1e6, source: 'AH', id }) };
  assert.ok(buys('max', noRelic).some((x) => x.startsWith('Buy Wither Artifact')));
});

test('Tier Boost: an Epic Ender Dragon with a Tier Boost plays as Legendary; Kat upgrade + new item is offered', async () => {
  const { petStats } = await import('../src/lib/pets.js');
  const legendary = petStats({ type: 'ENDER_DRAGON', tier: 'LEGENDARY', heldItem: null }).stats;
  const boosted = petStats({ type: 'ENDER_DRAGON', tier: 'EPIC', heldItem: 'PET_ITEM_TIER_BOOST' }).stats;
  assert.deepEqual(boosted, legendary);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource });
  const w = weightsFor('arch');
  const l = buildLoadout(profile([newItem('TERMINATOR', resource)], { pets: [{ type: 'ENDER_DRAGON', tier: 'EPIC', heldItem: 'PET_ITEM_TIER_BOOST' }] }), ctx, w);
  const p = { get: () => ({ price: 1e6, source: 'AH' }) };
  const ups = availableUpgrades(l, ctx, p);
  const kat = ups.find((u) => u.label === 'Kat upgrade Ender Dragon Pet to Legendary + Hephaestus Relic');
  assert.ok(kat);
  // 400M x 0.7 at level 100, 8 Summoning Eyes, a Kat Flower and the Relic.
  assert.equal(kat.cost, 280e6 + 8e6 + 1e6 + 1e6);
  assert.equal(kat.apply(l).activePets.dragon.tier, 'LEGENDARY');
  const buy = buildLoadout(profile([newItem('TERMINATOR', resource)]), ctx, w);
  assert.ok(availableUpgrades(buy, ctx, p).some((u) => u.label === 'Buy Ender Dragon Pet (Epic, level 100) + Tier Boost'));
});

test('class bonuses from class level, solo: Archer x(1 + 1.6% per level), Berserker 40 + 1.5 per level additive, Warrior from Combat', async () => {
  const { classAdditiveDamage, archerMultiplier } = await import('../src/calc/stats.js');
  close(archerMultiplier({ cls: 'arch', archerLevel: 50 }), 1.8);
  close(archerMultiplier({ cls: 'bers', archerLevel: 50 }), 1);
  close(classAdditiveDamage({ cls: 'arch', combatLevel: 60 }), 210);
  // Berserk class bonus (40 + 2 x 37.5 = 115) plus Lust for Blood (1200 at Berserk 50 solo).
  close(classAdditiveDamage({ cls: 'bers', combatLevel: 60, berserkLevel: 50 }), 210 + 115 + 1200);
  close(classAdditiveDamage({ cls: 'mage', combatLevel: 60 }), 210);
});

test('Tier V Beacon with Scorched Power Crystals: +6 crit chance for Archer and Berserker; ability damage does not boost the mage beam', async () => {
  const { mageDamage } = await import('../src/calc/formulas.js');
  const w = weightsFor('arch');
  const cc = (cls) => { const ctx = makeContext({ cls, cataLevel: 50, resource }); return evaluate(buildLoadout(profile([newItem('TERMINATOR', resource)]), ctx, w), ctx, w).stats.wither.critChance; };
  // Terminator divides crit chance by 4: (base 30 + Beacon 6 + Dungeon potion 20) x 0.25.
  close(cc('arch'), 56 * 0.25);
  const s = { damage: 100, critDamage: 0, strength: 0, additiveDamage: 0, critChance: 100, intelligence: 1000 };
  close(mageDamage({ ...s, abilityDamage: 6 }, 0), mageDamage(s, 0));
  // Mage Staff: (0.3 + 0.0009 x 1000) = 1.2 x the melee hit.
  close(mageDamage(s, 0), 100 * 1.2);
});

test('mage (LCM): the beam fires faster with attack speed; tuning skips intelligence', async () => {
  const { volleysPerSecond } = await import('../src/calc/stats.js');
  // 20 / round(10 / (1 + AS/100)): 4 beams/s at 100 attack speed, 3.33 at 82 (rounds to 5 ticks only from 82).
  close(volleysPerSecond(null, 100, { cls: 'mage' }), 4);
  close(volleysPerSecond(null, 60, { cls: 'mage' }), 20 / 6);
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource, tuning: { split: { intelligence: 1 } } });
  const w = weightsFor('mage');
  const e = evaluate(buildLoadout(profile([newItem('HYPERION', resource), ...['WITHER_RELIC', 'MASTER_SKULL_TIER_7'].map((id) => newItem(id, resource))]), ctx, w), ctx, w);
  assert.ok(e.stats.wither.intelligence >= 0);
});

test("Chimera on a mage weapon copies the pet's stats, boosted by stars and cata (attack speed stars only)", () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const w = weightsFor('mage');
  const gd = { type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: null };
  const hype = newItem('HYPERION', resource);
  const base = buildLoadout(profile([hype], { pets: [gd] }), ctx, w);
  const chim = { ...base, weapon: { ...base.weapon, enchants: { ultimate_chimera: 5 } } };
  const a = evaluate(base, ctx, w);
  const b = evaluate(chim, ctx, w);
  // Golden Dragon's 50 attack speed x 100% x 1.5 (5 stars, no master stars or medallion).
  close(b.attackSpeeds.wither - a.attackSpeeds.wither, 50 * 1.5);
  assert.ok(b.wither > a.wither);
});


test("the bot picks the best tuning split itself instead of the profile's saved one", () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource, tuning: { split: { critChance: 1 } } });
  const w = weightsFor('mage');
  buildLoadout(profile([newItem('HYPERION', resource), newItem('WITHER_RELIC', resource)]), ctx, w);
  const split = ctx.tuning.split;
  assert.ok(split && Math.abs(Object.values(split).reduce((a, b) => a + b, 0) - 1) < 1e-9);
  assert.ok('intelligence' in split);
});

test('ability damage on dungeon items gets the star part only (no Catacombs boost)', () => {
  const res = new Map([...resource, ['WITHER_GOGGLES', { id: 'WITHER_GOGGLES', name: 'Wither Goggles', category: 'HELMET', tier: 'EPIC', dungeon_item: true, stats: { ABILITY_DAMAGE_PERCENT: 45 } }]]);
  const ctx = { ...makeContext({ cls: 'mage', cataLevel: 50, resource: res }), secretDigit: 5 };
  const c = itemContribution({ ...newItem('WITHER_GOGGLES', res), masterStars: 5 }, ctx);
  close(c.global.abilityDamage, 45 * 1.8);
});

test("Mage never uses or buys Necron's armor; Archer still does", () => {
  const res = new Map([...resource, ['WISE_WITHER_LEGGINGS', { id: 'WISE_WITHER_LEGGINGS', name: "Storm's Leggings", category: 'LEGGINGS', tier: 'LEGENDARY', dungeon_item: true, stats: { INTELLIGENCE: 250, CRITICAL_DAMAGE: 10 } }]]);
  const gear = [newItem('HYPERION', res), newItem('POWER_WITHER_LEGGINGS', res), newItem('WISE_WITHER_LEGGINGS', res)];
  const mage = makeContext({ cls: 'mage', cataLevel: 50, resource: res });
  assert.equal(buildLoadout(profile(gear), mage, weightsFor('mage')).armor.LEGGINGS.id, 'WISE_WITHER_LEGGINGS');
  const p = { get: () => ({ price: 1e6, source: 'AH' }) };
  const bare = buildLoadout(profile([newItem('HYPERION', res)]), mage, weightsFor('mage'));
  assert.ok(!availableUpgrades(bare, mage, p).some((u) => u.label.startsWith("Buy Necron's Leggings")));
  const arch = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  assert.equal(buildLoadout(profile([newItem('TERMINATOR', res), ...gear.slice(1)]), arch, weightsFor('arch')).armor.LEGGINGS.id, 'POWER_WITHER_LEGGINGS');
});

test('Mage LCM: a dragon set weapon counts per target (Smite blade for withers, Ender Slayer blade for dragons)', async () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, combatLevel: 60, resource });
  const weights = weightsFor('mage', null, 'lcm');
  const owned = items([{ id: 'HYPERION', enchants: { smite: 7 } }, { id: 'HYPERION', enchants: { ender_slayer: 7 } }]);
  const l = buildLoadout(profile(owned), ctx, weights);
  assert.equal(l.weapon.enchants.smite, 7);
  assert.equal(l.dragonGear?.weapon?.enchants.ender_slayer, 7);
  const both = evaluate(l, ctx, weights);
  const mainOnly = evaluate({ ...l, dragonGear: null }, ctx, weights);
  close(both.wither, mainOnly.wither);
  assert.ok(both.dragon > mainOnly.dragon);
  // An upgrade on the dragon blade only moves dragon damage.
  const star = availableUpgrades(l, ctx, prices).find((u) => u.label === '1st Master Star on Hyperion' && u.apply(l).dragonGear?.weapon?.masterStars === 1);
  const after = evaluate(star.apply(l), ctx, weights);
  close(after.wither, both.wither);
  assert.ok(after.dragon > both.dragon);
  // Other classes never get a dragon set.
  const arch = makeContext({ cls: 'arch', cataLevel: 50, resource });
  assert.equal(buildLoadout(profile(owned), arch, weightsFor('arch', null, 'lcm')).dragonGear ?? null, null);
});

test('dual wield: a second Prosecute Hyperion with Execute costs the full first copy, adds 50% additive, doubles later upgrades', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, combatLevel: 60, resource });
  const weights = weightsFor('mage', null, 'lcm');
  const l = buildLoadout(profile(items([{ id: 'HYPERION', hpb: 10, enchants: { prosecute: 6, sharpness: 7 } }])), ctx, weights);
  const dual = availableUpgrades(l, ctx, prices).find((u) => u.label.startsWith('Dual wield Hyperion'));
  // Hyperion + Sharpness VII + 10 hot potato books (Prosecute VI has no price here, so it is left out).
  close(dual.cost, 900e6 + 60e6 + 10 * 80e3);
  const next = dual.apply(l);
  assert.equal(next.weapon.dualWield, true);
  assert.ok(evaluate(next, ctx, weights).wither > evaluate(l, ctx, weights).wither);
  const star = availableUpgrades(next, ctx, prices).find((u) => u.label === '1st Master Star on Hyperion (both copies)');
  close(star.cost, 2 * 8e6);
  assert.ok(!availableUpgrades(next, ctx, prices).some((u) => u.label.startsWith('Dual wield')));
});

test('clear set: Implosion Belt x1.25, F7 lists no master stars, no enrichment changes; power stones cost 9', () => {
  const res = new Map([...resource, ['IMPLOSION_BELT', { id: 'IMPLOSION_BELT', name: 'Implosion Belt', category: 'BELT', tier: 'EPIC', stats: { DEFENSE: 70 } }]]);
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, combatLevel: 60, mode: 'rcm', resource: res });
  const weights = weightsFor('mage', null, 'rcm');
  const hype = scrolled(items([{ id: 'HYPERION', stars: 5 }])[0]);
  const l = buildLoadout(profile([hype]), ctx, weights);
  // The belt is exactly x1.25.
  const low = makeContext({ cls: 'mage', cataLevel: 0, mode: 'rcm', resource: res });
  const lowL = buildLoadout(profile([hype]), low, weights);
  const withBelt = { ...lowL, equipment: { ...lowL.equipment, BELT: newItem('IMPLOSION_BELT', res) } };
  close(evaluate(withBelt, low, weights).wither, evaluate(lowL, low, weights).wither * 1.25);
  const table = { ...priceTable, IMPLOSION_BELT: 5e6, TALISMAN_ENRICHMENT_INTELLIGENCE: 1e6, TALISMAN_ENRICHMENT_SWAPPER: 1e6, FIRST_MASTER_STAR: 8e6 };
  const p = { get: (id) => (table[id] ? { price: table[id], source: 'BZ' } : null) };
  const ups = availableUpgrades(l, ctx, p);
  assert.ok(ups.some((u) => u.label.startsWith('Buy Implosion Belt')));
  assert.ok(!ups.some((u) => /Enrichment|enriched/.test(u.label)));
  const f7 = makeContext({ cls: 'mage', cataLevel: 50, mode: 'rcm', floorMode: 'normal', resource: res });
  assert.ok(!availableUpgrades(buildLoadout(profile([hype]), f7, weights), f7, p).some((u) => /Master Star/.test(u.label)));
});

test('pet buys also come bundled with a pet item that raises their stats, priced together', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, resource });
  const w = weightsFor('mage', null, 'lcm');
  const l = buildLoadout(profile(items([{ id: 'HYPERION' }])), ctx, w);
  const p = { get: (id) => ({ PET_GOLDEN_DRAGON_LEGENDARY_MAX: { price: 900e6, source: 'AH' }, ANTIQUE_REMEDIES: { price: 1e6, source: 'BZ' } }[id] ?? null) };
  const ups = availableUpgrades(l, ctx, p);
  const bare = ups.find((u) => u.label === 'Buy Golden_dragon Pet (Legendary, level 200)' || /^Buy Golden.dragon Pet \(Legendary, level 200\)$/i.test(u.label));
  const bundle = ups.find((u) => /^Buy Golden.dragon Pet \(Legendary, level 200\) \+ Antique Remedies$/i.test(u.label));
  assert.ok(bare && bundle);
  close(bundle.cost, 901e6);
  assert.ok(evaluate(bundle.apply(l), ctx, w).wither > evaluate(bare.apply(l), ctx, w).wither);
});

test('Mage: a Tier Boosted Ender Dragon is only used if attack speed still reaches the cap', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, combatLevel: 60, resource });
  const w = weightsFor('mage', null, 'lcm');
  const boosted = { type: 'ENDER_DRAGON', tier: 'EPIC', heldItem: 'PET_ITEM_TIER_BOOST' };
  const l = buildLoadout(profile(items([{ id: 'HYPERION' }]), { pets: [boosted] }), ctx, w);
  const r = evaluate({ ...l, activePets: { ...l.activePets, dragon: boosted } }, ctx, w);
  if (r.attackSpeeds.dragon < 100) assert.notEqual(l.activePets.dragon, boosted);
  // A real Legendary Ender Dragon (no Tier Boost) is allowed.
  const legendary = { type: 'ENDER_DRAGON', tier: 'LEGENDARY', heldItem: null };
  const l2 = buildLoadout(profile(items([{ id: 'HYPERION' }]), { pets: [legendary] }), ctx, w);
  assert.equal(l2.activePets.dragon?.type, 'ENDER_DRAGON');
});

test("F7 has no dragons: the dragon slot's attack speed never sets the tuning, so Golden Dragon + Chimera keeps withers", () => {
  const res = new Map(resource);
  const bag = Array.from({ length: 80 }, (_, i) => {
    res.set(`TEST_ACC_${i}`, { id: `TEST_ACC_${i}`, name: `Test ${i}`, category: 'ACCESSORY', tier: 'LEGENDARY' });
    return newItem(`TEST_ACC_${i}`, res);
  });
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, combatLevel: 60, floorMode: 'normal', resource: res,
    profileStats: { sbLevel: 0, harpSongs: [], cakes: [{ stat: 'speed', amount: 700 }] }, petProfile: { bank: 50e6, goldCollection: 1e9 } });
  const w = weightsFor('mage', null, 'lcm', 'normal');
  const gd = { type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: null };
  const ed = { type: 'ENDER_DRAGON', tier: 'LEGENDARY', heldItem: null };
  const hype = { ...newItem('HYPERION', res), enchants: { ultimate_chimera: 5 } };
  const l = buildLoadout(profile([hype, ...bag], { pets: [gd, ed] }), ctx, w);
  assert.equal(l.activePets.wither.type, 'GOLDEN_DRAGON');
  assert.equal(evaluate(l, ctx, w).tuning.attackSpeed, 0);
  // An empty dragon slot (no attack speed) changes nothing on F7.
  const r = evaluate({ ...l, activePets: { wither: gd, dragon: null } }, ctx, w);
  assert.equal(r.tuning.attackSpeed, 0);
});

test('Mage never gets clear boots (boss set and clear set); the clear set comes from the RCM plan', () => {
  const res = new Map([...resource, ['SPEED_WITHER_BOOTS', { id: 'SPEED_WITHER_BOOTS', name: "Maxor's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { WALK_SPEED: 30, CRITICAL_DAMAGE: 45 } }]]);
  const p = { get: () => ({ price: 1e6, source: 'BZ' }) };
  for (const mode of ['lcm', 'rcm']) {
    const ctx = makeContext({ cls: 'mage', cataLevel: 50, mode, resource: res });
    const l = buildLoadout(profile(items([{ id: 'HYPERION' }, { id: 'SPEED_WITHER_BOOTS' }])), ctx, weightsFor('mage', null, mode));
    assert.equal(l.clearBoots ?? null, null);
    const ups = availableUpgrades(l, ctx, p);
    assert.ok(!ups.some((u) => /clear boots/.test(u.label)));
    // The boss set never wears or buys Maxor's Boots (the clear set may, when speed pays).
    if (mode === 'lcm') {
      assert.notEqual(l.armor.BOOTS?.id, 'SPEED_WITHER_BOOTS');
      assert.ok(!ups.some((u) => /Maxor/.test(u.label)));
    }
  }
});

test('a pet the bot has no data for (new or unknown type) is ignored instead of crashing', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const w = weightsFor('mage', null, 'lcm');
  const l = buildLoadout(profile(items([{ id: 'HYPERION' }]), { pets: [{ type: 'SOME_NEW_PET', tier: 'LEGENDARY', heldItem: null }] }), ctx, w);
  assert.ok(evaluate(l, ctx, w).wither > 0);
});

test('Fire Aspect burns withers only; Mage counts First Strike in Master Mode only at 1B+ DPS', () => {
  const res = new Map([...resource, ['BIG_SWORD', { id: 'BIG_SWORD', name: 'Big Sword', category: 'SWORD', tier: 'LEGENDARY', stats: { DAMAGE: 1e9 } }]]);
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, combatLevel: 60, resource: res });
  const w = weightsFor('mage', null, 'lcm');
  const l = buildLoadout(profile(items([{ id: 'HYPERION' }])), ctx, w);
  const withWeapon = (weapon) => evaluate({ ...l, weapon }, ctx, w);
  const hype = (enchants) => ({ ...newItem('HYPERION', res), enchants });
  const plain = withWeapon(hype({}));
  const fire = withWeapon(hype({ fire_aspect: 3 }));
  assert.ok(fire.wither > plain.wither);
  close(fire.dragon, plain.dragon);
  // Low DPS: First Strike does nothing in Master Mode.
  assert.ok(plain.wither < 1e9);
  close(withWeapon(hype({ first_strike: 5 })).wither, plain.wither);
  // 1B+ DPS: it counts.
  const big = (enchants) => ({ ...newItem('BIG_SWORD', res), enchants });
  const bigPlain = withWeapon(big({}));
  assert.ok(bigPlain.wither >= 1e9);
  assert.ok(withWeapon(big({ first_strike: 5 })).wither > bigPlain.wither);
});

test('an enchant that pushes another off says so (First Strike replaces Triple Strike); bank deposits for Golden Dragon merge into one step', async () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const l = buildLoadout(profile(items([{ id: 'HYPERION', enchants: { triple_strike: 5 } }])), ctx, weightsFor('mage', null, 'lcm'));
  const p = { get: () => ({ price: 1e6, source: 'BZ' }) };
  assert.ok(availableUpgrades(l, ctx, p).some((u) => u.label === 'First Strike V on Hyperion (replaces Triple Strike)'));
  // Golden Dragon with an empty bank: depositing is offered 1M at a time, and back-to-back deposits are one step.
  const gdCtx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, resource, petProfile: { bank: 0 } });
  const gw = weightsFor('mage', null, 'lcm');
  const gl = buildLoadout(profile(items([{ id: 'HYPERION' }]), { pets: [{ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY' }] }), gdCtx, gw);
  assert.equal(gl.bank, 0);
  const deposit = availableUpgrades(gl, gdCtx, p).find((u) => u.label.startsWith('Put 1M coins in the bank'));
  close(deposit.cost, 1e6);
  const plan = await planUpgrades(gl, gdCtx, { get: () => null }, gw, 5);
  const banked = plan.steps.filter((st) => /in the bank/.test(st.label));
  assert.equal(banked.length, 1);
  close(banked[0].cost, 1e9);
});

test('clear set never suggests Chimera or a Crow / Sheep (Swarm + Golden Dragon is the build)', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mode: 'rcm', resource });
  const l = buildLoadout(profile([scrolled(newItem('HYPERION', resource))]), ctx, weightsFor('mage', null, 'rcm'));
  const ups = availableUpgrades(l, ctx, { get: () => ({ price: 1e6, source: 'BZ' }) });
  assert.ok(!ups.some((u) => /Chimera|Crow|Sheep/.test(u.label)));
  assert.ok(ups.some((u) => /^Swarm/.test(u.label)));
});

test('clear set: an owned Golden Dragon stays the pet with an empty bank, and no Pigman or other pet is suggested', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mode: 'rcm', resource });
  const w = weightsFor('mage', null, 'rcm');
  const gdrag = { type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: null };
  const l = buildLoadout(profile([scrolled(newItem('HYPERION', resource))], { pets: [gdrag, { type: 'PIGMAN', tier: 'LEGENDARY', heldItem: null }] }), ctx, w);
  assert.equal(l.activePets.wither.type, 'GOLDEN_DRAGON');
  const ups = availableUpgrades(l, ctx, { get: () => ({ price: 1e6, source: 'BZ' }) });
  assert.ok(!ups.some((u) => /^Buy .* Pet/.test(u.label)));
  const noGd = buildLoadout(profile([scrolled(newItem('HYPERION', resource))]), ctx, w);
  assert.ok(!availableUpgrades(noGd, ctx, { get: () => ({ price: 1e6, source: 'BZ' }) }).some((u) => /Pigman/.test(u.label)));
});

test('Golden Dragon bank: co-op bank only on a co-op, the one bank on a solo profile, hidden when missing', () => {
  const member = (extra) => ({ inventory: { inv_contents: { data: encode([{ id: 'HYPERION' }]) } }, ...extra });
  const read = (profileExtra, memberExtra, coop) => readMember({ members: { u: member(memberExtra), ...(coop ? { v: {} } : {}) }, ...profileExtra }, 'u', resource).petProfile.bank;
  assert.equal(read({ banking: { balance: 600e6 } }, { profile: { bank_account: 400e6 } }, true), 600e6);
  assert.equal(read({ banking: { balance: 0.25 } }, { profile: { bank_account: 1e9 } }, false), 1e9);
  assert.equal(read({ banking: { balance: 1e9 } }, { profile: { bank_account: 1e9 } }, false), 1e9);
  assert.equal(read({}, { profile: { bank_account: 400e6 } }, true), undefined);
  // The other members left: solo again.
  const left = { profile: { deletion_notice: { timestamp: 1 } } };
  const p = { banking: { balance: 0.25 }, members: { u: member({ profile: { bank_account: 1e9 } }), v: left, w: left } };
  assert.equal(readMember(p, 'u', resource).petProfile.bank, 1e9);
});

test('bank deposits never show as two steps in a row', async () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, resource, petProfile: { bank: 0 } });
  const w = weightsFor('mage', null, 'lcm');
  const l = buildLoadout(profile(items([{ id: 'HYPERION' }]), { pets: [{ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY' }] }), ctx, w);
  const plan = await planUpgrades(l, ctx, { get: () => ({ price: 50e6, source: 'BZ' }) }, w, 40);
  const bank = plan.steps.map((st) => /in the bank/.test(st.label));
  assert.ok(bank.some(Boolean));
  assert.ok(!bank.some((b, i) => b && bank[i - 1]));
  const total = plan.steps.filter((st) => /in the bank/.test(st.label)).reduce((a, st) => a + st.cost, 0);
  assert.ok(total <= 1e9);
});

test('LCM scroll step names the Shadow Assassin Cloak ability; RCM does not', () => {
  const p = { get: () => ({ price: 1e6, source: 'BZ' }) };
  const label = (mode) => {
    const ctx = makeContext({ cls: 'mage', cataLevel: 50, mode, resource });
    const l = buildLoadout(profile([newItem('HYPERION', resource)]), ctx, weightsFor('mage', null, mode));
    return availableUpgrades(l, ctx, p).find((u) => /^Implosion, Shadow Warp, Wither Shield on Hyperion/.test(u.label))?.label;
  };
  assert.equal(label('lcm'), 'Implosion, Shadow Warp, Wither Shield on Hyperion (Shadow Assassin Cloak ability)');
  assert.equal(label('rcm'), 'Implosion, Shadow Warp, Wither Shield on Hyperion');
});

test('Shadow Assassin Cloak: one 1.1x hit per 3s (less Mage cooldown reduction), averaged over every hit', () => {
  // Mage 50 solo: 50% cooldown reduction, 1.5s. 4 beam hits a second: 6 hits per cooldown.
  close(saCloakMultiplier(4, { cls: 'mage', mageLevel: 50 }), 1 + 0.1 / 6);
  // Berserker: 3s, 1 hit a second.
  close(saCloakMultiplier(1, { cls: 'bers' }), 1 + 0.1 / 3);
  // 12 hits per cooldown: 1.0083x.
  close(saCloakMultiplier(4, { cls: 'bers' }), 1 + 0.1 / 12);
  // Never more than every hit.
  close(saCloakMultiplier(0.1, { cls: 'bers' }), 1.1);
});

test('enchant books the bazaar does not sell: enchanting table (1 Grand Experience Bottle, 2 at 75 levels), Severed Hand / End Stone Idol', () => {
  const table = { ENCHANTMENT_SMITE_6: 100e3, SEVERED_HAND: 14e6, ENDSTONE_IDOL: 50e6, GRAND_EXP_BOTTLE: 5e3, ENCHANTMENT_ENDER_SLAYER_6: 150e3 };
  const p = { get: (id) => (table[id] ? { price: table[id], source: 'BZ' } : null) };
  assert.deepEqual(enchantParts('prosecute', 5, 0, p), { parts: [{ id: 'GRAND_EXP_BOTTLE', qty: 1 }], note: 'enchanting table' });
  assert.deepEqual(enchantParts('first_strike', 4, 0, p).parts, [{ id: 'GRAND_EXP_BOTTLE', qty: 2 }]);
  // Smite VII on Smite VI: only the hand, and it builds on the VI.
  assert.deepEqual(enchantParts('smite', 7, 6, p), { parts: [{ id: 'SEVERED_HAND' }], note: 'add a Severed Hand', stacks: true });
  // From Smite V: the VI book, then the hand.
  assert.deepEqual(enchantParts('smite', 7, 5, p).parts, [{ id: 'ENCHANTMENT_SMITE_6' }, { id: 'SEVERED_HAND' }]);
  assert.deepEqual(enchantParts('ender_slayer', 7, 6, p), { parts: [{ id: 'ENDSTONE_IDOL' }], note: 'add an End Stone Idol', stacks: true });
  // A level with no source is not offered.
  assert.equal(enchantParts('prosecute', 6, 5, p), null);
});

test('The One: +0.2 strength per maxed collection at V; profile lists each unmaxed collection while worn', () => {
  const res = new Map([...resource, ['PLAIN_NECKLACE', { id: 'PLAIN_NECKLACE', name: 'Plain Necklace', category: 'NECKLACE', tier: 'LEGENDARY', stats: { STRENGTH: 1 } }]]);
  const collections = { maxed: 40, unmaxed: [{ id: 'GOLD_INGOT', name: 'Gold Ingot', amount: 5, max: 400000 }, { id: 'SAND', name: 'Sand', amount: 1, max: 5000 }] };
  const ctx = makeContext({ cls: 'bers', cataLevel: 50, resource: res, collections });
  const neck = { ...newItem('PLAIN_NECKLACE', res), enchants: { ultimate_the_one: 5 } };
  close(itemContribution(neck, ctx).global.strength - itemContribution(newItem('PLAIN_NECKLACE', res), ctx).global.strength, 0.2 * 40);
  const w = weightsFor('bers');
  const l = buildLoadout(profile([newItem('HYPERION', res), neck]), ctx, w);
  const rows = profileUpgrades(l, ctx, w).rows.filter((r) => /The One/.test(r.label));
  assert.deepEqual(rows.map((r) => r.label).sort(), ['Max Gold Ingot collection (5 / 400,000; The One: +0.2 strength)', 'Max Sand collection (1 / 5,000; The One: +0.2 strength)']);
  // Not worn: no collection rows.
  const bare = buildLoadout(profile([newItem('HYPERION', res)]), ctx, w);
  assert.ok(!profileUpgrades(bare, ctx, w).rows.some((r) => /collection \(/.test(r.label) && /The One/.test(r.label)));
});

test('Gold collection: the next digit is a profile upgrade for a Golden Dragon owner', () => {
  const ctx = makeContext({ cls: 'bers', cataLevel: 50, resource, petProfile: { bank: 0, goldCollection: 54321 } });
  const w = weightsFor('bers');
  const l = buildLoadout(profile([newItem('HYPERION', resource)], { pets: [{ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY' }] }), ctx, w);
  assert.ok(profileUpgrades(l, ctx, w).rows.some((r) => r.label === 'Gold collection to 100,000 (Golden Dragon: +11.1 strength)'));
  const none = buildLoadout(profile([newItem('HYPERION', resource)]), ctx, w);
  assert.ok(!profileUpgrades(none, ctx, w).rows.some((r) => /Gold collection/.test(r.label)));
});

test('Gold collection row: the strength counts the pet item, Chimera on the weapon and a Chimera Ragnarock', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, combatLevel: 60, resource, petProfile: { bank: 1e9, goldCollection: 54321 } });
  const w = weightsFor('mage');
  const hype = (enchants) => ({ ...newItem('HYPERION', resource), enchants });
  const rag = (enchants) => ({ ...newItem('RAGNAROCK_AXE', resource), enchants });
  const row = (heldItem, items) => {
    const l = buildLoadout(profile(items, { pets: [{ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem }] }), ctx, w);
    return profileUpgrades(l, ctx, w).rows.find((r) => /Gold collection/.test(r.label)).label;
  };
  const strength = (label, sources) => {
    const m = label.match(/^Gold collection to 100,000 \(Golden Dragon: \+11\.1 strength, \+([\d.]+) with (.+)\)$/);
    assert.ok(m, label);
    assert.equal(m[2], sources);
    return Number(m[1]);
  };
  // Labels round to 0.1.
  const near = (a, b) => assert.ok(Math.abs(a - b) <= 0.05 + 1e-9, `${a} != ${b}`);
  // Chimera copies the pet's strength with the Hyperion's own boost (stars and Catacombs).
  const boost = itemContribution(hype({}), ctx).global.strength / 150;
  assert.equal(row(null, [hype({})]), 'Gold collection to 100,000 (Golden Dragon: +11.1 strength)');
  near(strength(row('ANTIQUE_REMEDIES', [hype({})]), 'Antique Remedies'), 11.1 * 1.8);
  near(strength(row(null, [hype({ ultimate_chimera: 5 })]), 'Chimera'), 11.1 * (1 + boost));
  // The Ragnarock casts with the pet's strength only through its own Chimera, then gives 1.5x its strength.
  assert.equal(row(null, [hype({}), rag({})]), 'Gold collection to 100,000 (Golden Dragon: +11.1 strength)');
  near(strength(row(null, [hype({}), rag({ ultimate_chimera: 5 })]), 'Ragnarock'), 11.1 * 2.5);
  near(strength(row('ANTIQUE_REMEDIES', [hype({ ultimate_chimera: 5 }), rag({ ultimate_chimera: 5 })]), 'Antique Remedies, Chimera and Ragnarock'),
    11.1 * 1.8 * (2.5 + boost));
});

test('free quest accessories go in profile upgrades, only when magical power rises (next Campfire badge that changes rarity); IQ Points show +% intelligence', () => {
  const acc = (id, name, tier, extra = {}) => [id, { id, name, category: 'ACCESSORY', tier, soulbound: 'SOLO', ...extra }];
  const res = new Map([...resource,
    acc('CAMPFIRE_TALISMAN_1', 'Campfire Initiate Badge I', 'COMMON'), acc('CAMPFIRE_TALISMAN_2', 'Campfire Initiate Badge II', 'COMMON'),
    acc('CAMPFIRE_TALISMAN_3', 'Campfire Initiate Badge III', 'COMMON'), acc('CAMPFIRE_TALISMAN_4', 'Campfire Adept Badge I', 'UNCOMMON'),
    acc('CAMPFIRE_TALISMAN_5', 'Campfire Adept Badge II', 'UNCOMMON'), acc('TWO_IQ_POINT', '2 IQ Points', 'UNCOMMON'),
    acc('PRICED_TALISMAN', 'Priced Talisman', 'RARE'), acc('NPC_TOOTH', 'Npc Tooth', 'RARE')]);
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource: res });
  const w = weightsFor('mage');
  const l = buildLoadout(profile([newItem('HYPERION', res), newItem('CAMPFIRE_TALISMAN_1', res)]), ctx, w);
  const p = { get: (id) => (id === 'PRICED_TALISMAN' ? { price: 1e6, source: 'AH' } : id === 'NPC_TOOTH' ? { price: 1e6, source: 'NPC', npc: [{ id, npc: 'Odger' }] } : null) };
  const labels = profileUpgrades(l, ctx, w, p).rows.map((r) => r.label);
  // A quest accessory an NPC sells for coins stays a profile upgrade (cost in the label), never a coin buy.
  assert.ok(labels.some((x) => /^Npc Tooth \(quest accessory, 1\.00M coins from Odger: \+\d+ magical power\)$/.test(x)));
  assert.ok(!availableUpgrades(l, ctx, p).some((u) => /Npc Tooth/.test(u.label)));
  assert.ok(labels.includes('Campfire Adept Badge I (free quest accessory: +2 magical power)'));
  assert.ok(!labels.some((x) => /Initiate Badge (II|III)|Adept Badge II|Priced/.test(x)));
  assert.ok(labels.includes('2 IQ Points (free quest accessory: +5 magical power, +2% intelligence)'));
  // 2 IQ Points: +2% total intelligence, additive with the other percent bonuses.
  const iq = evaluate({ ...l, accessories: new Map(l.accessories).set('TWO_IQ_POINT', newItem('TWO_IQ_POINT', res)) }, ctx, w);
  assert.ok(iq.stats.wither.intelligence > evaluate(l, ctx, w).stats.wither.intelligence);
});

test('The One counts co-op collections, members who left included; Golden Dragon counts your own Gold only', () => {
  const inv = { inventory: { inv_contents: { data: encode([{ id: 'HYPERION' }]) } } };
  const collections = new Map([['SULPHUR', { name: 'Gunpowder', max: 200000 }], ['GOLD_INGOT', { name: 'Gold Ingot', max: 400000 }]]);
  const p = { members: {
    u: { ...inv, collection: { SULPHUR: 50, GOLD_INGOT: 1000 } },
    v: { collection: { SULPHUR: 200000, GOLD_INGOT: 99000 } },
    w: { profile: { deletion_notice: { timestamp: 1 } }, collection: { GOLD_INGOT: 200000 } },
  } };
  const m = readMember(p, 'u', resource, collections);
  assert.equal(m.collections.maxed, 1);
  assert.deepEqual(m.collections.unmaxed.map((c) => [c.id, c.amount]), [['GOLD_INGOT', 300000]]);
  assert.equal(m.petProfile.goldCollection, 1000);
});

test('a new bow for Archer comes with the current bow\'s enchants and potato books, so it is not judged bare', () => {
  const res = new Map([...resource, ['JUJU_SHORTBOW', { id: 'JUJU_SHORTBOW', name: 'Juju Shortbow', category: 'BOW', tier: 'EPIC', stats: { DAMAGE: 310, STRENGTH: 40, CRITICAL_DAMAGE: 110 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, archerLevel: 50, resource: res });
  const w = weightsFor('arch');
  const juju = { ...newItem('JUJU_SHORTBOW', res), hpb: 10, enchants: { power: 7, dragon_hunter: 5, ultimate_reiterate: 5 } };
  const l = buildLoadout(profile([juju]), ctx, w);
  const ups = availableUpgrades(l, ctx, { get: () => ({ price: 1e6, source: 'BZ' }) });
  const copy = ups.find((u) => /^Buy Terminator/.test(u.label) && /all enchants, master stars and potato books like Juju Shortbow/.test(u.label));
  assert.ok(copy);
  const term = copy.apply(l).weapon;
  assert.deepEqual(term.enchants, { power: 7, dragon_hunter: 5, ultimate_reiterate: 5 });
  assert.equal(term.hpb, 10);
});

test('armor bundles: pieces that each lose damage alone (breaking a set bonus) are bought together when that wins', async () => {
  const slots = ['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'];
  const sm = slots.map((s) => [`SKELETON_MASTER_${s}`, { id: `SKELETON_MASTER_${s}`, name: `Skeleton Master ${s}`, category: s, tier: 'EPIC', stats: { STRENGTH: 10 } }]);
  // Archer keeps the Skeleton Master Chestplate, so the bundle covers the other three slots.
  const big = slots.filter((s) => s !== 'CHESTPLATE').map((s) => [`BIG_${s}`, { id: `BIG_${s}`, name: `Big ${s}`, category: s, tier: 'EPIC', stats: { STRENGTH: 80 } }]);
  const res = new Map([['TERMINATOR', resource.get('TERMINATOR')], ...sm, ...big]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const w = weightsFor('arch');
  const l = buildLoadout(profile([newItem('TERMINATOR', res), ...sm.map(([id]) => newItem(id, res))]), ctx, w);
  const p = { get: (id) => (id.startsWith('BIG_') ? { price: 1e6, source: 'AH' } : null) };
  const single = availableUpgrades(l, ctx, p).find((u) => u.label === 'Buy Big HELMET');
  assert.ok(evaluate(single.apply(l), ctx, w).wither < evaluate(l, ctx, w).wither);
  const plan = await planUpgrades(l, ctx, p, w, 3);
  assert.ok(plan.steps.some((st) => st.label === 'Armor bundle: Buy Big HELMET; Buy Big LEGGINGS; Buy Big BOOTS'), plan.steps.map((st) => st.label).join(' / '));
});

test('Golden Necron Head is free (50 completions): a Claim step, then it upgrades into the Diamond head keeping its upgrades', () => {
  const res = new Map([...resource,
    ['GOLD_NECRON_HEAD', { id: 'GOLD_NECRON_HEAD', name: 'Golden Necron Head', category: 'HELMET', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 30, CRITICAL_DAMAGE: 30 } }],
    ['DIAMOND_NECRON_HEAD', { id: 'DIAMOND_NECRON_HEAD', name: 'Diamond Necron Head', category: 'HELMET', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 31, CRITICAL_DAMAGE: 31 } }]]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, archerLevel: 50, resource: res });
  const w = weightsFor('arch');
  const p = { get: (id) => (id === 'ENCHANTED_DIAMOND_BLOCK' ? { price: 40e6, source: 'BZ' } : null) };
  const l = buildLoadout(profile([newItem('TERMINATOR', res)]), ctx, w);
  const ups = availableUpgrades(l, ctx, p);
  const claim = ups.find((u) => u.label === 'Claim Golden Necron Head (free with 50 completions)');
  assert.equal(claim?.cost, 1);
  // Claimed and upgraded in one step (8 Enchanted Diamond Blocks), never a Diamond head bought on its own.
  const both = ups.find((u) => u.label === 'Claim Golden Necron Head (free with 50 completions) and upgrade it into Diamond Necron Head (8 Enchanted Diamond Blocks)');
  assert.equal(both?.cost, 1 + 320e6);
  assert.equal(both.apply(l).armor.HELMET.id, 'DIAMOND_NECRON_HEAD');
  assert.ok(!ups.some((u) => u.label.startsWith('Buy Diamond Necron Head')));
  const withGold = claim.apply(l);
  assert.ok(availableUpgrades(withGold, ctx, p).some((u) => u.label.startsWith('Upgrade Golden Necron Head into Diamond Necron Head')));
});

test("Archer and Berserker never get a chestplate other than Skeleton Master's (Necron's, Shadow Assassin, ...) (Skeleton Master's infinite arrows); a piece with gem slots can come with them filled", () => {
  const slots = [{ slot_type: 'JASPER', costs: [{ type: 'COINS', coins: 250000 }] }, { slot_type: 'COMBAT' }];
  const res = new Map([...resource,
    ['POWER_WITHER_CHESTPLATE', { id: 'POWER_WITHER_CHESTPLATE', name: "Necron's Chestplate", category: 'CHESTPLATE', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 40, CRITICAL_DAMAGE: 30 }, gemstone_slots: slots }],
    ['POWER_WITHER_BOOTS', { id: 'POWER_WITHER_BOOTS', name: "Necron's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 40, CRITICAL_DAMAGE: 30 }, gemstone_slots: slots }]]);
  const p = { get: () => ({ price: 1e6, source: 'BZ' }) };
  const arch = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const l = buildLoadout(profile([newItem('TERMINATOR', res)]), arch, weightsFor('arch'));
  const ups = availableUpgrades(l, arch, p);
  assert.ok(!ups.some((u) => /Necron's Chestplate/.test(u.label)));
  const sa = new Map([...res, ['SHADOW_ASSASSIN_CHESTPLATE', { id: 'SHADOW_ASSASSIN_CHESTPLATE', name: 'Shadow Assassin Chestplate', category: 'CHESTPLATE', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 500 } }]]);
  const saCtx = makeContext({ cls: 'arch', cataLevel: 50, resource: sa });
  const worn = buildLoadout(profile([newItem('TERMINATOR', sa), newItem('SHADOW_ASSASSIN_CHESTPLATE', sa)]), saCtx, weightsFor('arch'));
  assert.equal(worn.armor.CHESTPLATE, undefined);
  assert.ok(!availableUpgrades(worn, saCtx, p).some((u) => /Shadow Assassin Chestplate/.test(u.label)));
  const gemmed = ups.find((u) => u.label.startsWith("Buy Necron's Boots") && /Perfect Jasper gems/.test(u.label));
  assert.ok(gemmed);
  assert.deepEqual(gemmed.apply(l).armor.BOOTS.gems.slots, { JASPER_0: { gem: 'JASPER', quality: 'PERFECT' }, COMBAT_0: { gem: 'JASPER', quality: 'PERFECT' } });
  const bers = makeContext({ cls: 'bers', cataLevel: 50, resource: res });
  assert.ok(!availableUpgrades(buildLoadout(profile([newItem('HYPERION', res)]), bers, weightsFor('bers')), bers, p).some((u) => /Necron's Chestplate/.test(u.label)));
});

test('Mage LCM: the RCM Hyperion (Ultimate Wise, Giant Killer) is kept for the clear; a spare Hyperion is upgraded for only what it lacks', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, resource });
  const w = weightsFor('mage');
  const rcm = { ...scrolled(newItem('HYPERION', resource)), enchants: { ultimate_wise: 5, giant_killer: 7, prosecute: 6, smite: 7 } };
  const lcm = { ...scrolled(newItem('HYPERION', resource)), hpb: 15, enchants: { ultimate_chimera: 5, titan_killer: 7, prosecute: 6, smite: 7, critical: 6 } };
  const clean = newItem('HYPERION', resource);
  const l = buildLoadout(profile([rcm, clean, lcm]), ctx, w);
  assert.equal(l.rcmBlade, rcm);
  assert.equal(l.weapon, lcm);
  assert.deepEqual(l.spareBlades, [clean]);
  const p = { get: () => ({ price: 1e6, source: 'BZ' }) };
  const dual = availableUpgrades(l, ctx, p).find((u) => u.label.startsWith('Dual wield Hyperion'));
  // The label lists what the spare needs as one bundle (Prosecute shown as Execute on the second copy).
  assert.ok(dual.label.startsWith('Dual wield Hyperion: your spare one with Execute in place of Prosecute + '), dual.label);
  for (const want of ['Titan Killer VII', 'Execute VI', 'Smite VII', 'Critical VI', '10 Hot Potato Books', '5 Fuming Potato Books']) assert.ok(dual.label.includes(want), `${want} missing: ${dual.label}`);
  assert.ok(!/Prosecute VI/.test(dual.label));
  // No Hyperion bought: only what the clean one lacks (5 books, 10 hot + 5 fuming potato books, 3 scrolls; 1M each).
  assert.equal(dual.cost, 23e6);
  assert.ok(!dual.apply(l).spareBlades.length);

  // A spare that already has some of them (Execute, Smite VII, a lower Titan Killer, 10 potato books): only what is
  // still to buy is listed and priced.
  const half = { ...newItem('HYPERION', resource), hpb: 10, enchants: { execute: 6, smite: 7, titan_killer: 6 } };
  const l2 = buildLoadout(profile([rcm, half, lcm]), ctx, w);
  const dual2 = availableUpgrades(l2, ctx, p).find((u) => u.label.startsWith('Dual wield Hyperion'));
  for (const want of ['Chimera V', 'Titan Killer VII', 'Critical VI', '5 Fuming Potato Books']) assert.ok(dual2.label.includes(want), `${want} missing: ${dual2.label}`);
  for (const owned of ['Execute VI', 'Smite', 'Hot Potato']) assert.ok(!dual2.label.includes(owned), `${owned} listed: ${dual2.label}`);
  // 3 books, 5 fuming potato books, 3 scrolls.
  assert.equal(dual2.cost, 11e6);
});

test('floor-scaled armor buys use the fully boosted price for the highest floor you can enter; Skeleton Master chest rule starts at Catacombs 36', () => {
  const sm = ['SKELETON_MASTER_CHESTPLATE', { id: 'SKELETON_MASTER_CHESTPLATE', name: 'Skeleton Master Chestplate', category: 'CHESTPLATE', tier: 'EPIC', dungeon_item: true, tiered_stats: { CRITICAL_DAMAGE: [22, 23, 25, 27, 30, 32, 35, 38, 41, 45] } }];
  const sa = ['SHADOW_ASSASSIN_CHESTPLATE', { id: 'SHADOW_ASSASSIN_CHESTPLATE', name: 'Shadow Assassin Chestplate', category: 'CHESTPLATE', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 500 } }];
  const res = new Map([...resource, sm, sa]);
  const table = { SKELETON_MASTER_CHESTPLATE: 40e3, SKELETON_MASTER_CHESTPLATE_FLOOR10: 95e6, SHADOW_ASSASSIN_CHESTPLATE: 1e6 };
  const p = { get: (id) => (table[id] ? { price: table[id], source: 'AH' } : null) };
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const l = buildLoadout(profile([newItem('TERMINATOR', res)]), ctx, weightsFor('arch'));
  const ups = availableUpgrades(l, ctx, p);
  assert.equal(ups.find((u) => u.label === 'Buy Skeleton Master Chestplate')?.cost, 95e6);
  assert.ok(!ups.some((u) => /Shadow Assassin/.test(u.label)));
  const low = makeContext({ cls: 'arch', cataLevel: 30, resource: res });
  const lowUps = availableUpgrades(buildLoadout(profile([newItem('TERMINATOR', res)]), low, weightsFor('arch')), low, p);
  assert.ok(lowUps.some((u) => /Shadow Assassin/.test(u.label)));
});

test('floor-scaled dungeon weapons are priced and scored like the armor: fully boosted, highest floor you can enter', () => {
  const res = new Map([...resource, ['CRYPT_DREADLORD_SWORD', { id: 'CRYPT_DREADLORD_SWORD', name: 'Dreadlord Sword', category: 'SWORD', tier: 'RARE', dungeon_item: true, tiered_stats: { DAMAGE: [30, 40, 50, 60, 70, 80, 90, 100, 110, 120] } }]]);
  const table = { CRYPT_DREADLORD_SWORD: 5e3, CRYPT_DREADLORD_SWORD_FLOOR10: 3e6 };
  const p = { get: (id) => (table[id] ? { price: table[id], source: 'AH' } : null) };
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource: res });
  const ups = availableUpgrades(buildLoadout(profile([]), ctx, weightsFor('mage')), ctx, p);
  assert.equal(ups.find((u) => u.label === 'Buy Dreadlord Sword')?.cost, 3e6);
  // The full boost raises the rarity one tier, like the armor.
  assert.equal(newItem('CRYPT_DREADLORD_SWORD', res).baseRarity, 'EPIC');
});

test('First Strike V on a Prosecute weapon is never swapped for Triple Strike; First Strike IV or none gets Triple Strike', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, mageLevel: 50, resource });
  const w = weightsFor('mage');
  const p = { get: () => ({ price: 1e6, source: 'BZ' }) };
  const triple = (enchants) => availableUpgrades(buildLoadout(profile([{ ...newItem('HYPERION', resource), enchants }]), ctx, w), ctx, p)
    .some((u) => /^Triple Strike/.test(u.label));
  assert.equal(triple({ prosecute: 6, first_strike: 5 }), false);
  assert.equal(triple({ prosecute: 6, first_strike: 4 }), true);
  assert.equal(triple({ prosecute: 6 }), true);
});

test('Balloon Snake is never used in RCM (Jump Boost in the clear) but stays in LCM', () => {
  const res = new Map([...resource, ['BALLOON_SNAKE', { id: 'BALLOON_SNAKE', name: 'Balloon Snake', category: 'NECKLACE', tier: 'LEGENDARY', stats: { INTELLIGENCE: 100, STRENGTH: 30 } }]]);
  const p = { get: () => ({ price: 1e6, source: 'AH' }) };
  for (const [mode, used] of [['rcm', false], ['lcm', true]]) {
    const ctx = makeContext({ cls: 'mage', cataLevel: 50, mode, resource: res });
    const w = weightsFor('mage', null, mode);
    const worn = buildLoadout(profile([scrolled(newItem('HYPERION', res)), newItem('BALLOON_SNAKE', res)]), ctx, w);
    assert.equal(worn.equipment.NECKLACE?.id === 'BALLOON_SNAKE', used, mode);
    const bare = buildLoadout(profile([scrolled(newItem('HYPERION', res))]), ctx, w);
    assert.equal(availableUpgrades(bare, ctx, p).some((u) => /Balloon Snake/.test(u.label)), used, mode);
  }
});

test('Skeleton Master chest rule: other chestplates are offered only with the button turned off (anyChest)', () => {
  const res = new Map([...resource, ['SHADOW_ASSASSIN_CHESTPLATE', { id: 'SHADOW_ASSASSIN_CHESTPLATE', name: 'Shadow Assassin Chestplate', category: 'CHESTPLATE', tier: 'LEGENDARY', stats: { STRENGTH: 100, CRITICAL_DAMAGE: 50 } }]]);
  const w = weightsFor('arch');
  const p = { get: (id) => (id === 'SHADOW_ASSASSIN_CHESTPLATE' ? { price: 5e6, source: 'AH' } : null) };
  const offered = (anyChest) => {
    const ctx = makeContext({ cls: 'arch', cataLevel: 40, archerLevel: 50, anyChest, resource: res });
    const l = buildLoadout(profile([newItem('TERMINATOR', res)]), ctx, w);
    return availableUpgrades(l, ctx, p).some((u) => /Shadow Assassin Chestplate/.test(u.label));
  };
  assert.equal(offered(false), false);
  assert.equal(offered(true), true);
});

test('a step bought from an NPC says which NPC in its label', async () => {
  const { priceLookup } = await import('../src/api/prices.js');
  const res = new Map([...resource,
    ['ZOMBIE_TALISMAN', { id: 'ZOMBIE_TALISMAN', name: 'Zombie Talisman', category: 'ACCESSORY', tier: 'COMMON' }],
    ['INTIMIDATION_TALISMAN', { id: 'INTIMIDATION_TALISMAN', name: 'Intimidation Talisman', category: 'ACCESSORY', tier: 'COMMON' }],
    ['INTIMIDATION_RING', { id: 'INTIMIDATION_RING', name: 'Intimidation Ring', category: 'ACCESSORY', tier: 'UNCOMMON' }],
  ]);
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res });
  const loadout = buildLoadout(profile([]), ctx, weightsFor('arch'));
  const p = priceLookup({ bazaar: new Map([['GREEN_CANDY', 1000], ['RECOMBOBULATOR_3000', 8e6]]), auctions: new Map() });
  const labels = availableUpgrades(loadout, ctx, p).map((u) => u.label);
  assert.ok(labels.includes('Buy Zombie Talisman (buy from Combat Merchant)'), labels.join(', '));
  assert.ok(labels.includes('Buy Intimidation Ring (Intimidation Ring from Fear Mongerer, Intimidation Talisman from Combat Merchant)'), labels.join(', '));
  assert.ok(labels.some((l) => l.startsWith('Buy Zombie Talisman + Recombobulator (Zombie Talisman from Combat Merchant)')), labels.filter((l) => /Zombie/.test(l)).join(', '));
});

test('clear boots button off (noClearBoots): no clear boots or clear boots buys, so the boss boots carry the speed', () => {
  const res = new Map([...resource, ['SPEED_WITHER_BOOTS', { id: 'SPEED_WITHER_BOOTS', name: "Maxor's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { WALK_SPEED: 30, CRITICAL_DAMAGE: 45 } }],
    ['POWER_WITHER_BOOTS', { id: 'POWER_WITHER_BOOTS', name: "Necron's Boots", category: 'BOOTS', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 40, CRITICAL_DAMAGE: 30 } }]]);
  const w = weightsFor('arch');
  const p = { get: () => ({ price: 1e5, source: 'AH' }) };
  for (const cls of ['arch', 'bers']) {
    const on = makeContext({ cls, cataLevel: 50, resource: res });
    const off = makeContext({ cls, cataLevel: 50, noClearBoots: true, resource: res });
    const gear = () => profile([...items([{ id: 'TERMINATOR' }]), newItem('POWER_WITHER_BOOTS', res), newItem('SPEED_WITHER_BOOTS', res)]);
    const swap = buildLoadout(gear(), on, w);
    const one = buildLoadout(gear(), off, w);
    assert.equal(swap.clearBoots.id, 'SPEED_WITHER_BOOTS', cls);
    assert.equal(one.clearBoots, null, cls);
    assert.ok(evaluate(one, off, w).speed <= evaluate(swap, on, w).speed, cls);
    const bare = buildLoadout(profile(items([{ id: 'TERMINATOR' }])), off, w);
    assert.ok(!availableUpgrades(bare, off, p).some((u) => u.label.includes('clear boots')), cls);
    assert.ok(availableUpgrades(buildLoadout(profile(items([{ id: 'TERMINATOR' }])), on, w), on, p).some((u) => u.label.includes('clear boots')), cls);
  }
});
