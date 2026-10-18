// Tunable constants for the DPS model. Every number the formulas use lives here.

export const CLASSES = {
  arch: { label: 'Archer', weights: { wither: 50, dragon: 50 } },
  bers: { label: 'Berserker', weights: { wither: 20, dragon: 80 } },
  mage: { label: 'Mage', weights: { wither: 80, dragon: 20 } },
};

// Class bonuses from class level. Each class is assumed to be the only one of its kind in the party, which
// doubles its class bonuses (SOLO_CLASS_MULTIPLIER). Source: https://hypixelskyblock.minecraft.wiki/w/Classes
export const SOLO_CLASS_MULTIPLIER = 2;
// Archer: +0.8% arrow damage per Archer level, a multiplicative buff (x1.8 at level 50 when solo).
export const ARCHER_ARROW_DAMAGE_PER_LEVEL = 0.8;
// Berserker: +0.75% damage per Berserk level on top of a 40% base, counted as additive damage
// (40 + 2 x 37.5 = 115 at level 50 when solo).
export const BERSERK_DAMAGE_BASE = 40;
export const BERSERK_DAMAGE_PER_LEVEL = 0.75;
// Berserker Lust for Blood: additive damage, 1200 at Berserk 50 when solo (12 per level, doubled solo).
export const LUST_FOR_BLOOD_PER_LEVEL = 12;
// Combat skill Warrior bonus: +4% additive damage per level to 50, +1% per level from 51 to 60 (210 at 60).
// Applies to every class (Mage is scored as LCM, the mage beam).
export const WARRIOR_PER_LEVEL = 4;
export const WARRIOR_PER_LEVEL_AFTER_50 = 1;

// A global upgrade counts for wither weight + dragon weight = 100.
export const GLOBAL_WEIGHT = 100;

// Boss defense, used in (1 - defense / (defense + 100)) on boss hits. The Mage clear set (RCM) is scored on clear
// mobs and ignores it.
export const TARGET_DEFENSE = {
  wither: 9000,
  dragon: 12500,
};

// damage = BASE_DAMAGE + weapon_damage
export const BASE_DAMAGE = 5;

// Dungeon blessings (https://hypixelskyblock.minecraft.wiki/w/Blessings).
// Each blessing adds level x 4 flat and level x 2% to its stats, both times the blessing effect E:
//   stat = (flat + sum(4 * level * E)) (1 + additive + 2nd additive)(multiplicative) * product(1 + 0.02 * level * E)
//   E = (1 + floor buff 20%) (1 + Forbidden Blessing 1-10%) (1 + Mimic shard 1-10%)
// The original formula's 1.1 * 1.1 * 1.2 is E with Forbidden Blessing X and Mimic X.
// Forbidden Blessing and Mimic come from the profile, so they also show up as upgrades.
// Mayor perks (Paul's Benediction and the rest) are never counted.
// Default levels; /dps can set others (power 5-29, wisdom 0-14, time 0 or 5).
export const BLESSINGS = {
  power: { level: 29, stats: ['strength', 'critDamage'] },
  wisdom: { level: 14, stats: ['intelligence'] },
  // Blessing of Time: from the Quiz puzzle, always 0 or 5.
  time: { level: 5, stats: ['strength', 'intelligence'] },
};
export const BLESSING_LIMITS = { power: { min: 5, max: 29 }, wisdom: { min: 0, max: 14 }, time: { values: [0, 5] } };

// The blessings with chosen levels ({ power, wisdom, time }; missing or invalid ones keep the default).
export function blessingsWith(levels = {}) {
  const out = {};
  for (const [name, b] of Object.entries(BLESSINGS)) {
    const limit = BLESSING_LIMITS[name];
    const v = levels?.[name];
    const ok = Number.isInteger(v) && (limit.values ? limit.values.includes(v) : v >= limit.min && v <= limit.max);
    out[name] = { ...b, level: ok ? v : b.level };
  }
  return out;
}
export const BLESSING_FLAT_PER_LEVEL = 4;
export const BLESSING_PERCENT_PER_LEVEL = 0.02;
export const BLESSING_FLOOR_BUFF = 0.2;

