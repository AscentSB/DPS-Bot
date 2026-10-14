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
  assert.equal(chim.activePet.type, 'GOLDEN_DRAGON');
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
  // 200 points, 380 speed: 80 points to speed (120 speed), 120 points to crit chance (0.2 each).
  const a = tuningAllocation(200, 380, ctx);
  assert.equal(a.speedPoints, 80);
  assert.equal(a.stats.speed, 120);
  assert.ok(Math.abs(a.stats.critChance - 24) < 1e-9);
  // Not enough points to reach 500: everything goes to speed, nothing to crit.
  const b = tuningAllocation(50, 300, ctx);
  assert.equal(b.speedPoints, 50);
  assert.equal(b.stats.critChance, undefined);
  // Already at 500: all points go to crit.
  assert.equal(tuningAllocation(100, 520, ctx).speedPoints, 0);
});

test('a speed upgrade below 500 frees tuning points for crit; past 500 it does nothing', () => {
  const res = new Map([...resource, ['FAST_BOOTS', { id: 'FAST_BOOTS', name: 'Fast Boots', category: 'BOOTS', tier: 'COMMON', stats: { WALK_SPEED: 200 } }]]);
  const w = weightsFor('arch');
  const ctx = makeContext({ cls: 'arch', cataLevel: 50, resource: res, tuning: { split: { critChance: 1 } } });
  const loadout = buildLoadout(profile([...items([{ id: 'TERMINATOR' }]), newItem('FAST_BOOTS', res)]), ctx, w);
  // Give the bag plenty of tuning points.
  loadout.accessories = new Map(Array.from({ length: 200 }, (_, i) => [`A${i}`, { id: `A${i}`, category: 'ACCESSORY', baseRarity: 'MYTHIC', enchants: {} }]));
  const base = evaluate(loadout, ctx, w);
  assert.ok(base.speed >= 500 && base.tuning.speed > 0);
  const faster = evaluate({ ...loadout, shards: { OBSIDIAN_DEFENDER: 10 } }, ctx, w);
  assert.ok(faster.tuning.speed < base.tuning.speed);
  assert.ok(faster.stats.dragon.critChance > base.stats.dragon.critChance);
  // Past 500 with no speed points left to free, more speed changes nothing.
  const capped = { ...loadout, armor: { BOOTS: { ...newItem('FAST_BOOTS', res), id: 'FAST_BOOTS' } }, shards: {} };
  const res2 = new Map([...res, ['FAST_BOOTS', { ...res.get('FAST_BOOTS'), stats: { WALK_SPEED: 450 } }]]);
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
  const gd = petStats({ type: 'GOLDEN_DRAGON', tier: 'LEGENDARY', heldItem: 'HEPHAESTUS_REMEDIES' });
  assert.ok(Math.abs(gd.stats.strength - (50 + 99.9) * 2) < 1e-6);
  assert.equal(gd.stats.additiveStrength, 0.05);
  assert.equal(gd.stats.additiveDamage, 250);
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
  assert.equal(both.activePet.type, 'GOLDEN_DRAGON');
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
  const a = tuningAllocation(300, 440, ctx, 70); // 100 points AS, 40 points speed, 160 crit
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
  const base = buildLoadout(profile([newItem('TERMINATOR', res)]), c, w);
  const ed = evaluate({ ...base, activePet: { type: 'ENDER_DRAGON', tier: 'LEGENDARY', heldItem: null } }, c, w);
  const none = evaluate({ ...base, activePet: null }, c, w);
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
