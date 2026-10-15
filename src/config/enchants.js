// Enchant stat table. Edit or add entries here.
//
//   key        enchant name as it appears in item NBT (ExtraAttributes.enchantments)
//   categories item categories the enchant applies to
//   target     'global', 'wither' or 'dragon'
//   stats      stat values at each level, e.g. { additiveDamage: [5, 10, ...] }. Index 0 = level 1
//   classes    only count for these classes (optional)
//   conflicts  enchants removed when this one is applied (optional)
//   name       label when the NBT key differs from the in-game name (optional)
//   melee      the enchant says "melee damage", so it does not boost abilities (Mage RCM)
//   clearMobs  a boss-only damage enchant that also hits clear mobs (Smite: Skeletons and Zombies), so it
//              boosts Mage RCM in the clear
//
// Sword damage enchants that do not say melee also raise ability damage (Mage RCM), as magic additive.
//
// Flat stats on dungeon items get the dungeon multiplier like every other item stat.
// Books are priced from the bazaar id ENCHANTMENT_<KEY>_<LEVEL>, with the auction
// house as a fallback. Every level above the current one is offered as its own upgrade.

const ARMOR = ['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'];
const SWORDS = ['SWORD', 'LONGSWORD'];

export const ENCHANTS = {
  sharpness: { categories: SWORDS, target: 'global', melee: true, stats: { additiveDamage: [5, 10, 15, 20, 30, 45, 65] }, conflicts: ['smite', 'bane_of_arthropods'] },
  power: { categories: ['BOW'], target: 'global', stats: { additiveDamage: [8, 16, 24, 32, 40, 50, 65] } },
  critical: { categories: SWORDS, target: 'global', stats: { critDamage: [10, 20, 30, 40, 50, 70, 100] } },
  smite: { categories: SWORDS, target: 'wither', clearMobs: true, stats: { additiveDamage: [10, 20, 30, 40, 60, 80, 100] }, conflicts: ['sharpness', 'bane_of_arthropods'] },
  ender_slayer: { categories: SWORDS, target: 'dragon', stats: { additiveDamage: [15, 30, 45, 60, 80, 100, 130] } },
  dragon_hunter: { categories: ['BOW'], target: 'dragon', stats: { additiveDamage: [8, 16, 24, 32, 40, 50] } },
  // From the M7 archer sheet: Snipe IV +2% over Snipe III, Overload V +5 crit damage and crit chance.
  snipe: { categories: ['BOW'], target: 'global', stats: { additiveDamage: [1, 2, 3, 5] } },
  overload: { categories: ['BOW'], target: 'global', stats: { critDamage: [1, 2, 3, 4, 5], critChance: [1, 2, 3, 4, 5] } },
  toxophilite: { categories: ['BOW'], target: 'global', stats: { critChance: [10] } },
  // Duplex: +0.04 arrows per shot per level (+0.2 at V). Ultimate enchant, stored as ultimate_reiterate.
  ultimate_reiterate: { name: 'Duplex', categories: ['BOW'], target: 'global', stats: { arrows: [0.04, 0.08, 0.12, 0.16, 0.2] } },
  // Sugar Rush: +2 speed per level on boots (helps reach the 500 speed cap).
  sugar_rush: { categories: ['BOOTS'], target: 'global', stats: { speed: [2, 4, 6] } },
  big_brain: { categories: ['HELMET'], target: 'global', stats: { intelligence: [5, 10, 15, 20, 25] } },
  smarty_pants: { categories: ['LEGGINGS'], target: 'global', stats: { intelligence: [5, 10, 15, 20, 25] } },
  // Legion is handled as a percent of combat stats per piece; see LEGION_* in constants.js.
  ultimate_legion: { categories: ARMOR, target: 'global', legion: true, maxLevel: 5 },
  // Chimera copies CHIMERA_PER_LEVEL of your active pet's stats per level onto the sword (constants.js).
  // On a Ragnarock it boosts the ability's strength; on a Mage weapon the copied stats get the dungeon boost.
  ultimate_chimera: { categories: SWORDS, target: 'global', chimera: true, maxLevel: 5 },
};

export function enchantMaxLevel(ench) {
  if (ench.maxLevel) return ench.maxLevel;
  return Math.max(...Object.values(ench.stats ?? {}).map((v) => v.length), 0);
}
