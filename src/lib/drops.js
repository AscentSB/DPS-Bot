import { RARITIES, DUNGEON_FLOOR_CATA, DUNGEON_DROP_BOOST } from '../config/constants.js';

// Floor-scaled dungeon items (tiered_stats in the items resource): armor like Skeleton Master and weapons like the
// Crypt Dreadlord Sword, Ice Spray Wand or Machine Gun Bow. Every piece is assumed dropped from the highest floor
// the player can enter, with the full 50% stat boost, and is priced that way. The boost also raises the piece one
// rarity (an Epic Skeleton Master Chestplate becomes Legendary, Mythic once recombobulated).
export function isFloorDrop(id, resource) {
  const info = resource?.get(id);
  return Boolean(info?.tiered_stats && info.dungeon_item);
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

// Floor-scaled armor counts as fully boosted (50%), and is priced only from fully boosted listings.
export const FULL_STAT_BOOST = 50;

// Price id for a fully boosted floor-scaled piece from at least this floor (filled in by the auction download).
export function floorDropKey(id, floor) {
  return `${id}_FLOOR${floor}`;
}

// The price id a buy uses: floor-scaled armor is priced as the fully boosted piece from the highest floor the
// player can enter; everything else as itself.
export function buyPriceId(id, resource, cataLevel) {
  return isFloorDrop(id, resource) ? floorDropKey(id, Math.max(1, highestFloor(cataLevel))) : id;
}
