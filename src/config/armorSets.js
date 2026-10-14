// Armor set and piece bonuses that raise DPS. Source: item lore in the NEU repo.
//
//   pieces   ITEM_IDs of the set (any slot)
//   perPiece stats for each piece worn
//   full     stats when all 4 pieces are worn
//   classes  only for these classes (optional)
//
// bowDamage = +% damage with arrows (counts only while holding a bow).
// Bonuses that depend on the run (Shadow Assassin kills, Final Destination sneaking) are left out.

const set = (prefix) => ['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'].map((s) => `${prefix}_${s}`);

export const ARMOR_SETS = [
  { name: 'Skeleton Master', pieces: set('SKELETON_MASTER'), perPiece: { bowDamage: 5 }, full: { bowDamage: 25 } },
  { name: 'Skeleton Soldier', pieces: set('SKELETON_SOLDIER'), perPiece: { bowDamage: 5 }, full: { bowDamage: 25 } },
  { name: "Maxor's", pieces: set('SPEED_WITHER'), perPiece: { bowDamage: 5 } },
  { name: 'Superior Dragon', pieces: set('SUPERIOR_DRAGON'), full: { additiveStrength: 0.05, additiveCritDamage: 0.05, additiveCritChance: 0.05, additiveIntelligence: 0.05 } },
  { name: 'Young Dragon', pieces: set('YOUNG_DRAGON'), full: { speed: 70 } },
  { name: 'Adaptive (Berserk)', pieces: [...set('ADAPTIVE'), ...set('STARRED_ADAPTIVE')], perPiece: { strength: 20 }, classes: ['bers'] },
  { name: 'Adaptive (Mage)', pieces: [...set('ADAPTIVE'), ...set('STARRED_ADAPTIVE')], perPiece: { intelligence: 50 }, classes: ['mage'] },
];
