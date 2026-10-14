// Enchant stat table. Edit or add entries here.
//
//   key        enchant name as it appears in item NBT (ExtraAttributes.enchantments)
//   categories item categories the enchant applies to
//   target     'global', 'wither' or 'dragon'
//   stats      stat values at each level, e.g. { additiveDamage: [5, 10, ...] }. Index 0 = level 1
//   classes    only count for these classes (optional)
//   conflicts  enchants removed when this one is applied (optional)
//
// Flat stats on dungeon items get the dungeon multiplier like every other item stat.
// Books are priced from the bazaar id ENCHANTMENT_<KEY>_<LEVEL>, with the auction
// house as a fallback. Every level above the current one is offered as its own upgrade.

const ARMOR = ['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'];
const SWORDS = ['SWORD', 'LONGSWORD'];

export const ENCHANTS = {
  sharpness: { categories: SWORDS, target: 'global', stats: { additiveDamage: [5, 10, 15, 20, 30, 45, 65] }, conflicts: ['smite', 'bane_of_arthropods'] },
  power: { categories: ['BOW'], target: 'global', stats: { additiveDamage: [8, 16, 24, 32, 40, 50, 65] } },
  critical: { categories: SWORDS, target: 'global', stats: { critDamage: [10, 20, 30, 40, 50, 70, 100] } },
  smite: { categories: SWORDS, target: 'wither', stats: { additiveDamage: [10, 20, 30, 40, 60, 80, 100] }, conflicts: ['sharpness', 'bane_of_arthropods'] },
  ender_slayer: { categories: SWORDS, target: 'dragon', stats: { additiveDamage: [15, 30, 45, 60, 80, 100, 130] } },
  dragon_hunter: { categories: ['BOW'], target: 'dragon', stats: { additiveDamage: [8, 16, 24, 32, 40, 50] } },
  // From the M7 archer sheet: Snipe IV +2% over Snipe III, Overload V +5 crit damage and crit chance.
  snipe: { categories: ['BOW'], target: 'global', stats: { additiveDamage: [1, 2, 3, 5] } },
  overload: { categories: ['BOW'], target: 'global', stats: { critDamage: [1, 2, 3, 4, 5], critChance: [1, 2, 3, 4, 5] } },
  toxophilite: { categories: ['BOW'], target: 'global', stats: { critChance: [10] } },
  big_brain: { categories: ['HELMET'], target: 'global', stats: { intelligence: [5, 10, 15, 20, 25] } },
  smarty_pants: { categories: ['LEGGINGS'], target: 'global', stats: { intelligence: [5, 10, 15, 20, 25] } },
  // Legion is handled as a percent of combat stats per piece; see LEGION_* in constants.js.
  ultimate_legion: { categories: ARMOR, target: 'global', legion: true, maxLevel: 5 },
  // Chimera copies part of your pet's stats onto a Ragnarock; see CHIMERA_PER_LEVEL in constants.js.
  ultimate_chimera: { categories: SWORDS, target: 'global', chimera: true, maxLevel: 5, items: ['RAGNAROCK_AXE'] },
};

export function enchantMaxLevel(ench) {
  if (ench.maxLevel) return ench.maxLevel;
  return Math.max(...Object.values(ench.stats ?? {}).map((v) => v.length), 0);
}