// Speed: tuning points go into speed until you reach SPEED_CAP (with your damage pet). Only the
// points left after that can go to crit damage / crit chance. Speed past the cap does nothing.
// Speed = base 100 + Dungeon VII potion (Speed III, see DUNGEON_POTION) + Wolf Slayer + every tracked gear/accessory/perk source.
// The Rogue Sword is not counted. Put any other always-on speed in EXTRA_SPEED.
export const BASE_SPEED = 100;
// 500 already includes the Folf shard's Catacombs Cap (+50 at level X).
export const SPEED_CAP = 500;
// Mage clears in RCM armor and is only scored in the boss, where 450 speed is enough.
export const SPEED_CAP_BY_CLASS = { mage: 450 };
// The Mage RCM clear set needs the full 500.
export const speedCap = (cls, mode) => (mode === 'rcm' ? SPEED_CAP : SPEED_CAP_BY_CLASS[cls] ?? SPEED_CAP);
// Classes that wear separate clear boots (e.g. Maxor's) and swap for the boss. Mage clears in RCM armor,
// so its boss gear is scored on its own, with no clear boots.
export const CLEAR_BOOTS_CLASSES = ['arch', 'bers'];
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
// Combat skill adds +0.5 base crit chance per level (60 at Combat 60).
// Source: https://hypixelskyblock.minecraft.wiki/w/Crit_Chance
export const BASE_CRIT_CHANCE = 30;
export const COMBAT_CRIT_CHANCE_PER_LEVEL = 0.5;
export const COMBAT_MAX_LEVEL = 60;
// Skill XP needed for each level (index 0 = level 1). Source: NEU repo constants/leveling.json.
export const SKILL_XP_PER_LEVEL = [
  50, 125, 200, 300, 500, 750, 1000, 1500, 2000, 3500, 5000, 7500, 10000, 15000, 20000, 30000, 50000, 75000, 100000,
  200000, 300000, 400000, 500000, 600000, 700000, 800000, 900000, 1000000, 1100000, 1200000, 1300000, 1400000, 1500000,
  1600000, 1700000, 1800000, 1900000, 2000000, 2100000, 2200000, 2300000, 2400000, 2500000, 2600000, 2750000, 2900000,
  3100000, 3400000, 3700000, 4000000, 4300000, 4600000, 4900000, 5200000, 5500000, 5800000, 6100000, 6400000, 6700000, 7000000,
];

// Damage formula switches at 100% and 200% crit chance (overload).
export const OVERLOAD_MULTIPLIER = 1.5;

// Mage is scored as LCM (Mage Staff passive): each beam deals (0.3 + 0.0009 x intelligence) of the melee hit
// (strength, crit damage, crit chance). Ability damage does not boost the beam (it is for RCM). Beams fire faster
// with attack speed: 20 / round(MAGE_BEAM_TICKS / (1 + attack speed / 100)) per second.
// Source: https://hypixelskyblock.minecraft.wiki/w/Mage
export const MAGE_BEAM_BASE = 0.3;
export const MAGE_BEAM_PER_INTELLIGENCE = 0.0009;
export const MAGE_BEAM_TICKS = 10;
// Mage class intelligence: 250 + 5 per Mage level (500 at 50), per-level part doubled when solo (750 at 50).
export const MAGE_INT_BASE = 250;
export const MAGE_INT_PER_LEVEL = 5;

// Mage RCM (clear set): Wither Impact on a Necron's Blade variant (Hyperion, Astraea, Valkyrie, Scylla).
// One cast = 10,000 x (1 + Catacombs stat boost) x (1 + intelligence / 100 x 0.3) x (1 + ability damage / 100)
//   x (1 + magic additive / 100). No crits, no strength, no weapon enchants.
// Source: https://hypixelskyblock.minecraft.wiki/w/Damage_Calculation#Ability_Damage
export const WITHER_IMPACT_BASE = 10000;
export const WITHER_IMPACT_SCALING = 0.3;
export const WITHER_IMPACT_WEAPONS = ['HYPERION', 'ASTRAEA', 'VALKYRIE', 'SCYLLA', 'NECRON_BLADE'];
// Wither Impact needs all 3 scrolls (ExtraAttributes.ability_scroll). The Ultimate Wither Scroll counts as all 3.
export const WITHER_SCROLLS = ['IMPLOSION_SCROLL', 'SHADOW_WARP_SCROLL', 'WITHER_SHIELD_SCROLL'];
export const ULTIMATE_WITHER_SCROLL = 'ULTIMATE_WITHER_SCROLL';
// Shadow Assassin Cloak: the next hit after a teleport deals 10% more, once every SA_CLOAK_COOLDOWN_S seconds
// (shortened by Mage cooldown reduction). Averaged over every hit: 1 + 0.1 / (hits per cooldown). Needs a Wither
// Impact blade in hand (all 3 scrolls) to teleport. 2 of 3 scrolls does nothing.
// The One counts maxed collections. With no collection data, every collection counts as maxed.
export const DEFAULT_MAXED_COLLECTIONS = 90;
export const SA_CLOAK_IDS = ['SHADOW_ASSASSIN_CLOAK', 'STARRED_SHADOW_ASSASSIN_CLOAK'];
export const SA_CLOAK_MULTIPLIER = 1.1;
export const SA_CLOAK_COOLDOWN_S = 3;
// Mage cooldown reduction: Mage level / 2 percent (max 25), doubled when solo: 50% at level 50.
export const MAGE_COOLDOWN_REDUCTION_PER_LEVEL = 0.5;
export const MAGE_COOLDOWN_REDUCTION_MAX = 25;
// Mage class ability damage: +5, plus +1 per 5 Mage levels (doubled when solo, like the class intelligence).
export const MAGE_ABILITY_DAMAGE_BASE = 5;
export const MAGE_ABILITY_DAMAGE_PER_5_LEVELS = 1;
// Enchanting skill: +0.5% ability damage per level (skills API off: assumed maxed).
export const ENCHANTING_ABILITY_DAMAGE_PER_LEVEL = 0.5;
export const ENCHANTING_MAX_LEVEL = 60;
// Loving reforge (Red Scarf) on the chestplate: 1.05x ability damage (multiplicative).
export const LOVING_ABILITY_MULTIPLIER = 1.05;
// Implosion Belt (RCM): x1.25 Wither Impact damage.
export const IMPLOSION_BELT_ID = 'IMPLOSION_BELT';
export const IMPLOSION_BELT_MULTIPLIER = 1.25;

