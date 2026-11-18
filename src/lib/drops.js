import { DUNGEON_FLOOR_CATA, DUNGEON_DROP_BOOST } from '../config/constants.js';

// Floor-scaled dungeon items (tiered_stats in the items resource): armor like Skeleton Master and weapons like the
// Crypt Dreadlord Sword, Ice Spray Wand or Machine Gun Bow. Every piece is assumed dropped from the highest floor
// the player can enter, with the full 50% stat boost, and is priced that way. Since 0.27.3 the boost no longer raises
// the rarity (owner, 2026-11-17): a piece keeps its listed tier.
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

// Base rarity of an item: its listed tier. Floor-scaled dungeon armor no longer gets +1 rarity (0.27.3).
export function dropBaseRarity(id, resource) {
  return resource?.get(id)?.tier ?? 'COMMON';
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
