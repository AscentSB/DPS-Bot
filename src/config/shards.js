// Attribute shards that raise DPS. Source: https://hypixelskyblock.minecraft.wiki/w/Attributes
// Each attribute goes from level 1 to 10. `perLevel` is the effect of one level.
// Bazaar id: SHARD_<ID>. `attribute` is the key in the profile's attributes.stacks (syphoned shard count).
//
//   family   'elemental' | 'unlimited' | 'ruler' | 'box' (what the echo shards boost)
//   echo     boosts another family by `perLevel.echo` per level (0.02 = +2% per level)
//   target   'global' | 'wither' | 'dragon'
//   classes  only count for these classes (optional)

export const SHARDS = {
  // Elemental: +1 strength per level each. Bolt is newer than the M7 archer sheet.
  FLASH: { name: 'Flash', attribute: 'light_elemental', rarity: 'COMMON', family: 'elemental', perLevel: { strength: 1 } },
  QUAKE: { name: 'Quake', attribute: 'stone_elemental', rarity: 'UNCOMMON', family: 'elemental', perLevel: { strength: 1 } },
  BOLT: { name: 'Bolt', attribute: 'lightning_elemental', rarity: 'RARE', family: 'elemental', perLevel: { strength: 1 } },
  AERO: { name: 'Aero', attribute: 'wind_elemental', rarity: 'EPIC', family: 'elemental', perLevel: { strength: 1 } },
  TEMPEST: { name: 'Tempest', attribute: 'storm_elemental', rarity: 'LEGENDARY', family: 'elemental', perLevel: { strength: 1 } },
  STARBORN: { name: 'Starborn', attribute: 'echo_of_elemental', rarity: 'LEGENDARY', echo: 'elemental', perLevel: { echo: 0.02 } },

  // Unlimited Power / Energy: +0.1% strength / crit damage per level.
  JORMUNG: { name: 'Jormung', attribute: 'unlimited_power', rarity: 'LEGENDARY', family: 'unlimited', perLevel: { additiveStrength: 0.001 } },
  ETHERDRAKE: { name: 'Etherdrake', attribute: 'unlimited_energy', rarity: 'LEGENDARY', family: 'unlimited', perLevel: { additiveCritDamage: 0.001 } },
  MOLTHORN: { name: 'Molthorn', attribute: 'almighty', rarity: 'LEGENDARY', echo: 'unlimited', perLevel: { echo: 0.05 } },

  // Rulers: +3% damage per level against one mob type. Dragons are Ender, withers are Undead.
  BRUISER: { name: 'Zealot Bruiser', attribute: 'ender', rarity: 'UNCOMMON', family: 'ruler', target: 'dragon', perLevel: { additiveDamage: 3 } },
  ZOMBIE_SOLDIER: { name: 'Zombie Soldier', attribute: 'undead', rarity: 'UNCOMMON', family: 'ruler', target: 'wither', perLevel: { additiveDamage: 3 } },
  CAIMAN: { name: 'Caiman', attribute: 'echo_of_ruler', rarity: 'EPIC', echo: 'ruler', perLevel: { echo: 0.02 } },

  // Tuning Box: +1 tuning point per level.
  HIDEONBOX: { name: 'Hideonbox', attribute: 'tuning_box', rarity: 'LEGENDARY', family: 'box', perLevel: { tuningPoints: 1 } },
  CUBOA: { name: 'Cuboa', attribute: 'echo_of_boxes', rarity: 'UNCOMMON', echo: 'box', perLevel: { echo: 0.02 } },

  // Echo of Echoes: boosts every echo shard above by +5% per level.
  TIAMAT: { name: 'Tiamat', attribute: 'echo_of_echoes', rarity: 'LEGENDARY', echo: 'echo', perLevel: { echo: 0.05 } },

  // Attack Speed: +1 attack speed per level, frees tuning points. Bazaar id kept from its old Burningsoul name.
  BURNINGSOUL: { name: 'Inferno Demonlord', attribute: 'attack_speed', rarity: 'EPIC', perLevel: { attackSpeed: 1 } },

  // Speed: frees tuning points (see TUNING_PER_POINT).
  OBSIDIAN_DEFENDER: { name: 'Obsidian Defender', attribute: 'speed', rarity: 'UNCOMMON', perLevel: { speed: 1 } },

  // Faker: dungeon blessings +1% stronger per level.
  MIMIC: { name: 'Mimic', attribute: 'faker', rarity: 'EPIC', perLevel: { blessingShard: 0.01 } },

  // Plain damage attributes.
  POWER_DRAGON: { name: 'Power Dragon', attribute: 'elite', rarity: 'LEGENDARY', perLevel: { additiveDamage: 3 } },
  SKELETOR: { name: 'Skeletor', attribute: 'deadeye', rarity: 'RARE', classes: ['arch'], perLevel: { additiveDamage: 2.5 } },
  THORN: { name: 'Thorn', attribute: 'dominance', rarity: 'EPIC', perLevel: { additiveDamage: 1.5 } },
  BARBARIAN_DUKE_X: { name: 'Barbarian Duke X', attribute: 'warrior', rarity: 'EPIC', classes: ['bers'], perLevel: { additiveDamage: 2.5 } },
};

export const SHARD_MAX_LEVEL = 10;

// Cumulative shards needed to reach each level (index 0 = level 1).
export const SHARDS_CUMULATIVE = {
  COMMON: [1, 4, 9, 15, 22, 30, 40, 54, 72, 96],
  UNCOMMON: [1, 3, 6, 10, 15, 21, 28, 36, 48, 64],
  RARE: [1, 3, 6, 9, 13, 17, 22, 28, 36, 48],
  EPIC: [1, 2, 4, 6, 9, 12, 16, 20, 25, 32],
  LEGENDARY: [1, 2, 3, 5, 7, 9, 12, 15, 19, 24],
};
