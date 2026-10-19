// Loads the JS bot's planner modules (from JS_SRC, default /app/src) and sets up one case the way runDps in
// src/calc/run.js does. Shared by evals.mjs and the other oracle scripts.
import { SRC, freezeNow, loadSnapshot } from './common.mjs';

let bot = null;
// Freezes Date.now at snapshot.now first, so modules and readMember see the fixed time.
export async function loadBot() {
  if (bot) return bot;
  const snap = loadSnapshot();
  freezeNow(snap.now);
  const upgrades = await import(`${SRC}/calc/upgrades.js`);
  const stats = await import(`${SRC}/calc/stats.js`);
  const { priceLookup } = await import(`${SRC}/api/prices.js`);
  const { readMember } = await import(`${SRC}/api/hypixel.js`);
  const { runDps } = await import(`${SRC}/calc/run.js`);
  // The frozen price tables of one plan, shaped like snapshotPrices() / the plan worker's prices.
  const prices = { ...priceLookup(snap.tables), tables: snap.tables };
  bot = { snap, upgrades, stats, prices, readMember, runDps };
  return bot;
}

// ctx, weights and the start loadout for a job (model::DpsJob) on a member (readMember output), exactly as runDps
// builds them, plus ctx.weights = weights (planUpgrades sets it before the first availableUpgrades call).
export function prepareCase(b, member, job) {
  const { makeContext, weightsFor, buildLoadout } = b.upgrades;
  const { cls, target = null, upgradeMode = 'all', mode = 'lcm', floorMode = 'master', blessings, anyChest = false, noClearBoots = false } = job;
  const ctx = makeContext({
    cls, cataLevel: member.cataLevel, secrets: member.secrets, power: member.power, tuning: member.tuning,
    profileSpeed: member.profileSpeed, petProfile: member.petProfile, archerLevel: member.archerLevel,
    berserkLevel: member.berserkLevel, mageLevel: member.mageLevel, combatLevel: member.combatLevel,
    skills: member.skills, slayers: member.slayers, collections: member.collections, profileStats: member.profileStats, upgradeMode, mode, floorMode, blessings, anyChest, noClearBoots, resource: b.snap.resource,
  });
  const weights = weightsFor(cls, target, mode, ctx.floorMode);
  const loadout = buildLoadout(member, ctx, weights);
  ctx.weights = weights;
  return { ctx, weights, loadout };
}
