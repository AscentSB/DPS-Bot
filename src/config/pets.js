// Pets: base stats come from src/data/pets.json (NEU repo, `npm run update-data`).
// Only one pet is active at a time; the bot uses whichever owned pet gives the most damage.
// Source: https://hypixelskyblock.minecraft.wiki/w/Pets and the pet item pages.

// Pet items. `mult` multiplies the pet's stat, `flat` adds to it.
export const PET_ITEMS = {
  ANTIQUE_REMEDIES: { name: 'Antique Remedies', mult: { strength: 1.8 } },
  HEPHAESTUS_REMEDIES: { name: 'Hephaestus Remedies', mult: { strength: 2 } },
  // +50% to every pet stat (Crow + Hephaestus Relic + Chimera is an RCM setup: intelligence and ability damage).
  HEPHAESTUS_RELIC: { name: 'Hephaestus Relic', mult: { strength: 1.5, critDamage: 1.5, critChance: 1.5, intelligence: 1.5, speed: 1.5, attackSpeed: 1.5, abilityDamage: 1.5 } },
  GOLD_CLAWS: { name: 'Gold Claws', mult: { critDamage: 1.5, critChance: 1.5 } },
  PET_ITEM_SHARPENED_CLAWS_UNCOMMON: { name: 'Sharpened Claws', flat: { critDamage: 15 } },
  PET_ITEM_BIG_TEETH_COMMON: { name: 'Big Teeth', flat: { critChance: 5 } },
  PET_ITEM_TEXTBOOK: { name: 'Textbook', mult: { intelligence: 2 } },
  // Plushies: the easiest way to reach 100 attack speed.
  CROCHET_TIGER_PLUSHIE: { name: 'Crochet Tiger Plushie', flat: { attackSpeed: 35 } },
  HEPHAESTUS_PLUSHIE: { name: 'Hephaestus Plushie', flat: { attackSpeed: 50 } },
};

// Perks that raise damage, by pet type. `n` is the pet's perk numbers at its level (NEU otherNums),
// `p` is profile data ({ bank, goldCollection }). Return stats; `potionBoost` buffs the Dungeon potion.
// Pets not listed here count their base stats only.
export const PET_PERKS = {
  // Shining Scales: +11.1 strength per digit of Gold collection (max 9 digits).
  // Dragon's Greed: +n[1]% strength per 5 Magic Find, max n[2]% (assumed maxed).
  // Legendary Treasure: +n[3]% damage per million coins in the bank, max n[4]%.
  GOLDEN_DRAGON: (n, p) => ({
    petStrength: 11.1 * Math.min(9, digits(p.goldCollection)),
    additiveStrength: (n[2] ?? 0) / 100,
    additiveDamage: Math.min(n[4] ?? 0, (n[3] ?? 0) * (p.bank / 1e6)),
  }),
  // End Strike: +n[0]% damage to Ender mobs (dragons). Superior: +n[3]% all combat stats, attack speed included.
  ENDER_DRAGON: (n) => ({
    dragon: { additiveDamage: n[0] ?? 0 },
    additiveStrength: (n[3] ?? 0) / 100,
    additiveCritDamage: (n[3] ?? 0) / 100,
    additiveCritChance: (n[3] ?? 0) / 100,
    additiveIntelligence: (n[3] ?? 0) / 100,
    additiveAttackSpeed: (n[3] ?? 0) / 100,
  }),
  // Powerful Potions: Dungeon Potions are n[2]% stronger. Stays after swapping to another pet.
  JELLYFISH: (n) => ({ potionBoost: (n[2] ?? 0) / 100 }),
};

// Pet used against each boss, by class. Golden Dragon vs withers for everyone. Ender Dragon vs dragons
// for Archer and Berserker (dragons are Ender mobs, so End Strike applies). Mage keeps Golden Dragon against dragons
// too: Chimera on the Mage weapon copies its attack speed for the beam. When the player does not own
// the listed pet, or no pet is listed, the owned pet that gives the most damage against that boss is used.
export const PHASE_PETS = {
  wither: { arch: 'GOLDEN_DRAGON', bers: 'GOLDEN_DRAGON', mage: 'GOLDEN_DRAGON' },
  dragon: { arch: 'ENDER_DRAGON', bers: 'ENDER_DRAGON', mage: 'GOLDEN_DRAGON' },
};

// Tier Boost: raises the pet one rarity (never to Mythic). Profiles store the base tier.
export const TIER_BOOST_ID = 'PET_ITEM_TIER_BOOST';

// Kat rarity upgrades: coins and items at level 1. Each pet level removes 0.3% of the coins, so 30% off at
// level 100. A Kat Flower skips one day of the wait. Source: https://hypixelskyblock.minecraft.wiki/w/Kat
export const KAT_LEVEL_DISCOUNT = 0.003;
export const KAT_FLOWER_ID = 'KAT_FLOWER';
export const KAT_UPGRADES = {
  ENDER_DRAGON: { EPIC: { to: 'LEGENDARY', coins: 400_000_000, items: [{ id: 'SUMMONING_EYE', qty: 8 }] } },
};

// Rarity to buy when a pet comes in several (highest first).
export const BUY_RARITIES = ['MYTHIC', 'LEGENDARY'];

function digits(n) {
  return n > 0 ? String(Math.floor(n)).length : 0;
}
