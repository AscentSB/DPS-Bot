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
//   floorMode  only counts in 'master' (Master Mode) or 'normal' (Normal Mode) runs (the /dps floor toggle)
//   swarm      Swarm: additiveDamage is per enemy within 10 blocks (1 in the boss, RCM_SWARM_ENEMIES = 7 in the clear)
//   thunder    Thunderlord: every 3rd hit adds a lightning hit of this % of the hit's damage
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
  // Kill speed enchants. Bosses and clear mobs have far more health than you, so Giant Killer sits at its cap.
  // Titan Killer is for Master Mode (high defense): counted at its cap there, 0 in Normal Mode.
  // Prosecute is counted at full target health: early damage matters most (Lethality speeds up the rest), and RCM
  // one-taps. Execute (missing health) is left out for the same reason. Cubism hits nothing in dungeons.
  // Boss set: Titan Killer in Master Mode (M7), Giant Killer in Normal Mode (F7). The clear set uses Giant Killer on both.
  giant_killer: { categories: SWORDS, target: 'global', floorMode: 'normal', stats: { additiveDamage: [5, 10, 15, 20, 30, 45, 65] }, conflicts: ['titan_killer'] },
  titan_killer: { categories: SWORDS, target: 'global', floorMode: 'master', stats: { additiveDamage: [6, 12, 18, 24, 40, 60, 80] }, conflicts: ['giant_killer'] },
  prosecute: { categories: SWORDS, target: 'global', stats: { additiveDamage: [10, 20, 30, 40, 70, 100] } },
  // First hits on a mob (melee only): Triple-Strike for Master Mode (M7), First Strike for Normal Mode (F7).
  first_strike: { categories: SWORDS, target: 'global', melee: true, floorMode: 'normal', stats: { additiveDamage: [25, 50, 75, 100, 125] }, conflicts: ['triple_strike'] },
  triple_strike: { categories: SWORDS, target: 'global', melee: true, floorMode: 'master', stats: { additiveDamage: [10, 20, 30, 40, 50] }, conflicts: ['first_strike'] },
  // Thunderlord beats Thunderbolt on one target (Thunderbolt splits its lightning), so only Thunderlord is counted.
  // Fire Aspect: burns for a % of the hit every second, withers only (dragons do not burn). Source: wiki (3/6/9%).
  fire_aspect: { categories: SWORDS, target: 'wither', fire: [3, 6, 9], maxLevel: 3 },
  thunderlord: { categories: SWORDS, target: 'global', melee: true, thunder: [8, 16, 24, 32, 40, 50, 60], maxLevel: 7, conflicts: ['thunderbolt'] },
  ultimate_swarm: { name: 'Swarm', categories: [...SWORDS, 'BOW'], target: 'global', swarm: true, stats: { additiveDamage: [2, 4, 6, 8, 10] }, conflicts: ['ultimate_chimera', 'ultimate_reiterate'] },
  // Vicious: +3 / +4 / +5 ferocity at III-V (I and II do not exist; listed as 0). Goes on bows (Terminator) too.
  vicious: { categories: [...SWORDS, 'BOW'], target: 'global', stats: { ferocity: [0, 0, 3, 4, 5] } },
  // Hypixel lets Sharpness and Smite sit on the same sword, so they do not conflict.
  // Sharpness VII and Smite VII give +50% and Smite VI +40% (stats team, 2026-10-30: the mob types release changed
  // them; the wiki's 65, 100 and 80 are out of date). The other levels are the wiki's, unchecked.
  sharpness: { categories: SWORDS, target: 'global', melee: true, stats: { additiveDamage: [5, 10, 15, 20, 30, 45, 50] }, conflicts: ['bane_of_arthropods'] },
  power: { categories: ['BOW'], target: 'global', stats: { additiveDamage: [8, 16, 24, 32, 40, 50, 65] } },
  critical: { categories: SWORDS, target: 'global', stats: { critDamage: [10, 20, 30, 40, 50, 70, 100] } },
  smite: { categories: SWORDS, target: 'wither', clearMobs: true, stats: { additiveDamage: [10, 20, 30, 40, 60, 40, 50] }, conflicts: ['bane_of_arthropods'] },
  ender_slayer: { categories: SWORDS, target: 'dragon', stats: { additiveDamage: [15, 30, 45, 60, 80, 100, 130] } },
  dragon_hunter: { categories: ['BOW'], target: 'dragon', stats: { additiveDamage: [8, 16, 24, 32, 40, 50] } },
  // From the M7 archer sheet: Snipe IV +2% over Snipe III, Overload V +5 crit damage and crit chance.
  snipe: { categories: ['BOW'], target: 'global', stats: { additiveDamage: [1, 2, 3, 5] } },
  overload: { categories: ['BOW'], target: 'global', stats: { critDamage: [1, 2, 3, 4, 5], critChance: [1, 2, 3, 4, 5] } },
  toxophilite: { categories: ['BOW'], target: 'global', stats: { critChance: [10] } },
  // Duplex: +0.04 arrows per shot per level (+0.2 at V). Ultimate enchant, stored as ultimate_reiterate.
  ultimate_reiterate: { name: 'Duplex', categories: ['BOW'], target: 'global', conflicts: ['ultimate_swarm'], stats: { arrows: [0.04, 0.08, 0.12, 0.16, 0.2] } },
  // Sugar Rush: +2 speed per level on boots (helps reach the 500 speed cap).
  sugar_rush: { categories: ['BOOTS'], target: 'global', stats: { speed: [2, 4, 6] } },
  big_brain: { categories: ['HELMET'], target: 'global', stats: { intelligence: [5, 10, 15, 20, 25] } },
  smarty_pants: { categories: ['LEGGINGS'], target: 'global', stats: { intelligence: [5, 10, 15, 20, 25] } },
  // Legion is handled as a percent of combat stats per piece; see LEGION_* in constants.js.
  ultimate_legion: { categories: ARMOR, target: 'global', legion: true, maxLevel: 5 },
  // Chimera copies CHIMERA_PER_LEVEL of your active pet's stats per level onto the sword (constants.js).
  // On a Ragnarock it boosts the ability's strength; on a Mage weapon the copied stats get the dungeon boost.
  ultimate_chimera: { categories: SWORDS, target: 'global', chimera: true, maxLevel: 5, conflicts: ['ultimate_swarm'] },
  // The One (necklace): +0.1 (IV) / +0.2 (V) strength per maxed collection. Only IV and V exist (Riftstalker
  // Bloodfiend, bazaar).
  ultimate_the_one: { name: 'The One', categories: ['NECKLACE'], target: 'global', theOne: [0, 0, 0, 0.1, 0.2], minLevel: 4, maxLevel: 5 },
};

