import test from 'node:test';
import assert from 'node:assert/strict';
import {
  calcStrength, calcIntelligence, cataLevelFromXp, cataBoost, dungeonStat, dungeonCritChance,
  applySoftCap, archerDamage, berserkerDamage, mageDamage,
} from '../src/calc/formulas.js';

const close = (a, b) => assert.ok(Math.abs(a - b) < 1e-6, `${a} != ${b}`);

test('strength and intelligence formulas', () => {
  close(calcStrength(100, 0.1, 0.05, 1.2), 176 * 1.15 * 1.2 * (1.1 * 1.1 * 1.2 * 0.38));
  close(calcIntelligence(500), 548 * (1.1 * 1.1 * 1.2 * 0.24));
});

test('catacombs level and boost', () => {
  assert.equal(cataLevelFromXp(0), 0);
  assert.equal(cataLevelFromXp(50), 1);
  assert.equal(cataLevelFromXp(569_809_640), 50);
  assert.equal(cataLevelFromXp(2e9), 50);
  close(cataBoost(50) + 1.5, 6.25);
});

test('dungeon stat scaling', () => {
  close(dungeonStat(100, 0, 50, 0), 625);
  close(dungeonStat(100, 2, 50, 3), 100 * (6.25 + 10 + 0.03));
  close(dungeonCritChance(10, 5, 9), 10 * (1.5 + 0.25 + 0.05));
});

test('soft cap keeps 10% above 1M', () => {
  assert.equal(applySoftCap(500_000), 500_000);
  close(applySoftCap(3_000_000), 1_200_000);
});

test('berserker adds 1200 additive damage on top of archer formula', () => {
  const s = { damage: 10, critDamage: 0, strength: 0, additiveDamage: 0, critChance: 100 };
  close(berserkerDamage(s, 0), archerDamage(s, 0) * 13);
});

test('hyperion multiplies mage wither damage', () => {
  const s = { damage: 10, critDamage: 0, strength: 0, additiveDamage: 0, intelligence: 100 };
  close(mageDamage(s, 0, true), mageDamage(s, 0, false) * 1.5);
});
