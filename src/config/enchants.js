// Enchant stat table. Edit or add entries here.
//
//   key        enchant name as it appears in item NBT (ExtraAttributes.enchantments)
//   categories item categories the enchant applies to
//   target     'global', 'wither' or 'dragon'
//   stat       stat key (see itemStats.js for the list)
//   values     stat value at each level. values[0] = level 1
//   conflicts  enchants removed when this one is applied (optional)
//
// Books are priced from the bazaar id ENCHANTMENT_<KEY>_<LEVEL>, with the auction
// house as a fallback. Every level above the current one is offered as its own upgrade.

export const ENCHANTS = {
  sharpness: { categories: ['SWORD', 'LONGSWORD'], target: 'global', stat: 'additiveDamage', values: [5, 10, 15, 20, 30, 45, 65], conflicts: ['smite', 'bane_of_arthropods'] },
  power: { categories: ['BOW'], target: 'global', stat: 'additiveDamage', values: [8, 16, 24, 32, 40, 50, 65] },
  critical: { categories: ['SWORD', 'LONGSWORD'], target: 'global', stat: 'critDamage', values: [10, 20, 30, 40, 50, 70, 100] },
  smite: { categories: ['SWORD', 'LONGSWORD'], target: 'wither', stat: 'additiveDamage', values: [10, 20, 30, 40, 60, 80, 100], conflicts: ['sharpness', 'bane_of_arthropods'] },
  ender_slayer: { categories: ['SWORD', 'LONGSWORD'], target: 'dragon', stat: 'additiveDamage', values: [15, 30, 45, 60, 80, 100, 130] },
  dragon_hunter: { categories: ['BOW'], target: 'dragon', stat: 'additiveDamage', values: [8, 16, 24, 32, 40] },
  big_brain: { categories: ['HELMET'], target: 'global', stat: 'intelligence', values: [5, 10, 15, 20, 25] },
  smarty_pants: { categories: ['LEGGINGS'], target: 'global', stat: 'intelligence', values: [5, 10, 15, 20, 25] },
};
