// Essence shop perks that raise DPS. Source: https://hypixelskyblock.minecraft.wiki/w/Essence_Shops
// Cost = essence per level x bazaar price of ESSENCE_<TYPE>.
// Essence can only be bought from the bazaar at Catacombs ESSENCE_MIN_CATA or higher,
// so these upgrades are only offered then.
//
//   key       perk key in the profile (player_data.perks), from SkyCrypt
//   perLevel  stats per level. `blessing` = blessings x% more effective (0.01 = 1%).
//             `petCritDamage` = fraction of the pet's crit damage added (needs `pet` active).
//   costs     essence for each level (index 0 = level 1)

export const ESSENCE_MIN_CATA = 20;

export const ESSENCE_PERKS = {
  catacombs_strength: { name: 'Strength Essence', essence: 'UNDEAD', perLevel: { strength: 10 }, costs: [1000, 4000, 6000, 8000, 10000] },
  catacombs_crit_damage: { name: 'Critical Essence', essence: 'UNDEAD', perLevel: { critDamage: 10 }, costs: [1000, 3000, 10000, 20000, 50000] },
  catacombs_intelligence: { name: 'Intelligence Essence', essence: 'UNDEAD', perLevel: { intelligence: 15 }, costs: [1000, 4000, 6000, 8000, 10000] },
  permanent_strength: { name: 'Forbidden Strength', essence: 'WITHER', perLevel: { strength: 1 }, costs: [100, 250, 500, 1000, 1500] },
  permanent_intelligence: { name: 'Forbidden Intelligence', essence: 'WITHER', perLevel: { intelligence: 2 }, costs: [100, 250, 500, 1000, 1500] },
  forbidden_blessing: { name: 'Forbidden Blessing', essence: 'WITHER', perLevel: { blessing: 0.01 }, costs: [200, 400, 600, 800, 1000, 1200, 1400, 1600, 1800, 2000] },
  edrag_cd: { name: 'Infused Dragon', essence: 'DRAGON', pet: 'PET_ENDER_DRAGON', perLevel: { petCritDamage: 0.02 }, costs: [500, 1500, 2500, 3500, 4000] },
};
