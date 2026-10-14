// ============================================================================
//  EXTRA UPGRADES DATABASE (ability items)
// ============================================================================
// Gear, reforges, enchants, gems, master stars, potato books, recombobulators, fragging,
// accessories, shards and essence shop perks (src/config/essence.js) are handled generically.
// Pets live in src/config/pets.js. This list covers ability items.
// Values come from the M7 archer sheet unless noted.
//
//   key       unique id
//   name      label shown in Discord
//   stats     applied against every target; `wither` / `dragon` for one target
//   cost      list of { id, qty } priced from bazaar / auction house
//   owned     how to detect it on a profile:
//               items: [ITEM_IDs]   any of these anywhere in the profile
//             leave out when the API does not expose it; the upgrade is then shown with a "?"
//   group     only one member of a group is active at a time (the best one you own)
//   requires  key that must be owned first
//   classes   only offer for these classes (optional)
// ============================================================================

export const EXTRA_UPGRADES = [
  // ---- Ability items (buffs while used) ----
  // Tubas: Howl only gets used during dragons (P5), so the buff counts against dragons only.
  // Source: https://hypixelskyblock.minecraft.wiki/w/Weirder_Tuba (outside the Rift).
  {
    key: 'WEIRD_TUBA', name: 'Weird Tuba', group: 'tuba',
    dragon: { strength: 30, speed: 30 }, cost: [{ id: 'WEIRD_TUBA' }], owned: { items: ['WEIRD_TUBA'] },
  },
  {
    key: 'WEIRDER_TUBA', name: 'Weirder Tuba', group: 'tuba',
    dragon: { strength: 40, attackSpeed: 5, speed: 30, critDamage: 10 }, cost: [{ id: 'WEIRDER_TUBA' }], owned: { items: ['WEIRDER_TUBA'] },
  },
  {
    key: 'SWORD_OF_BAD_HEALTH', name: 'Sword of Bad Health (ability)',
    stats: { strength: 100 }, cost: [{ id: 'SWORD_OF_BAD_HEALTH' }], owned: { items: ['SWORD_OF_BAD_HEALTH'] },
  },
];
