import test from 'node:test';
import assert from 'node:assert/strict';
import {
  calcStrength, calcCritDamage, calcCritChance, calcIntelligence, cataLevelFromXp, cataBoost,
  dungeonMultiplier, dungeonCritMultiplier, applySoftCap, expectedHit, archerDamage, berserkerDamage,
  mageDamage, powerMultiplier, secretDigits,
} from '../src/calc/formulas.js';

const close = (a, b, eps = 1e-6) => assert.ok(Math.abs(a - b) <= eps * Math.max(1, Math.abs(b)), `${a} != ${b}`);

test('final stat formulas match the M7 archer sheet totals', () => {
  // Sheet row 2: flat STR 5459.19, add STR 0.116, multi STR 1.1 -> 10544.21
  close(calcStrength(5459.192581, 0.116, 0, 1.1), 10544.21294);
  // flat CD 4996.49, add CD 0.066 -> 8390.80
  close(calcCritDamage(4996.492581, 0.066), 8390.796299);
  // flat CC 424.34 (+30 base), add CC 0.056, multi CC 0.25 (Terminator) -> 119.95
  close(calcCritChance(424.3383228, 0.056, 0.25), 119.9453172);
  close(calcIntelligence(500), 548 * (1 + 1.1 * 1.1 * 1.2 * 0.24));
});

test('expected hit matches the sheet output, including overload', () => {
  const s = { damage: 2233, strength: 10544.21294, critDamage: 8390.796299, additiveDamage: 807, critChance: 119.9453172 };
  close(expectedHit(s, 0), 201299506.7, 1e-8);
  // Below 100% crit chance: plain expected value.
  const low = { ...s, critChance: 50 };
  const nonCrit = 2233 * (1 + 105.4421294) * (1 + 8.07);
  close(expectedHit(low, 0), 0.5 * nonCrit * (1 + 83.90796299) + 0.5 * nonCrit);
  // At 200%+ every hit overloads.
  close(expectedHit({ ...s, critChance: 250 }, 0), 1.5 * nonCrit * (1 + 83.90796299));
});

test('catacombs level, boost and dungeon multiplier', () => {
  assert.equal(cataLevelFromXp(0), 0);
  assert.equal(cataLevelFromXp(50), 1);
  assert.equal(cataLevelFromXp(569_809_640), 50);
  close(cataBoost(0), 0.10);
  close(cataBoost(21), 1.95);
  close(cataBoost(60), 4.85);
  close(dungeonMultiplier(50), 6.35);
  close(dungeonMultiplier(50, 5, 2, 3), 6.35 + 0.1 + 0.03);
  close(dungeonMultiplier(50, 3), 1 + 4.85 + 0.3);
  close(dungeonCritMultiplier(5, 5, 9), 1.5 + 0.25 + 0.05);
});

test('general medallion secret digits', () => {
  assert.equal(secretDigits(0), 0);
  assert.equal(secretDigits(1234), 4);
  assert.equal(secretDigits(123456), 5);
});

test('accessory power multiplier matches the wiki (Silky at 250 AP is about 220 crit damage)', () => {
  close(powerMultiplier(250), 29.97 * Math.log(0.0019 * 250 + 1) ** 1.2);
  assert.ok(Math.abs(22.8 * powerMultiplier(250) - 220) < 2);
  assert.equal(powerMultiplier(0), 0);
});

test('soft cap keeps 10% above 1M', () => {
  assert.equal(applySoftCap(500_000), 500_000);
  close(applySoftCap(3_000_000), 1_200_000);
});

test('berserker adds 1200 additive damage on top of the archer formula', () => {
  const s = { damage: 10, critDamage: 0, strength: 0, additiveDamage: 0, critChance: 100 };
  close(berserkerDamage(s, 0), archerDamage(s, 0) * 13);
});

test('hyperion multiplies mage wither damage', () => {
  const s = { damage: 10, critDamage: 0, strength: 0, additiveDamage: 0, intelligence: 100 };
  close(mageDamage(s, 0, true), mageDamage(s, 0, false) * 1.5);
});
