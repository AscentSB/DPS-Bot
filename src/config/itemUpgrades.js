// Items that upgrade into a better item and keep every upgrade on them (stars, enchants, gems, reforge,
// recombobulator). Recipes from the NEU repo: the item in the middle, 8 of the part around it.
//   Golden boss heads -> Diamond boss heads (8 Enchanted Diamond Blocks).
//   Wither armor -> Necron's / Storm's / Maxor's / Goldor's armor (8 Giant fragments).
// Fragging (STARRED_ items) works the same way and lives in src/data/fragged.json.

const HEAD_BOSSES = ['BONZO', 'SCARF', 'PROFESSOR', 'THORN', 'LIVID', 'SADAN', 'NECRON'];
const WITHER_CLASSES = { POWER: 'GIANT_FRAGMENT_DIAMOND', WISE: 'GIANT_FRAGMENT_LASER', SPEED: 'GIANT_FRAGMENT_BIGFOOT', TANK: 'GIANT_FRAGMENT_BOULDER' };
const ARMOR = ['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'];

// from id -> [{ id: upgraded id, parts: [{ id, qty }] }]
export const ITEM_UPGRADES = {
  ...Object.fromEntries(HEAD_BOSSES.map((b) => [`GOLD_${b}_HEAD`, [{ id: `DIAMOND_${b}_HEAD`, parts: [{ id: 'ENCHANTED_DIAMOND_BLOCK', qty: 8 }] }]])),
  ...Object.fromEntries(ARMOR.map((s) => [`WITHER_${s}`,
    Object.entries(WITHER_CLASSES).map(([cls, frag]) => ({ id: `${cls}_WITHER_${s}`, parts: [{ id: frag, qty: 8 }] }))])),
};
