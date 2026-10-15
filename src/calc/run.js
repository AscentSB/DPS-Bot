import { makeContext, buildLoadout, planUpgrades, weightsFor, profileUpgrades } from './upgrades.js';

// One /dps run from a read profile (readMember output). Returns plain data only, so it can cross a worker
// thread: { weights, menu, plan?: { steps, startDamage, endDamage }, tuningSplit?, profile?: { rows, damage } }.
// pool (optional): evaluation workers that score the planner's candidates in parallel.
// isCancelled (optional): returns true once the plan should stop (checked once per planner step).
export async function runDps({ member, cls, target, upgradeMode = 'all', mode = 'lcm', floorMode = 'master', menu = 'coins', limit, resource, prices, pool, isCancelled }) {
  const ctx = makeContext({
    cls, cataLevel: member.cataLevel, secrets: member.secrets, power: member.power, tuning: member.tuning,
    profileSpeed: member.profileSpeed, petProfile: member.petProfile, archerLevel: member.archerLevel,
    berserkLevel: member.berserkLevel, mageLevel: member.mageLevel, combatLevel: member.combatLevel,
    skills: member.skills, slayers: member.slayers, profileStats: member.profileStats, upgradeMode, mode, floorMode, resource,
  });
  const weights = weightsFor(cls, target, mode, ctx.floorMode);
  const loadout = buildLoadout(member, ctx, weights);
  // Stats shown with the damage (after tuning), per target: the dragon pet and dragon set can change them.
  const STAT_KEYS = ['strength', 'critDamage', 'critChance', 'intelligence', 'abilityDamage'];
  const stats = (d) => Object.fromEntries(['wither', 'dragon'].map((t) => [t, {
    ...Object.fromEntries(STAT_KEYS.map((k) => [k, d.stats?.[t]?.[k] ?? 0])), attackSpeed: d.attackSpeeds?.[t] ?? 0, speed: d.speed ?? 0,
  }]));
  const damage = (d) => ({ wither: d.wither, dragon: d.dragon, ap: d.ap, tuning: d.tuning, stats: stats(d) });
  if (menu === 'profile') {
    const { rows, damage: now } = profileUpgrades(loadout, ctx, weights);
    return { weights, menu, floorMode: ctx.floorMode, profile: { rows, damage: damage(now) } };
  }
  const plan = await planUpgrades(loadout, ctx, prices, weights, limit, pool && prices.tables ? pool.scorer(ctx, weights, prices.tables) : null, isCancelled);
  return {
    weights, menu, floorMode: ctx.floorMode, tuningSplit: ctx.tuning?.split ?? null,
    plan: {
      steps: plan.steps.map(({ label, cost, source, unverified, gain, perMillion, witherGain, dragonGain }) => ({ label, cost, source, unverified, gain, perMillion, witherGain, dragonGain })),
      startDamage: damage(plan.startDamage),
      endDamage: damage(plan.endDamage),
    },
  };
}
