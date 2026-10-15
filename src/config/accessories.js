// Accessory Power (formerly Magical Power) settings.
// Source: https://hypixelskyblock.minecraft.wiki/w/Accessory_Power and /w/Accessory_Powers

// Accessory Power each accessory gives, by rarity (after recombobulation).
export const AP_BY_RARITY = {
  COMMON: 3, UNCOMMON: 5, RARE: 8, EPIC: 12, LEGENDARY: 16, MYTHIC: 22, DIVINE: 28, SPECIAL: 3, VERY_SPECIAL: 5,
};

// Dungeon accessories give double Accessory Power inside dungeons. The Hegemony Artifact always gives double.
// The profile's Accessory Power is usually read outside a dungeon, so it can come out lower than the bot's number.
export const DUNGEON_AP_MULTIPLIER = 2;

export const DOUBLE_AP_ITEMS = ['HEGEMONY_ARTIFACT'];

// Fixed Accessory Power for items that do not follow their rarity.
// The Rift Prism gives 11 and is usually consumed (rift.access.consumed_prism) instead of kept in the bag.
export const RIFT_PRISM_ID = 'RIFT_PRISM';
export const AP_OVERRIDES = { [RIFT_PRISM_ID]: 11 };

// Power stats multiplier = 29.97 * ln(0.0019 * AP + 1) ^ 1.2
export const POWER_SCALE = 29.97;
export const POWER_LOG_FACTOR = 0.0019;
export const POWER_EXPONENT = 1.2;

// Stats per point of the power multiplier. A stat = value * multiplier.
// Stone powers are listed on the wiki as "base" values. They convert with
//   value = base / 100 * 24 * statFactor   (statFactor: strength 1, crit damage 1, crit chance 0.4, intelligence 1.5, speed 0.5, attack speed 0.3)
// "bonus" stats are flat unique power bonuses.
const STAT_FACTOR = { strength: 1, critDamage: 1, critChance: 0.4, intelligence: 1.5, speed: 0.5, attackSpeed: 0.3 };
function stone(base, bonus = {}) {
  const stats = {};
  for (const [k, v] of Object.entries(base)) stats[k] = (v / 100) * 24 * STAT_FACTOR[k];
  return { stats, bonus };
}

export const POWERS = {
  // Starter and intermediate powers (values already per multiplier point).
  fortuitous: { stats: { strength: 4.8, critChance: 4.35, critDamage: 4.8 } },
  pretty: { stats: { strength: 4.8, intelligence: 10.8, critChance: 0.475, critDamage: 1.2, speed: 0.65 } },
  protected: { stats: { strength: 2.4, critChance: 0.475, critDamage: 1.2 } },
  simple: { stats: { strength: 3.6, intelligence: 5.4, critChance: 1.45, critDamage: 3.6, speed: 1.2 } },
  warrior: { stats: { strength: 8.4, critChance: 2.4, critDamage: 6 } },
  commando: { stats: { strength: 8.4, critChance: 0.475, critDamage: 8.4 } },
  disciplined: { stats: { strength: 7.2, critChance: 1.45, critDamage: 7.2 } },
  inspired: { stats: { strength: 4.8, intelligence: 16.2, critChance: 0.95, critDamage: 3.6 } },
  ominous: { stats: { strength: 3.6, intelligence: 6.1, critChance: 1.45, critDamage: 3.6, speed: 0.95, attackSpeed: 0.9 } },
  prepared: { stats: { strength: 1.95, critChance: 0.4, critDamage: 0.95 } },
  // Stone powers.
  forceful: stone({ strength: 75, critDamage: 20 }),
  bloody: stone({ strength: 45, critDamage: 45, intelligence: 10 }, { attackSpeed: 10 }),
  bubba: stone({ strength: 20, critChance: 10, critDamage: 45, attackSpeed: 25 }),
  shaded: stone({ strength: 20, critDamage: 75, speed: 5 }, { attackSpeed: 3 }),
  sanguisuge: stone({ strength: 60, critDamage: 20, critChance: 10 }, { intelligence: 100 }),
  bizarre: stone({ intelligence: 120 }),
  adept: stone({ intelligence: 10 }),
  sighted: stone({ intelligence: 100 }),
  crumbly: stone({ intelligence: 15, critChance: 5, attackSpeed: 10 }, { speed: 25 }),
  itchy: stone({ strength: 30, critDamage: 35, speed: 5, attackSpeed: 30 }, { strength: 15, critDamage: 15 }),
  frozen: stone({}, { strength: 25, critDamage: 25 }),
  slender: stone({ strength: 25, critDamage: 25, intelligence: 25, speed: 5, attackSpeed: 15 }, { strength: 50 }),
  demonic: stone({ strength: 23, intelligence: 77 }, { critDamage: 50 }),
  silky: stone({ critDamage: 95, speed: 5 }, { attackSpeed: 5 }),
  hurtful: stone({ strength: 20, critDamage: 80 }, { attackSpeed: 15 }),
  strong: stone({ strength: 50, critDamage: 50 }, { strength: 25, critDamage: 25 }),
  mythical: stone({ strength: 17, critChance: 17, critDamage: 17, intelligence: 17, speed: 8 }, { strength: 40 }),
  scorching: stone({ strength: 35, critDamage: 40, attackSpeed: 25 }),
  pleasant: stone({}),
  sweet: stone({ speed: 10 }, { speed: 5 }),
  healthy: stone({}),
  buttery: stone({ speed: 20, attackSpeed: 30 }, { attackSpeed: 5 }),
};