// Enchanting table: XP levels for each level it sells (NEU enchants_xp_cost, up to max_xp_table_levels). A level
// the bazaar does not sell costs 1 Grand Experience Bottle, or 2 when it costs ENCHANT_TABLE_TWO_BOTTLES levels.
export const GRAND_EXP_BOTTLE_ID = 'GRAND_EXP_BOTTLE';
export const ENCHANT_TABLE_TWO_BOTTLES = 75;
export const TABLE_ENCHANT_XP = {
  sharpness: [10, 15, 20, 25, 30],
  smite: [10, 15, 20, 25, 30],
  giant_killer: [10, 20, 30, 40, 50],
  titan_killer: [10, 20, 30, 40, 50],
  critical: [10, 20, 30, 40, 50],
  first_strike: [20, 30, 40, 75],
  triple_strike: [20, 30, 40, 75],
  ender_slayer: [10, 20, 25, 30, 40],
  thunderlord: [20, 25, 30, 40, 50],
  prosecute: [20, 25, 30, 40, 50],
  power: [10, 20, 30, 40, 50],
};

// Bazaar items (Item Upgrades) that raise an enchant from VI to VII once.
export const ENCHANT_UPGRADE_ITEMS = {
  smite: { level: 7, id: 'SEVERED_HAND', name: 'Severed Hand' },
  ender_slayer: { level: 7, id: 'ENDSTONE_IDOL', name: 'End Stone Idol' },
};

// Non-DPS sword enchants: never bought when a blade copies another (spare blade, dual-wield copy).
export const SKIPPED_SWORD_ENCHANTS = ['divine_gift', 'looting', 'luck', 'experience', 'cleave', 'smoldering', 'cubism'];

export function skippedSwordEnchant(key, category) {
  return SWORDS.includes(category) && SKIPPED_SWORD_ENCHANTS.includes(key);
}

export function enchantMaxLevel(ench) {
  if (ench.maxLevel) return ench.maxLevel;
  return Math.max(...Object.values(ench.stats ?? {}).map((v) => v.length), 0);
}
