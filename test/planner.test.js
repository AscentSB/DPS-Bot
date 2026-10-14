import test from 'node:test';
import assert from 'node:assert/strict';
import zlib from 'node:zlib';
import nbt from 'prismarine-nbt';
import { decodeItems, toItemState } from '../src/lib/nbt.js';
import { makeContext, buildLoadout, planUpgrades, weightsFor, availableUpgrades, newItem } from '../src/calc/upgrades.js';
import { evaluate, itemContribution, shardStats, totalAccessoryPower, accessoryStats } from '../src/calc/stats.js';

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

test('greedy plan is in dependency order and every step gains damage', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const weights = weightsFor('mage');
  const plan = planUpgrades(buildLoadout(profile([]), ctx, weights), ctx, prices, weights, 12);
  const labels = plan.steps.map((s) => s.label);
  const buy = labels.indexOf('Buy Hyperion');
  const star = labels.indexOf('1st Master Star on Hyperion');
  if (star !== -1) assert.ok(buy !== -1 && buy < star);
  for (const s of plan.steps) assert.ok(s.gain > 0 && s.perMillion > 0);
});

test('smite only helps withers, ender slayer only helps dragons', () => {
  const ctx = makeContext({ cls: 'bers', cataLevel: 50, resource });
  const w = weightsFor('bers');
  const base = buildLoadout(profile(items([{ id: 'GIANTS_SWORD' }])), ctx, w);
  const ups = availableUpgrades(base, ctx, prices);
  const before = evaluate(base, ctx, w);
  const smite = evaluate(ups.find((u) => u.label.startsWith('Smite VII')).apply(base), ctx, w);
  const ender = evaluate(ups.find((u) => u.label.startsWith('Ender Slayer VII')).apply(base), ctx, w);
  assert.ok(smite.wither > before.wither && smite.dragon === before.dragon);
  assert.ok(ender.dragon > before.dragon && ender.wither === before.wither);
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
  const chim = buildLoadout(profile(items([{ id: 'TERMINATOR' }, { id: 'RAGNAROCK_AXE', stars: 0, enchants: { ultimate_chimera: 5 } }]), { pets: [{ type: 'GOLDEN_DRAGON', heldItem: 'ANTIQUE_REMEDIES' }] }), ctx, w);
  assert.ok(chim.extras.has('PET_GOLDEN_DRAGON'));
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

test('speed only helps past the 500 cap, capped by points in speed', () => {
  const res = new Map([...resource, ['FAST_BOOTS', { id: 'FAST_BOOTS', name: 'Fast Boots', category: 'BOOTS', tier: 'COMMON', stats: { WALK_SPEED: 380 } }]]);
  const w = weightsFor('arch');
  const cases = [
    [null, 0, false], // 495 -> 500: still at or below the cap
    [null, 5, true],
    [{ allocation: { speed: 20, critDamage: 20 }, split: { speed: 0.5, critDamage: 0.5 } }, 0, true], // 495 gear speed + 30 from tuning: 16.7 of 20 points free, +5 speed frees more
    [{ allocation: { critDamage: 30 }, split: { critDamage: 1 } }, 5, false],
  ];
  for (const [tuning, extraBoots, expectGain] of cases) {
    const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res, tuning });
    const boots = { ...newItem('FAST_BOOTS', res) };
    const loadout = buildLoadout(profile([...items([{ id: 'TERMINATOR' }]), boots]), ctx, w);
    // Start at 100 + 15 potion + 380 = 495 (+ extraBoots via shard levels), then add 5 more speed.
    const start = { ...loadout, shards: { OBSIDIAN_DEFENDER: extraBoots } };
    const faster = { ...loadout, shards: { OBSIDIAN_DEFENDER: extraBoots + 5 } };
    const a = evaluate(start, ctx, w);
    const b = evaluate(faster, ctx, w);
    assert.equal(b.speed - a.speed, 5);
    assert.equal(b.dragon > a.dragon, expectGain, `tuning=${JSON.stringify(tuning)} start=${a.speed}`);
  }
});

test('Wolf Slayer speed: +1 at levels 1, 3 and 8', async () => {
  const { wolfSlayerSpeed } = await import('../src/api/hypixel.js');
  assert.equal(wolfSlayerSpeed(0), 0);
  assert.equal(wolfSlayerSpeed(10), 1);
  assert.equal(wolfSlayerSpeed(250), 2);
  assert.equal(wolfSlayerSpeed(400000), 3);
});
