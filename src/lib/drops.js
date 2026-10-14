import { ARMOR_SLOTS, RARITIES, DUNGEON_FLOOR_CATA, DUNGEON_DROP_BOOST } from '../config/constants.js';

// Floor-scaled dungeon armor (tiered_stats in the items resource, e.g. Skeleton Master).
// Every piece is assumed dropped from the highest floor the player can enter, with the full 50% stat
// boost. The boost also raises the piece one rarity (an Epic Skeleton Master Chestplate becomes Legendary,
// Mythic once recombobulated).
export function isFloorDrop(id, resource) {
  const info = resource?.get(id);
  return Boolean(info?.tiered_stats && info.dungeon_item && ARMOR_SLOTS.includes(info.category));
}

// Highest floor (1-10) the player's Catacombs level allows.
export function highestFloor(cataLevel) {
  return DUNGEON_FLOOR_CATA.filter((need) => (cataLevel ?? 0) >= need).length;
}

// Index into a tiered_stats array for that floor (floor 10 = last entry).
export function floorTierIndex(cataLevel, length) {
  return Math.max(0, Math.min(length - 1, highestFloor(cataLevel) - 1));
}

export function dropStatMultiplier(id, resource) {
  return isFloorDrop(id, resource) ? 1 + DUNGEON_DROP_BOOST : 1;
}

// Base rarity of an item, one higher for floor-scaled dungeon armor.
export function dropBaseRarity(id, resource) {
  const tier = resource?.get(id)?.tier ?? 'COMMON';
  if (!isFloorDrop(id, resource)) return tier;
  const i = RARITIES.indexOf(tier);
  return i >= 0 && i < RARITIES.length - 1 ? RARITIES[i + 1] : tier;
}
