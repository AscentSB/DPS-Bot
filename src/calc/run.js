import { makeContext, buildLoadout, planUpgrades, weightsFor, profileUpgrades } from './upgrades.js';

// One /dps run from a read profile (readMember output). Returns plain data only, so it can cross a worker
// thread: { weights, menu, plan?: { steps, startDamage, endDamage }, tuningSplit?, profile?: { rows, damage } }.
export function runDps({ member, cls, target, upgradeMode = 'all', mode = 'lcm', floorMode = 'master', menu = 'coins', resource, prices }) {
  const ctx = makeContext({
    cls, cataLevel: member.cataLevel, secrets: member.secrets, power: member.power, tuning: member.tuning,
    profileSpeed: member.profileSpeed, petProfile: member.petProfile, archerLevel: member.archerLevel,
    berserkLevel: member.berserkLevel, mageLevel: member.mageLevel, combatLevel: member.combatLevel,
    skills: member.skills, slayers: member.slayers, profileStats: member.profileStats, upgradeMode, mode, floorMode, resource,
  });
  const weights = weightsFor(cls, target, mode, ctx.floorMode);
  const loadout = buildLoadout(member, ctx, weights);
  const damage = (d) => ({ wither: d.wither, dragon: d.dragon, ap: d.ap, tuning: d.tuning });
  if (menu === 'profile') {
    const { rows, damage: now } = profileUpgrades(loadout, ctx, weights);
    return { weights, menu, floorMode: ctx.floorMode, profile: { rows, damage: damage(now) } };
  }
  const plan = planUpgrades(loadout, ctx, prices, weights);
  return {
    weights, menu, floorMode: ctx.floorMode, tuningSplit: ctx.tuning?.split ?? null,
    plan: {
      steps: plan.steps.map(({ label, cost, source, unverified, gain, perMillion }) => ({ label, cost, source, unverified, gain, perMillion })),
      startDamage: damage(plan.startDamage),
      endDamage: damage(plan.endDamage),
    },
  };
}
