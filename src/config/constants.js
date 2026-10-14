// Tunable constants for the DPS model. Every number the formulas use lives here.

export const CLASSES = {
  arch: { label: 'Archer', weights: { wither: 50, dragon: 50 }, baseAdditiveDamage: 0 },
  bers: { label: 'Berserker', weights: { wither: 20, dragon: 80 }, baseAdditiveDamage: 1200 },
  mage: { label: 'Mage', weights: { wither: 80, dragon: 20 }, baseAdditiveDamage: 0 },
};

// A global upgrade counts for wither weight + dragon weight = 100.
export const GLOBAL_WEIGHT = 100;

// Enemy defense used in (1 - defense / (defense + 100)). Edit to match the bosses you fight.
export const TARGET_DEFENSE = {
  wither: 0,
  dragon: 0,
};

// damage = BASE_DAMAGE + weapon_damage
export const BASE_DAMAGE = 5;

// Flat values added before the multipliers: (stat + base)(1 + add1 + add2)(mult)(constant)
export const BASE_STRENGTH = 76;
export const BASE_CRIT_DAMAGE = 76;
export const BASE_INTELLIGENCE = 48;

export const STRENGTH_CONSTANT = 1.1 * 1.1 * 1.2 * 0.38;
export const CRIT_DAMAGE_CONSTANT = 1.1 * 1.1 * 1.2 * 0.38;
export const INTELLIGENCE_CONSTANT = 1.1 * 1.1 * 1.2 * 0.24;

// Base crit chance before gear. Crit chance is capped in the damage formula.
export const BASE_CRIT_CHANCE = 30;
export const CRIT_CHANCE_CAP = 100;

// Mage formula: .3 * (.09 + intelligence / MAGE_INT_DIVISOR)
export const MAGE_INT_DIVISOR = 100;
export const MAGE_ABILITY_SCALING = 0.3;
export const MAGE_ABILITY_BASE = 0.09;

// Hyperion: damage multiplier against withers for mage.
export const HYPERION_ID = 'HYPERION';
export const HYPERION_WITHER_MULTIPLIER = 1.5;

// Soft cap: everything above SOFT_CAP keeps SOFT_CAP_KEEP of its value (90% reduction).
export const SOFT_CAP = 1_000_000;
export const SOFT_CAP_KEEP = 0.1;

// Dungeon item stats:
//   stat       * (cataBoost + DUNGEON_BASE_ADD + MASTER_STAR_STEP * masterStars + SECRET_STEP * secretDigit)
//   critChance * (CRIT_BASE + CRIT_MASTER_STAR_STEP * masterStars + SECRET_STEP * min(secretDigit, SECRET_DIGIT_CAP))
// cataBoost comes from the player's Catacombs level (see CATA_STAT_BONUS below).
export const DUNGEON_BASE_ADD = 1.5;
export const MASTER_STAR_STEP = 5; // as specified. Change to 0.05 if master stars should give +5% each.
export const CRIT_BASE = 1.5;
export const CRIT_MASTER_STAR_STEP = 0.05;
export const SECRET_STEP = 0.01;
export const SECRET_DIGIT_CAP = 5;
export const DEFAULT_SECRET_DIGIT = 0;
export const MAX_MASTER_STARS = 5;

// Catacombs dungeon item stat bonus in percent, cumulative. Index 0 = level 1.
// Copied from the "Stats Increment" column of the Catacombs/Levels table on
// https://hypixelskyblock.minecraft.wiki/w/Catacombs/Levels
// Paste new values here if Hypixel changes them. cataBoost = value / 100.
export const CATA_STAT_BONUS = [
  4, 8, 12, 16, 20, 25, 30, 35, 40, 45,
  51, 57, 63, 69, 75, 82, 89, 96, 103, 110,
  118, 126, 134, 142, 150, 159, 168, 177, 186, 195,
  205, 215, 225, 235, 245, 257, 269, 281, 293, 305,
  319, 333, 347, 361, 375, 391, 408, 426, 445, 465,
];

// XP needed for each Catacombs level (not cumulative). Index 0 = level 1.
export const CATA_XP_PER_LEVEL = [
  50, 75, 110, 160, 230, 330, 470, 670, 950, 1340,
  1890, 2665, 3760, 5260, 7380, 10300, 14400, 20000, 27600, 38000,
  52500, 71500, 97000, 132000, 180000, 243000, 328000, 445000, 600000, 800000,
  1065000, 1410000, 1900000, 2500000, 3300000, 4300000, 5600000, 7200000, 9200000, 12000000,
  15000000, 19000000, 24000000, 30000000, 38000000, 48000000, 60000000, 75000000, 93000000, 116250000,
];

// Hot potato books on weapons.
export const HOT_POTATO = { id: 'HOT_POTATO_BOOK', max: 10, damage: 2, strength: 2 };
export const FUMING_POTATO = { id: 'FUMING_POTATO_BOOK', max: 5, damage: 2, strength: 2 };

// Bazaar ids for master stars, in order. A star can only be bought after the one before it.
export const MASTER_STAR_IDS = [
  'FIRST_MASTER_STAR', 'SECOND_MASTER_STAR', 'THIRD_MASTER_STAR', 'FOURTH_MASTER_STAR', 'FIFTH_MASTER_STAR',
];

// Which item categories (from the items resource) each class may use as a weapon.
export const CLASS_WEAPON_CATEGORIES = {
  arch: ['BOW'],
  bers: ['SWORD', 'LONGSWORD'],
  mage: ['SWORD', 'WAND', 'LONGSWORD'],
};

export const ARMOR_SLOTS = ['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'];
export const EQUIPMENT_SLOTS = ['NECKLACE', 'CLOAK', 'BELT', 'GLOVES', 'BRACELET'];
export const ACCESSORY_CATEGORY = 'ACCESSORY';

// How many upgrades the greedy planner lists.
export const MAX_UPGRADES_LISTED = 15;

// Price refresh intervals.
export const AUCTION_REFRESH_MS = 5 * 60 * 1000;
export const BAZAAR_REFRESH_MS = 2 * 60 * 1000;
export const ITEMS_REFRESH_MS = 60 * 60 * 1000;
