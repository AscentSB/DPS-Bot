import test from 'node:test';
import assert from 'node:assert/strict';
import {
  calcStrength, calcCritDamage, calcCritChance, calcIntelligence, cataLevelFromXp, cataBoost,
  dungeonMultiplier, dungeonCritMultiplier, expectedHit, archerDamage, berserkerDamage,
  mageDamage, rcmDamage, powerMultiplier, secretDigits, applyBlessings, blessingEffect,
} from '../src/calc/formulas.js';

const close = (a, b, eps = 1e-6) => assert.ok(Math.abs(a - b) <= eps * Math.max(1, Math.abs(b)), `${a} != ${b}`);

test('blessings match the wiki worked example (Power 29 + Time 5, Benediction, Forbidden Blessing X)', () => {
  const effect = 1.1 * 1.25 * 1.2;
  close(effect, 1.65);
  const b = applyBlessings(6000, 'strength', effect, { power: { level: 29, stats: ['strength'] }, time: { level: 5, stats: ['strength'] } });
  close(b.flat * b.pct, 14191.04, 1e-6);
});

test('default blessing effect is the floor buff only; Forbidden Blessing and Mimic multiply it', () => {
  close(blessingEffect(), 1.2);
  close(blessingEffect(0.1, 0.1), 1.1 * 1.1 * 1.2);
  // Strength: Power 29 and Time 5 (defaults), flat and percent both scaled by the effect.
  const e = blessingEffect(0.1, 0.1);
  const expected = (1000 + 4 * 29 * e + 4 * 5 * e) * 1.1 * 1.2 * (1 + 0.02 * 29 * e) * (1 + 0.02 * 5 * e);
  close(calcStrength(1000, 0.1, 0, 1.2, e), expected);
  close(calcCritDamage(1000, 0, 0, 1, e), (1000 + 4 * 29 * e) * (1 + 0.02 * 29 * e));
  close(calcIntelligence(500, 0, 0, 1, e), (500 + 4 * 14 * e + 4 * 5 * e) * (1 + 0.02 * 14 * e) * (1 + 0.02 * 5 * e));
  // Crit chance from the sheet: flat 424.34 (+30 base), add 0.056, Terminator 0.25 -> 119.95
  close(calcCritChance(424.3383228, 0.056, 0.25), 119.9453172);
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


test('berserker adds its extra additive damage on top of the archer formula; archer takes a class multiplier', () => {
  const s = { damage: 10, critDamage: 0, strength: 0, additiveDamage: 0, critChance: 100 };
  close(berserkerDamage(s, 0, 115), archerDamage(s, 0) * 2.15);
  close(archerDamage(s, 0, 1.8), archerDamage(s, 0) * 1.8);
});

test('mage beam multipliers scale the beam', () => {
  const s = { damage: 10, critDamage: 0, strength: 0, additiveDamage: 0, intelligence: 100 };
  close(mageDamage(s, 0, 1.5), mageDamage(s, 0) * 1.5);
});

test('RCM Wither Impact: 10,000 x cata boost x intelligence (0.3 scaling) x ability damage x magic additive', () => {
  const s = { intelligence: 1000, abilityDamage: 50 };
  // 10000 x 1 x (1 + 3) x 1.5 = 60,000.
  close(rcmDamage(s, 0, 0), 60000);
  close(rcmDamage(s, 0, 0, 100), 120000);
  close(rcmDamage(s, 0, 0, 0, 1.05), 63000);
  // Cata boost 4.85: 60,000 x 5.85 = 351,000. No soft cap: big casts stay whole.
  close(rcmDamage(s, 0, 4.85), 351000);
  close(rcmDamage({ intelligence: 20000, abilityDamage: 0 }, 0, 4.85), 10000 * 5.85 * 61);
});

test('mage gets no overload: crit chance above 100% adds nothing to the beam', () => {
  const s = { damage: 100, critDamage: 500, strength: 1000, additiveDamage: 50, intelligence: 5000 };
  close(mageDamage({ ...s, critChance: 150 }, 0), mageDamage({ ...s, critChance: 100 }, 0));
  close(mageDamage({ ...s, critChance: 250 }, 0), mageDamage({ ...s, critChance: 100 }, 0));
  assert.ok(mageDamage({ ...s, critChance: 100 }, 0) > mageDamage({ ...s, critChance: 90 }, 0));
  // Archer still overloads.
  assert.ok(archerDamage({ ...s, critChance: 150 }, 0) > archerDamage({ ...s, critChance: 100 }, 0));
});

test('blessing levels can be chosen within their limits; anything else keeps the default', async () => {
  const { blessingsWith } = await import('../src/config/constants.js');
  const b = blessingsWith({ power: 10, wisdom: 3, time: 0 });
  assert.deepEqual([b.power.level, b.wisdom.level, b.time.level], [10, 3, 0]);
  const bad = blessingsWith({ power: 4, wisdom: 15, time: 3 });
  assert.deepEqual([bad.power.level, bad.wisdom.level, bad.time.level], [29, 14, 5]);
  close(calcStrength(1000, 0, 0, 1, 1.2, b), (1000 + 4 * 10 * 1.2) * (1 + 0.02 * 10 * 1.2));
});