// Ferocity hits land about 0.5s late, and Goldor (the only place Berserker procs it) lives about 1s, so
// ferocity damage counts half.
export const FEROCITY_EFFECTIVENESS = 0.5;

// Arrows: Archer always uses Nansorb Arrows, Berserker always Bouncy Arrows. Both add 5 base damage to the bow;
// their special effects (piercing, bouncing) do nothing against one boss.
export const ARROW_DAMAGE = { arch: 5, bers: 5 };

// Swarm: enemies within 10 blocks. One boss at a time; RCM clears hit packs of mobs.
export const BOSS_SWARM_ENEMIES = 1;
export const RCM_SWARM_ENEMIES = 7;
// The only enchants that raise Wither Impact damage on the clear set, in Master Mode and Normal Mode alike.
export const RCM_ENCHANTS = ['giant_killer', 'prosecute', 'ultimate_swarm', 'smite'];
// Thunderlord strikes every 3rd hit.
export const THUNDERLORD_EVERY = 3;
// Mage boss set at or above this DPS (against a boss) also counts First Strike in Master Mode.
export const MAGE_FIRST_STRIKE_DPS = 1e9;

// Fabled reforge (swords): every hit deals 0-15% more damage, multiplicative, so 1.075x on average.
export const FABLED_MULTIPLIER = 1.075;

// Necron's Blade and its refinements (WITHER_IMPACT_WEAPONS): "Deals +50% damage to wither mobs" while held,
// for every class.
export const NECRON_BLADE_WITHER_MULTIPLIER = 1.5;

// Dual wield (LCM): a second copy of a Prosecute weapon with Execute, swapped in once the boss is low. Counted as
// this much more additive damage on average.
export const DUAL_WIELD_WEAPONS = ['HYPERION', 'DARK_CLAYMORE'];
export const DUAL_WIELD_ADDITIVE = 50;


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
export const EQUIPMENT_SLOTS = ['NECKLACE', 'CLOAK', 'BELT', 'GLOVES'];
// Equipment categories that share a slot: bracelets go in the gloves slot (one item per slot).
export const EQUIPMENT_SLOT_OF = { BRACELET: 'GLOVES' };

// Equipment slot for an item category, or null when the category is not equipment.
export function equipmentSlot(category) {
  const slot = EQUIPMENT_SLOT_OF[category] ?? category;
  return EQUIPMENT_SLOTS.includes(slot) ? slot : null;
}
export const ACCESSORY_CATEGORY = 'ACCESSORY';

// Diamond and gold boss heads are SPECIAL items that already get max reforge stats; recombobulating does nothing.
export const MAX_REFORGE_ITEMS = /^(DIAMOND|GOLD)_[A-Z_]+_HEAD$/;

export const RARITIES = ['COMMON', 'UNCOMMON', 'RARE', 'EPIC', 'LEGENDARY', 'MYTHIC', 'DIVINE'];

