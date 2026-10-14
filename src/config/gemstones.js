// Gemstone stats by quality and item rarity (COMMON .. MYTHIC; DIVINE uses MYTHIC).
// Source: SkyCrypt gemstone constants and the wiki Gemstone Slots page.
export const GEMSTONES = {
  JASPER: {
    stat: 'strength',
    values: {
      ROUGH: [1, 1, 1, 2, 3, 4], FLAWED: [2, 2, 3, 4, 4, 5], FINE: [3, 3, 4, 5, 6, 7],
      FLAWLESS: [5, 6, 7, 8, 10, 12], PERFECT: [6, 7, 9, 11, 13, 16],
    },
  },
  ONYX: {
    stat: 'critDamage',
    values: {
      ROUGH: [1, 1, 2, 2, 3, 4], FLAWED: [2, 2, 3, 3, 4, 6], FINE: [3, 3, 4, 5, 6, 8],
      FLAWLESS: [4, 5, 6, 7, 8, 10], PERFECT: [5, 6, 7, 8, 10, 12],
    },
  },
  SAPPHIRE: {
    stat: 'intelligence',
    values: {
      ROUGH: [2, 3, 4, 5, 6, 7], FLAWED: [5, 5, 6, 7, 8, 10], FINE: [7, 8, 9, 10, 11, 12],
      FLAWLESS: [10, 11, 12, 14, 17, 20], PERFECT: [12, 14, 17, 20, 24, 30],
    },
  },
};

export const GEM_QUALITIES = ['ROUGH', 'FLAWED', 'FINE', 'FLAWLESS', 'PERFECT'];
// Qualities the planner offers as upgrades.
export const UPGRADE_QUALITIES = ['FLAWLESS', 'PERFECT'];

// Which gems a slot type accepts. Typed slots (JASPER, ONYX, ...) only accept themselves.
export const SLOT_GEMS = {
  COMBAT: ['JASPER', 'SAPPHIRE', 'ONYX', 'RUBY', 'AMETHYST'],
  OFFENSIVE: ['JASPER', 'SAPPHIRE'],
  DEFENSIVE: ['RUBY', 'AMETHYST', 'OPAL'],
  UNIVERSAL: ['JASPER', 'SAPPHIRE', 'ONYX', 'RUBY', 'AMETHYST', 'OPAL', 'TOPAZ', 'JADE', 'AMBER', 'PERIDOT', 'CITRINE', 'AQUAMARINE'],
};
