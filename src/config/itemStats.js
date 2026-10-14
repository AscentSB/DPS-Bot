// ============================================================================
//  ADD YOUR ITEM IDS AND STATS HERE
// ============================================================================
//
// Base stats come from https://api.hypixel.net/v2/resources/skyblock/items.
// Use this file to add stats the API does not list, or to override them.
// Look items up by their ITEM_ID (for example HYPERION, TERMINATOR, POWER_WITHER_HELMET).
//
// Each entry:
//   ITEM_ID: {
//     replaceStats: false,   // true = ignore API stats and use only `stats` below
//     dungeon: true,         // force dungeon scaling on or off (default: API flags and item NBT)
//     stats: { ... },        // applies against every target
//     wither: { ... },       // applies only against withers
//     dragon: { ... },       // applies only against dragons
//     classes: ['arch'],     // only count these stats for these classes (optional)
//     arrows: 3,             // shortbows: arrows per shot (DPS = damage per arrow x arrows x shots per second)
//     shotTicks: 10,         // shortbows: base ticks between shots (attack speed lowers it)
//     statMultiplier: 2,     // multiplies every stat on the item, reforge and gems included (Necron heads on Floor 7)
//   }
//
// Stat keys you can use:
//   damage, strength, critDamage, critChance, intelligence  flat stats (scaled on dungeon items)
//   additiveDamage                                         percent, e.g. 50 = +50% damage
//   additiveStrength, additiveStrength2                    fraction, e.g. 0.1 = +10% strength
//   multStrength                                           multiplier, e.g. 1.2
//   additiveCritDamage, additiveCritDamage2, multCritDamage
//   additiveCritChance, multCritChance
//   additiveIntelligence, additiveIntelligence2, multIntelligence
//
// Fraction and multiplier stats are never scaled by dungeon stars.
// Shards, pets, ability items and perks live in src/config/upgrades.js.
// ============================================================================

export const ITEM_STATS = {
  // Terminator: 3 arrows per shot every 10 ticks (4 shots/s at 100 attack speed); crit chance is quartered.
  TERMINATOR: { stats: { multCritChance: 0.25 }, arrows: 3, shotTicks: 10 },
  JUJU_SHORTBOW: { arrows: 1, shotTicks: 10 },
  ITEM_SPIRIT_BOW: { arrows: 1, shotTicks: 15 },
  STARRED_ITEM_SPIRIT_BOW: { arrows: 1, shotTicks: 14 },

  // Necron heads: 2x stats on The Catacombs Floor VII.
  DIAMOND_NECRON_HEAD: { statMultiplier: 2 },
  GOLD_NECRON_HEAD: { statMultiplier: 2 },

  // Master Skulls boost strength in Master Mode. Only your highest tier counts.
  MASTER_SKULL_TIER_1: { stats: { multStrength: 1.01 } },
  MASTER_SKULL_TIER_2: { stats: { multStrength: 1.02 } },
  MASTER_SKULL_TIER_3: { stats: { multStrength: 1.03 } },
  MASTER_SKULL_TIER_4: { stats: { multStrength: 1.04 } },
  MASTER_SKULL_TIER_5: { stats: { multStrength: 1.06 } },
  MASTER_SKULL_TIER_6: { stats: { multStrength: 1.08 } },
  MASTER_SKULL_TIER_7: { stats: { multStrength: 1.1 } },

  // Example: a sword that adds +25% damage against dragons only.
  // EXAMPLE_SWORD: { dragon: { additiveDamage: 25 } },
};

// ============================================================================
//  MISC BUFFS (potions, mayor perks, anything always active)
// ============================================================================
// Always active for the given class. Same keys as above, split by target.
// A Tier V Beacon powered by Scorched Power Crystals (+20%) is assumed: crit chance 5 x 1.2 = 6 for Archer and
// Berserker, ability damage 5% x 1.2 = 6% for Mage. Source: https://hypixelskyblock.minecraft.wiki/w/Beacon
const BEACON_SCORCHED = 1.2;
export const MISC_BUFFS = {
  arch: { stats: { critChance: 5 * BEACON_SCORCHED }, wither: {}, dragon: {} },
  bers: { stats: { critChance: 5 * BEACON_SCORCHED }, wither: {}, dragon: {} },
  mage: { stats: { abilityDamage: 5 * BEACON_SCORCHED }, wither: {}, dragon: {} },
};

// ============================================================================
//  REFORGE OVERRIDES
// ============================================================================
// Reforge stats load from src/data/reforges.json (NEU repo, `npm run update-data`).
// Add or override one here, keyed by lowercase name with letters only.
// Example: fabled: { stone: 'DRAGON_CLAW', types: ['SWORD'], stats: { LEGENDARY: { strength: 60, critDamage: 40 } } }
export const REFORGE_OVERRIDES = {};
