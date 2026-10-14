// Accessory Power (formerly Magical Power) settings.
// Source: https://hypixelskyblock.minecraft.wiki/w/Accessory_Power and /w/Accessory_Powers

// Accessory Power each accessory gives, by rarity (after recombobulation).
export const AP_BY_RARITY = {
  COMMON: 3, UNCOMMON: 5, RARE: 8, EPIC: 12, LEGENDARY: 16, MYTHIC: 22, DIVINE: 28, SPECIAL: 3, VERY_SPECIAL: 5,
};

// Dungeon accessories give double Accessory Power inside dungeons. The Hegemony Artifact always gives double.
export const DUNGEON_AP_MULTIPLIER = 2;
export const DOUBLE_AP_ITEMS = ['HEGEMONY_ARTIFACT'];

// Power stats multiplier = 29.97 * ln(0.0019 * AP + 1) ^ 1.2
export const POWER_SCALE = 29.97;
export const POWER_LOG_FACTOR = 0.0019;
export const POWER_EXPONENT = 1.2;

// Stats per point of the power multiplier. A stat = value * multiplier.
// Stone powers are listed on the wiki as "base" values. They convert with
//   value = base / 100 * 24 * statFactor   (statFactor: strength 1, crit damage 1, crit chance 0.4, intelligence 1.5, speed 0.5)
// "bonus" stats are flat unique power bonuses.
const STAT_FACTOR = { strength: 1, critDamage: 1, critChance: 0.4, intelligence: 1.5, speed: 0.5 };
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
  ominous: { stats: { strength: 3.6, intelligence: 6.1, critChance: 1.45, critDamage: 3.6, speed: 0.95 } },
  prepared: { stats: { strength: 1.95, critChance: 0.4, critDamage: 0.95 } },
  // Stone powers.
  forceful: stone({ strength: 75, critDamage: 20 }),
  bloody: stone({ strength: 45, critDamage: 45, intelligence: 10 }),
  bubba: stone({ strength: 20, critChance: 10, critDamage: 45 }),
  shaded: stone({ strength: 20, critDamage: 75, speed: 5 }),
  sanguisuge: stone({ strength: 60, critDamage: 20, critChance: 10 }, { intelligence: 100 }),
  bizarre: stone({ intelligence: 120 }),
  adept: stone({ intelligence: 10 }),
  sighted: stone({ intelligence: 100 }),
  crumbly: stone({ intelligence: 15, critChance: 5 }, { speed: 25 }),
  itchy: stone({ strength: 30, critDamage: 35, speed: 5 }, { strength: 15, critDamage: 15 }),
  frozen: stone({}, { strength: 25, critDamage: 25 }),
  slender: stone({ strength: 25, critDamage: 25, intelligence: 25, speed: 5 }, { strength: 50 }),
  demonic: stone({ strength: 23, intelligence: 77 }, { critDamage: 50 }),
  silky: stone({ critDamage: 95, speed: 5 }),
  hurtful: stone({ strength: 20, critDamage: 80 }),
  strong: stone({ strength: 50, critDamage: 50 }, { strength: 25, critDamage: 25 }),
  mythical: stone({ strength: 17, critChance: 17, critDamage: 17, intelligence: 17, speed: 8 }, { strength: 40 }),
  scorching: stone({ strength: 35, critDamage: 40 }),
  pleasant: stone({}),
  sweet: stone({ speed: 10 }, { speed: 5 }),
  healthy: stone({}),
  buttery: stone({ speed: 20 }),
};

// Power used when the profile has none selected or an unknown one.
export const DEFAULT_POWER = { arch: 'silky', bers: 'silky', mage: 'sighted' };

// Every 10 Accessory Power grants 1 tuning point.
export const AP_PER_TUNING_POINT = 10;
export const TUNING_PER_POINT = { strength: 1, critDamage: 1, critChance: 0.2, intelligence: 2, speed: 1.5 };
// Speed above SPEED_CAP (constants.js) lets you move tuning points off speed onto DPS stats
// (1 point per 1.5 speed). Capped at the points the profile has in speed when its tuning is known.
// Profile tuning keys -> stat keys.
export const TUNING_KEYS = {
  strength: 'strength', critical_damage: 'critDamage', crit_damage: 'critDamage',
  critical_chance: 'critChance', crit_chance: 'critChance', intelligence: 'intelligence',
  walk_speed: 'speed', speed: 'speed',
};
// Where new tuning points go when your Accessory Power rises, if the profile has no tuning yet.
export const DEFAULT_TUNING_SPLIT = {
  arch: { strength: 0.5, critDamage: 0.5 },
  bers: { strength: 0.5, critDamage: 0.5 },
  mage: { intelligence: 1 },
};

// Enrichments on Legendary and higher accessories. Item ids: TALISMAN_ENRICHMENT_<KEY>.
export const ENRICHMENTS = {
  strength: { key: 'STRENGTH', stats: { strength: 1 } },
  critical_damage: { key: 'CRITICAL_DAMAGE', stats: { critDamage: 1 } },
  critical_chance: { key: 'CRITICAL_CHANCE', stats: { critChance: 1 } },
  intelligence: { key: 'INTELLIGENCE', stats: { intelligence: 2 } },
  walk_speed: { key: 'WALK_SPEED', stats: { speed: 1 } },
};
export const ENRICHABLE_RARITIES = ['LEGENDARY', 'MYTHIC', 'DIVINE', 'SPECIAL', 'VERY_SPECIAL'];
export const BEST_ENRICHMENT = { arch: 'critical_damage', bers: 'critical_damage', mage: 'intelligence' };