// Catacombs level needed for each dungeon floor tier (index 0 = floor 1). Floors 8-10 are the item tiers
// of Master Mode 5-7 drops. Source: the wiki floor pages.
export const DUNGEON_FLOOR_CATA = [1, 3, 5, 9, 14, 19, 24, 32, 34, 36];
// Floor-scaled dungeon armor is assumed to have the max 50% stat boost.
export const DUNGEON_DROP_BOOST = 0.5;
// Catacombs level needed for Master Mode floors 1-7 (requirement checks on items).
export const MASTER_FLOOR_CATA = [24, 26, 28, 30, 32, 34, 36];
// Slayer XP needed for each level, cumulative (index 0 = level 1). Source: NEU repo constants/leveling.json.
export const SLAYER_XP = {
  zombie: [5, 15, 200, 1000, 5000, 20000, 100000, 400000, 1000000],
  spider: [5, 15, 200, 1000, 5000, 20000, 100000, 400000, 1000000],
  wolf: [10, 30, 250, 1500, 5000, 20000, 100000, 400000, 1000000],
  enderman: [10, 30, 250, 1500, 5000, 20000, 100000, 400000, 1000000],
  blaze: [10, 30, 250, 1500, 5000, 20000, 100000, 400000, 1000000],
  vampire: [20, 75, 240, 840, 2400],
};
export const SPECIAL_RARITIES = ['SPECIAL', 'VERY_SPECIAL'];

// Ragnarock: its ability grants RAGNAROCK_MULTIPLIER x the axe's strength.
export const RAGNAROCK_ID = 'RAGNAROCK_AXE';
export const RAGNAROCK_MULTIPLIER = 1.5;
export const CHIMERA_PER_LEVEL = 0.2;

// Legion: +0.07% combat stats per level per nearby player. 4 nearby players against withers (every class) and
// against dragons for Mage. 1 everywhere else (Archer / Berserker dragons, the RCM clear).
export const LEGION_PER_LEVEL = 0.0007;
export const LEGION_PLAYERS = 1;
export const LEGION_GROUPED_PLAYERS = 4;

// How many upgrades the greedy planner lists.
export const MAX_UPGRADES_LISTED = 15;

// Boots: an Archer or Berserker with Maxor's boots only swaps them off for Necron's boots in the boss.
// Mage has no such rule (Storm's boots beat Maxor's in the boss).
export const MAXOR_BOOTS_ID = 'SPEED_WITHER_BOOTS';
export const NECRON_BOOTS_ID = 'POWER_WITHER_BOOTS';

// Items a class never uses. Necron's armor (and Necron heads) is Archer and Berserker gear; Mage tops out at
// Storm's armor.
// Archer and Berserker always wear the Skeleton Master Chestplate (infinite arrows) from Catacombs 36 (M7): no
// other chestplate is used. Below that, any chestplate can be.
export const SKELETON_MASTER_CHEST_CLASSES = ['arch', 'bers'];
export const SKELETON_MASTER_CHEST_CATA = 36;
export const SKELETON_MASTER_CHESTS = ['SKELETON_MASTER_CHESTPLATE', 'STARRED_SKELETON_MASTER_CHESTPLATE'];

export const CLASS_EXCLUDED_ITEMS = {
  mage: /^(POWER_WITHER_(HELMET|CHESTPLATE|LEGGINGS|BOOTS)|DIAMOND_NECRON_HEAD|GOLD_NECRON_HEAD)$/,
};

// Blacksmith reforges (no stone, e.g. Blended on equipment) cost 500 coins per roll and roll at random
// from every basic reforge for the item type, so getting one costs 500 x that many rolls on average.
// Source: https://hypixelskyblock.minecraft.wiki/w/Reforging#Basic_Reforging
export const BLACKSMITH_REFORGE_COST = 500;

// Armor only ever gets Ancient: nothing else is offered or assumed on armor. (Light on Maxor's boots would
// win on paper by freeing tuning points, but most players wear the same boots all run.)
export const ARMOR_REFORGE = 'ancient';

// Price refresh intervals.
export const AUCTION_REFRESH_MS = 5 * 60 * 1000;
export const BAZAAR_REFRESH_MS = 5 * 60 * 1000;
export const ITEMS_REFRESH_MS = 60 * 60 * 1000;
// Auction prices are the median of each item's lowest BIN over the last few changed downloads, so one sniped
// listing or one bought-out cheapest listing does not swing the plan.
export const AUCTION_SMOOTH_DOWNLOADS = 3;
// A player's profile is reused for this long before Hypixel is asked again.
export const PROFILE_CACHE_MS = 5 * 60 * 1000;
