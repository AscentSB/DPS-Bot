// Stats from the player's profile (not gear). Counted when the profile is read (ctx.profileStats).
// Skill and slayer rewards: NEU repo constants/bonuses.json. A key is the first level that gives that much
// per level, until the next key.

// Player base crit damage.
export const BASE_CRIT_DAMAGE = 50;

// Per-level rewards, from the key level on. Only damage stats are kept.
export const SKILL_BONUSES = {
  FORAGING: { max: 57, steps: { 1: { strength: 1 }, 15: { strength: 2 } } },
  ENCHANTING: { max: 60, steps: { 1: { intelligence: 1 }, 15: { intelligence: 2 } } },
  ALCHEMY: { max: 50, steps: { 1: { intelligence: 1 }, 15: { intelligence: 2 } } },
};
// Skills API off: skills count as maxed.

// Slayer rewards by level (wolf speed is already in the profile speed).
export const SLAYER_BONUSES = {
  spider: { 1: { critDamage: 1 }, 2: { critDamage: 1 }, 3: { critDamage: 1 }, 4: { critDamage: 1 }, 5: { critDamage: 2 }, 6: { critDamage: 2 }, 7: { critChance: 1 }, 8: { critDamage: 3 }, 9: { critDamage: 3 } },
  wolf: { 5: { critDamage: 1 }, 7: { critDamage: 2 } },
  enderman: { 2: { intelligence: 1 }, 4: { intelligence: 2 }, 6: { intelligence: 3 }, 8: { intelligence: 4 } },
};
export const SLAYER_MAX_LEVEL = 9;

// SkyBlock level: +1 strength every 5 levels (100 XP per level).
export const SB_LEVEL_XP = 100;
export const SB_STRENGTH_EVERY = 5;

// Melody's Harp: intelligence for each song played perfectly (song_<key>_perfect_completions).
export const HARP_SONGS = {
  hymn_joy: { name: 'Hymn to the Joy', intelligence: 1 },
  frere_jacques: { name: 'Frère Jacques', intelligence: 1 },
  amazing_grace: { name: 'Amazing Grace', intelligence: 1 },
  brahms: { name: "Brahm's Lullaby", intelligence: 2 },
  happy_birthday: { name: 'Happy Birthday to You', intelligence: 2 },
  greensleeves: { name: 'Greensleeves', intelligence: 2 },
  jeopardy: { name: 'Geothermy?', intelligence: 3 },
  minuet: { name: 'Minuet', intelligence: 3 },
  joy_world: { name: 'Joy to the World', intelligence: 3 },
  pure_imagination: { name: 'Godly Imagination', intelligence: 4 },
  vie_en_rose: { name: 'La Vie en Rose', intelligence: 4 },
};

// Active Century Cake buffs (player_data.temp_stat_buffs) that raise damage stats.
export const CAKE_STATS = { strength: 'strength', critical_damage: 'critDamage', critical_chance: 'critChance', intelligence: 'intelligence', walk_speed: 'speed' };