// Power used when the profile has none selected or an unknown one.
export const DEFAULT_POWER = { arch: 'silky', bers: 'silky', mage: 'sighted' };

// Every 10 Accessory Power grants 1 tuning point.
export const AP_PER_TUNING_POINT = 10;
export const TUNING_PER_POINT = { strength: 1, critDamage: 1, critChance: 0.2, intelligence: 2, speed: 1.5, attackSpeed: 0.3 };
// Tuning order: attack speed up to ATTACK_SPEED_CAP (0.3 per point), then speed up to SPEED_CAP
// (1.5 per point), then the rest to crit chance / crit damage (by the profile's split).
// Profile tuning keys -> stat keys.
export const TUNING_KEYS = {
  strength: 'strength', critical_damage: 'critDamage', crit_damage: 'critDamage',
  critical_chance: 'critChance', crit_chance: 'critChance', intelligence: 'intelligence',
  walk_speed: 'speed', speed: 'speed', attack_speed: 'attackSpeed',
};
// How leftover tuning points split over DPS stats when the profile has no tuning yet.
export const DEFAULT_TUNING_SPLIT = {
  arch: { strength: 0.5, critDamage: 0.5 },
  bers: { strength: 0.5, critDamage: 0.5 },
  // Mage is scored as LCM (mage beam), which uses melee stats.
  mage: { strength: 0.5, critDamage: 0.5 },
};

// Hatcessories (Crab / Sloth Hats of Celebration, Balloon Hats, Cake Hats) all count as one accessory:
// only your best one gives Accessory Power and stats, the rest are duplicates.
export const HATCESSORY = /^(PARTY_HAT_|BALLOON_HAT_|CAKE_HAT_)/;

// Accessories that cannot be recombobulated, on top of every item the Hypixel items resource marks
// with can_recombobulate: false (SkyCrypt SPECIAL_ACCESSORIES).
export const NO_RECOMB = ['BOOK_OF_PROGRESSION', 'PANDORAS_BOX', 'RIFT_PRISM'];

// Enrichments on Legendary and higher accessories. Item ids: TALISMAN_ENRICHMENT_<KEY>.
export const ENRICHMENTS = {
  strength: { key: 'STRENGTH', stats: { strength: 1 } },
  critical_damage: { key: 'CRITICAL_DAMAGE', stats: { critDamage: 1 } },
  critical_chance: { key: 'CRITICAL_CHANCE', stats: { critChance: 1 } },
  intelligence: { key: 'INTELLIGENCE', stats: { intelligence: 2 } },
  walk_speed: { key: 'WALK_SPEED', stats: { speed: 1 } },
  attack_speed: { key: 'ATTACK_SPEED', stats: { attackSpeed: 0.5 } },
};
export const ENRICHABLE_RARITIES = ['LEGENDARY', 'MYTHIC', 'DIVINE', 'SPECIAL', 'VERY_SPECIAL'];
export const BEST_ENRICHMENT = { arch: 'critical_damage', bers: 'critical_damage', mage: 'critical_damage' };
// Enrichments offered per class (each accessory can take one).
export const CLASS_ENRICHMENTS = {
  arch: ['critical_damage', 'critical_chance', 'strength', 'attack_speed', 'walk_speed'],
  bers: ['critical_damage', 'critical_chance', 'strength', 'attack_speed', 'walk_speed'],
  mage: ['critical_damage', 'critical_chance', 'strength', 'intelligence', 'attack_speed', 'walk_speed'],
};
// Tuning stats that raise damage for each class (leftover points are split between these).
export const CLASS_TUNING_STATS = {
  arch: ['strength', 'critDamage', 'critChance'],
  bers: ['strength', 'critDamage', 'critChance'],
  mage: ['strength', 'critDamage', 'critChance', 'intelligence'],
};
// Mage RCM clear set: leftover tuning and enrichments go to intelligence (ability damage cannot be tuned).
export const RCM_TUNING_STATS = ['intelligence'];
export const RCM_ENRICHMENTS = ['intelligence', 'walk_speed'];
export const tuningStats = (ctx) => (ctx?.cls === 'mage' && ctx?.mode === 'rcm' ? RCM_TUNING_STATS : CLASS_TUNING_STATS[ctx?.cls] ?? []);
export const enrichmentKinds = (ctx) => (ctx?.cls === 'mage' && ctx?.mode === 'rcm' ? RCM_ENRICHMENTS : CLASS_ENRICHMENTS[ctx?.cls] ?? []);
// Dungeon accessories (and their enrichments) get the Catacombs boost: x (1 + cata boost). Stars and the
// General's Medallion do not apply, and crit chance and attack speed are not boosted. See accessoryItemStats.
