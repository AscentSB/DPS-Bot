import test from 'node:test';
import assert from 'node:assert/strict';
import zlib from 'node:zlib';
import nbt from 'prismarine-nbt';
import { decodeItems, toItemState } from '../src/lib/nbt.js';
import { makeContext, buildLoadout, planUpgrades, weightsFor, availableUpgrades } from '../src/calc/upgrades.js';
import { evaluate } from '../src/calc/stats.js';

const resource = new Map([
  ['HYPERION', { id: 'HYPERION', name: 'Hyperion', category: 'SWORD', tier: 'LEGENDARY', dungeon_item: true, stats: { DAMAGE: 260, STRENGTH: 150, INTELLIGENCE: 350 } }],
  ['GIANTS_SWORD', { id: 'GIANTS_SWORD', name: "Giant's Sword", category: 'SWORD', tier: 'LEGENDARY', dungeon_item: true, stats: { DAMAGE: 500 } }],
  ['TERMINATOR', { id: 'TERMINATOR', name: 'Terminator', category: 'BOW', tier: 'LEGENDARY', stats: { DAMAGE: 310, STRENGTH: 50, CRITICAL_DAMAGE: 250 } }],
  ['NECRON_HELMET', { id: 'NECRON_HELMET', name: "Necron's Helmet", category: 'HELMET', tier: 'LEGENDARY', dungeon_item: true, stats: { STRENGTH: 40, CRITICAL_DAMAGE: 30, CRITICAL_CHANCE: 3 } }],
  ['WITHER_RELIC', { id: 'WITHER_RELIC', name: 'Wither Relic', category: 'ACCESSORY', tier: 'LEGENDARY', stats: { STRENGTH: 5 } }],
]);

function encode(items) {
  const list = items.map((x) => ({
    id: nbt.short(1), Count: nbt.byte(1),
    tag: nbt.comp({ ExtraAttributes: nbt.comp({
      id: nbt.string(x.id),
      upgrade_level: nbt.int(x.stars ?? 5),
      hot_potato_count: nbt.int(x.hpb ?? 0),
      enchantments: nbt.comp(Object.fromEntries(Object.entries(x.enchants ?? {}).map(([k, v]) => [k, nbt.int(v)]))),
    }) }),
  }));
  const root = nbt.comp({ i: nbt.list(nbt.comp(list)) }, '');
  return zlib.gzipSync(nbt.writeUncompressed(root)).toString('base64');
}

const priceTable = {
  FIRST_MASTER_STAR: 8e6, SECOND_MASTER_STAR: 15e6, THIRD_MASTER_STAR: 25e6, FOURTH_MASTER_STAR: 40e6, FIFTH_MASTER_STAR: 60e6,
  ENCHANTMENT_SHARPNESS_7: 60e6, ENCHANTMENT_SMITE_7: 5e6, ENCHANTMENT_ENDER_SLAYER_7: 30e6,
  HOT_POTATO_BOOK: 80e3, FUMING_POTATO_BOOK: 1.2e6,
  HYPERION: 900e6, TERMINATOR: 1.2e9, WITHER_RELIC: 50e6,
};
const prices = { get: (id) => (priceTable[id] ? { price: priceTable[id], source: 'BZ' } : null) };

test('NBT decode finds ITEM_IDs, stars and enchants', () => {
  const items = decodeItems(encode([{ id: 'HYPERION', stars: 7, hpb: 10, enchants: { sharpness: 6 } }]))
    .map((i) => toItemState(i, resource));
  assert.equal(items[0].id, 'HYPERION');
  assert.equal(items[0].masterStars, 2);
  assert.equal(items[0].hpb, 10);
  assert.equal(items[0].enchants.sharpness, 6);
});

test('only the next master star is offered, and only on owned items', () => {
  const owned = [toItemState(decodeItems(encode([{ id: 'HYPERION', stars: 8 }]))[0], resource)];
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const loadout = buildLoadout(owned, ctx, weightsFor('mage'));
  const labels = availableUpgrades(loadout, ctx, prices).map((u) => u.label);
  assert.ok(labels.includes('4th Master Star on Hyperion'));
  assert.ok(!labels.some((l) => /5th Master Star/.test(l)));
  assert.ok(!labels.some((l) => /Master Star on Necron/.test(l)));
});

test('greedy plan is in dependency order and every step gains damage', () => {
  const ctx = makeContext({ cls: 'mage', cataLevel: 50, resource });
  const weights = weightsFor('mage');
  const loadout = buildLoadout([], ctx, weights);
  const plan = planUpgrades(loadout, ctx, prices, weights, 12);
  const labels = plan.steps.map((s) => s.label);
  const buy = labels.indexOf('Buy Hyperion');
  const star = labels.indexOf('1st Master Star on Hyperion');
  if (star !== -1) assert.ok(buy !== -1 && buy < star);
  for (const s of plan.steps) assert.ok(s.gain > 0 && s.perMillion > 0);
});

test('smite only helps withers, ender slayer only helps dragons', () => {
  const owned = [toItemState(decodeItems(encode([{ id: 'GIANTS_SWORD' }]))[0], resource)];
  const ctx = makeContext({ cls: 'bers', cataLevel: 50, resource });
  const base = buildLoadout(owned, ctx, weightsFor('bers'));
  const ups = availableUpgrades(base, ctx, prices);
  const before = evaluate(base, ctx);
  const smite = evaluate(ups.find((u) => u.label.startsWith('Smite VII')).apply(base), ctx);
  const ender = evaluate(ups.find((u) => u.label.startsWith('Ender Slayer VII')).apply(base), ctx);
  assert.ok(smite.wither > before.wither && smite.dragon === before.dragon);
  assert.ok(ender.dragon > before.dragon && ender.wither === before.wither);
});
