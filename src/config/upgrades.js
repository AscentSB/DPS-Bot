// ============================================================================
//  EXTRA UPGRADES DATABASE (pets, ability items, perks)
// ============================================================================
// Gear, reforges, enchants, gems, master stars, potato books, recombobulators,
// accessories and shards are handled generically. This list covers everything else.
// Values come from the M7 archer sheet unless noted.
//
//   key       unique id
//   name      label shown in Discord
//   stats     applied against every target; `wither` / `dragon` for one target
//   cost      list of { id, qty } priced from bazaar / auction house,
//             or { coins, estimate: true } when there is no market item (shown as "est.")
//   owned     how to detect it on a profile:
//               items: [ITEM_IDs]   any of these anywhere in the profile
//               pets:  [PET_TYPEs]  any of these pets
//               petItem: { pet, item } pet with that held item
//             leave out when the API does not expose it; the upgrade is then shown with a "?"
//   group     only one member of a group is active at a time (the best one you own)
//   requires  key that must be owned first
//   petStrength  strength a Chimera-enchanted Ragnarock copies from this pet
//   classes   only offer for these classes (optional)
// ============================================================================

export const EXTRA_UPGRADES = [
  // ---- Pets (only one active) ----
  {
    key: 'PET_GOLDEN_DRAGON', name: 'Golden Dragon Pet + Antique Remedies', group: 'pet',
    stats: { strength: 300, additiveDamage: 250, additiveStrength: 0.05 }, petStrength: 300,
    cost: [{ id: 'PET_GOLDEN_DRAGON_LEGENDARY' }, { id: 'ANTIQUE_REMEDIES' }],
    owned: { pets: ['GOLDEN_DRAGON'] },
  },
  {
    key: 'PET_ENDER_DRAGON', name: 'Ender Dragon Pet + Gold Claws', group: 'pet',
    stats: { strength: 80, critDamage: 90, critChance: 10, additiveStrength: 0.1, additiveCritDamage: 0.1, additiveCritChance: 0.1 },
    dragon: { additiveDamage: 200 }, petStrength: 80,
    cost: [{ id: 'PET_ENDER_DRAGON_LEGENDARY' }, { id: 'GOLD_CLAWS' }],
    owned: { pets: ['ENDER_DRAGON'] },
  },
  {
    key: 'PET_JELLYFISH', name: 'Jellyfish Pet', group: 'pet',
    stats: { strength: 20, critDamage: 15, critChance: 10, additiveDamage: 25 }, petStrength: 20,
    cost: [{ id: 'PET_JELLYFISH_LEGENDARY' }],
    owned: { pets: ['JELLYFISH'] },
  },
  {
    key: 'HEPHAESTUS_REMEDIES', name: 'Hephaestus Remedies (Golden Dragon)', requires: 'PET_GOLDEN_DRAGON', activeWith: 'PET_GOLDEN_DRAGON',
    stats: { strength: 30 }, petStrength: 30,
    cost: [{ id: 'HEPHAESTUS_REMEDIES' }],
    owned: { petItem: { pet: 'GOLDEN_DRAGON', item: 'HEPHAESTUS_REMEDIES' } },
  },
  {
    key: 'HEPHAESTUS_RELIC', name: 'Hephaestus Relic (Golden Dragon)', requires: 'PET_GOLDEN_DRAGON', activeWith: 'PET_GOLDEN_DRAGON',
    stats: { strength: 25 }, petStrength: 25,
    cost: [{ id: 'HEPHAESTUS_RELIC' }],
    owned: { petItem: { pet: 'GOLDEN_DRAGON', item: 'HEPHAESTUS_RELIC' } },
  },

  // ---- Ability items (buffs while used) ----
  {
    key: 'WEIRD_TUBA', name: 'Weird Tuba', group: 'tuba',
    stats: { strength: 30 }, cost: [{ id: 'WEIRD_TUBA' }], owned: { items: ['WEIRD_TUBA'] },
  },
  {
    key: 'WEIRDER_TUBA', name: 'Weirder Tuba', group: 'tuba',
    stats: { strength: 40, critDamage: 10 }, cost: [{ id: 'WEIRDER_TUBA' }], owned: { items: ['WEIRDER_TUBA'] },
  },
  {
    key: 'SWORD_OF_BAD_HEALTH', name: 'Sword of Bad Health (ability)',
    stats: { strength: 100 }, cost: [{ id: 'SWORD_OF_BAD_HEALTH' }], owned: { items: ['SWORD_OF_BAD_HEALTH'] },
  },

  // ---- Perks (not in the public API; shown with "?") ----
  { key: 'PERK_STRENGTH_ESSENCE', name: 'Strength Essence perk', stats: { strength: 50 }, cost: [{ coins: 17_400_000, estimate: true }] },
  { key: 'PERK_CRITICAL_ESSENCE', name: 'Critical Essence perk', stats: { critDamage: 50 }, cost: [{ coins: 50_400_000, estimate: true }] },
  { key: 'PERK_FORBIDDEN_STRENGTH', name: 'Forbidden Strength perk', stats: { strength: 5 }, cost: [{ coins: 9_000_000, estimate: true }] },
  { key: 'PERK_FORBIDDEN_SPEED', name: 'Forbidden Speed perk', stats: { critChance: 0.4 }, cost: [{ coins: 1_000_000, estimate: true }] },
  { key: 'PERK_BLESSING_OF_TIME', name: 'Blessing of Time (Rift Sun Gecko shop)', stats: { strength: 6 }, cost: [{ coins: 1_000_000, estimate: true }] },
];
