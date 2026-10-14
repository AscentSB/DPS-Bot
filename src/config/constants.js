// Tunable constants for the DPS model. Every number the formulas use lives here.

// baseAdditiveDamage: Archer 310 matches the M7 archer sheet (combat 60 and class bonuses).
export const CLASSES = {
  arch: { label: 'Archer', weights: { wither: 50, dragon: 50 }, baseAdditiveDamage: 310 },
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

// Dungeon blessings (https://hypixelskyblock.minecraft.wiki/w/Blessings).
// Each blessing adds level x 4 flat and level x 2% to its stats, both times the blessing effect E:
//   stat = (flat + sum(4 * level * E)) (1 + additive + 2nd additive)(multiplicative) * product(1 + 0.02 * level * E)
//   E = (1 + floor buff 20%) (1 + Forbidden Blessing 1-10%) (1 + Mimic shard 1-10%) (1 + Paul's Benediction 25%)
// The original formula's 1.1 * 1.1 * 1.2 is E with Forbidden Blessing X and Mimic X.
// Forbidden Blessing and Mimic come from the profile, so they also show up as upgrades.
export const BLESSINGS = {
  power: { level: 19, stats: ['strength', 'critDamage'] },
  wisdom: { level: 12, stats: ['intelligence'] },
  // Blessing of Time: from the Quiz puzzle, always 0 or 5.
  time: { level: 5, stats: ['strength', 'intelligence'] },
};
export const BLESSING_FLAT_PER_LEVEL = 4;
export const BLESSING_PERCENT_PER_LEVEL = 0.02;
export const BLESSING_FLOOR_BUFF = 0.2;
// Set to true while Paul is mayor with Benediction.
export const PAUL_BENEDICTION = false;
export const BENEDICTION_BUFF = 0.25;

// Speed: tuning points go into speed until you reach SPEED_CAP (with your damage pet). Only the
// points left after that can go to crit damage / crit chance. Speed past the cap does nothing.
// Speed = base 100 + Dungeon VII potion (Speed III, see DUNGEON_POTION) + Wolf Slayer + every tracked gear/accessory/perk source.
// The Rogue Sword is not counted. Put any other always-on speed in EXTRA_SPEED.
export const BASE_SPEED = 100;
export const SPEED_CAP = 500;
export const EXTRA_SPEED = 0;

// Dungeon VII potion (Ophelia): Strength V, Critical III, Archery III, Speed III.
// A Jellyfish pet makes it up to 50% stronger, and the buff stays after swapping pets.
// bowDamage only counts while holding a bow.
export const DUNGEON_POTION = { strength: 40, critChance: 20, critDamage: 30, bowDamage: 50, speed: 15 };

// Attack speed: tuning fills attack speed up to ATTACK_SPEED_CAP first (before speed and crit).
// Shortbows fire every ceil(shotTicks / (1 + AS/100)) ticks.
export const ATTACK_SPEED_CAP = 100;
export const TICKS_PER_SECOND = 20;

// Archer class: Doubleshot is 50%, doubled to 100% as the only Archer in the party (assumed),
// plus 1% per Archer level: 1.5 extra arrows at level 50.
export const DOUBLESHOT_BASE = 1;
export const DOUBLESHOT_PER_LEVEL = 0.01;

// Wolf Slayer: +1 speed at levels 1, 3 and 8. XP needed for levels 1-9.
export const WOLF_SLAYER_XP = [10, 30, 250, 1500, 5000, 20000, 100000, 400000, 1000000];
export const WOLF_SLAYER_SPEED_LEVELS = [1, 3, 8];

// crit chance = (flat + BASE_CRIT_CHANCE)(1 + additive)(multiplicative). Terminator has a 0.25 multiplier.
export const BASE_CRIT_CHANCE = 30;

// Damage formula switches at 100% and 200% crit chance (overload).
export const OVERLOAD_MULTIPLIER = 1.5;

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
//   stat       * (1 + cataBoost + 0.1 * stars + 0.05 * masterStars + 0.01 * secretDigit)
//   critChance * (1 + 0.1 * stars + 0.05 * masterStars + 0.01 * secretDigit)
// At 5 stars this is cataBoost + 1.5 + ... and 1.5 + ... as in the original formula.
// secretDigit comes from the General's Medallion: number of digits in your secret count, capped at 5.
export const DUNGEON_BASE = 1;
export const STAR_STEP = 0.1;
export const MAX_STARS = 5;
export const MASTER_STAR_STEP = 0.05;
export const MAX_MASTER_STARS = 5;
export const SECRET_STEP = 0.01;
export const SECRET_DIGIT_CAP = 5;
export const GENERAL_MEDALLION_ID = 'GENERAL_MEDALLION';

// Catacombs dungeon item stat bonus in percent, cumulative. Index = Catacombs level (0 to 50).
// Level 0 already gives +10%. From the current Dungeoneering table:
// https://hypixelskyblock.minecraft.wiki/w/Dungeoneering
// Paste new values here if Hypixel changes them. cataBoost = value / 100.
export const CATA_STAT_BONUS = [
  10,
  15, 20, 25, 31, 38, 46, 55, 65, 75, 85,
  95, 105, 115, 125, 135, 145, 155, 165, 175, 185,
  195, 205, 215, 225, 235, 245, 255, 265, 275, 285,
  295, 305, 315, 325, 335, 345, 355, 365, 375, 385,
  395, 405, 415, 425, 435, 445, 455, 465, 475, 485,
];

// XP needed for each Catacombs level (not cumulative). Index 0 = level 1.
export const CATA_XP_PER_LEVEL = [
  50, 75, 110, 160, 230, 330, 470, 670, 950, 1340,
  1890, 2665, 3760, 5260, 7380, 10300, 14400, 20000, 27600, 38000,
  52500, 71500, 97000, 132000, 180000, 243000, 328000, 445000, 600000, 800000,
  1065000, 1410000, 1900000, 2500000, 3300000, 4300000, 5600000, 7200000, 9200000, 12000000,
  15000000, 19000000, 24000000, 30000000, 38000000, 48000000, 60000000, 75000000, 93000000, 116250000,
];

// Hot potato books (weapons only for damage and strength).
export const HOT_POTATO = { id: 'HOT_POTATO_BOOK', max: 10, damage: 2, strength: 2 };
export const FUMING_POTATO = { id: 'FUMING_POTATO_BOOK', max: 5, damage: 2, strength: 2 };

// The Art of War: +5 strength on a weapon, one per item.
export const ART_OF_WAR = { id: 'THE_ART_OF_WAR', strength: 5 };

export const RECOMBOBULATOR_ID = 'RECOMBOBULATOR_3000';

// Bazaar ids for master stars, in order. A star can only be bought after the one before it.
export const MASTER_STAR_IDS = [
  'FIRST_MASTER_STAR', 'SECOND_MASTER_STAR', 'THIRD_MASTER_STAR', 'FOURTH_MASTER_STAR', 'FIFTH_MASTER_STAR',
];

// Which item categories (from the items resource) each class may use as a weapon.
export const CLASS_WEAPON_CATEGORIES = {
  arch: ['BOW'],
  // Berserkers also use the Terminator in M7 and never melee.
  bers: ['BOW'],
  mage: ['SWORD', 'WAND', 'LONGSWORD'],
};

export const ARMOR_SLOTS = ['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'];
export const EQUIPMENT_SLOTS = ['NECKLACE', 'CLOAK', 'BELT', 'GLOVES', 'BRACELET'];
export const ACCESSORY_CATEGORY = 'ACCESSORY';

// Diamond and gold boss heads are SPECIAL items that already get max reforge stats; recombobulating does nothing.
export const MAX_REFORGE_ITEMS = /^(DIAMOND|GOLD)_[A-Z_]+_HEAD$/;

export const RARITIES = ['COMMON', 'UNCOMMON', 'RARE', 'EPIC', 'LEGENDARY', 'MYTHIC', 'DIVINE'];
export const SPECIAL_RARITIES = ['SPECIAL', 'VERY_SPECIAL'];

// Ragnarock: its ability grants RAGNAROCK_MULTIPLIER x the axe's strength.
export const RAGNAROCK_ID = 'RAGNAROCK_AXE';
export const RAGNAROCK_MULTIPLIER = 1.5;
export const CHIMERA_PER_LEVEL = 0.2;

// Legion: +0.07% combat stats per level per nearby player. Assume 1 nearby player in every boss phase
export const LEGION_PER_LEVEL = 0.0007;
export const LEGION_PLAYERS = 1;

// How many upgrades the greedy planner lists.
export const MAX_UPGRADES_LISTED = 15;

// Blacksmith reforges (no stone, e.g. Blended on equipment) cost 500 coins per roll and roll at random
// from every basic reforge for the item type, so getting one costs 500 x that many rolls on average.
// Source: https://hypixelskyblock.minecraft.wiki/w/Reforging#Basic_Reforging
export const BLACKSMITH_REFORGE_COST = 500;

// Armor only ever gets Ancient: nothing else is offered or assumed on armor. (Light on Maxor's boots would
// win on paper by freeing tuning points, but most players wear the same boots all run.)
export const ARMOR_REFORGE = 'ancient';

// Price refresh intervals.
export const AUCTION_REFRESH_MS = 5 * 60 * 1000;
export const BAZAAR_REFRESH_MS = 2 * 60 * 1000;
export const ITEMS_REFRESH_MS = 60 * 60 * 1000;
