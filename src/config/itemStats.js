// ============================================================================
//  ADD YOUR ITEM IDS AND STATS HERE
// ============================================================================
//
// Base stats come from https://api.hypixel.net/v2/resources/skyblock/items.
// Use this file to add stats the API does not list, or to override them.
// Look items up by their ITEM_ID (for example HYPERION, TERMINATOR, NECRON_HELMET).
//
// Each entry:
//   ITEM_ID: {
//     replaceStats: false,   // true = ignore API stats and use only `stats` below
//     dungeon: true,         // force dungeon scaling on or off (default: API "dungeon_item" flag)
//     stats: { ... },        // applies against every target
//     wither: { ... },       // applies only against withers
//     dragon: { ... },       // applies only against dragons
//   }
//
// Stat keys you can use:
//   damage, strength, critDamage, critChance, intelligence  flat stats (scaled on dungeon items)
//   additiveDamage                                         percent, e.g. 50 = +50% damage
//   additiveStrength, additiveStrength2                    fraction, e.g. 0.1 = +10% strength
//   multStrength                                           multiplier, e.g. 1.2
//   additiveCritDamage, additiveCritDamage2, multCritDamage
//   additiveIntelligence, additiveIntelligence2, multIntelligence
//
// Fraction and multiplier stats are never scaled by dungeon stars.
// ============================================================================

export const ITEM_STATS = {
  // Example: a sword that adds +25% damage against dragons only.
  // EXAMPLE_SWORD: { dragon: { additiveDamage: 25 } },

  // Example: an accessory that adds 2% strength as a second additive source.
  // EXAMPLE_TALISMAN: { stats: { additiveStrength2: 0.02 } },
};

// ============================================================================
//  MISC BUFFS (pets, potions, powers, etc.)
// ============================================================================
// Always active for the given class. Same keys as above, split by target.
export const MISC_BUFFS = {
  arch: { stats: {}, wither: {}, dragon: {} },
  bers: { stats: {}, wither: {}, dragon: {} },
  mage: { stats: {}, wither: {}, dragon: {} },
};

// ============================================================================
//  REFORGES
// ============================================================================
// Stats a reforge gives, keyed by the item's "modifier" NBT value and rarity.
// Example: fabled: { LEGENDARY: { strength: 30, critDamage: 35 } }
export const REFORGES = {};
