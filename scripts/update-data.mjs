// Regenerates src/data/*.json from the NotEnoughUpdates community repo.
// Run with: npm run update-data
import { writeFile, mkdir } from 'node:fs/promises';

const NEU = 'https://raw.githubusercontent.com/NotEnoughUpdates/NotEnoughUpdates-REPO/master/constants';
const OUT = new URL('../src/data/', import.meta.url);

const STAT_KEYS = {
  strength: 'strength',
  crit_damage: 'critDamage',
  crit_chance: 'critChance',
  intelligence: 'intelligence',
};

async function getJson(name) {
  const res = await fetch(`${NEU}/${name}`);
  if (!res.ok) throw new Error(`${name}: HTTP ${res.status}`);
  return res.json();
}

export function reforgeKey(name) {
  return String(name).toLowerCase().replace(/[^a-z]/g, '');
}

// Returns { types, items }: item categories, or specific ITEM_IDs the reforge is limited to.
function itemTypes(raw) {
  if (typeof raw === 'string') return { types: raw.split(/[ ,/]+/).filter(Boolean) };
  if (Array.isArray(raw?.internalName)) return { types: [], items: raw.internalName };
  return { types: [] };
}

function convertStats(byRarity) {
  const out = {};
  for (const [rarity, stats] of Object.entries(byRarity ?? {})) {
    const s = {};
    for (const [k, v] of Object.entries(stats)) if (STAT_KEYS[k]) s[STAT_KEYS[k]] = v;
    if (Object.keys(s).length) out[rarity] = s;
  }
  return out;
}

// Reforges that grant a stat per Catacombs level.
const CATA_ABILITY = { ancient: 'critDamage', withered: 'strength' };

const [basic, stones, misc] = await Promise.all([
  getJson('reforges.json'), getJson('reforgestones.json'), getJson('misc.json'),
]);

const reforges = {};
for (const r of Object.values(basic)) {
  reforges[reforgeKey(r.reforgeName)] = { name: r.reforgeName, stone: null, ...itemTypes(r.itemTypes), stats: convertStats(r.reforgeStats) };
}
for (const [stoneId, r] of Object.entries(stones)) {
  if (!r.reforgeName) continue;
  const key = reforgeKey(r.reforgeName);
  reforges[key] = {
    name: r.reforgeName,
    stone: stoneId,
    ...itemTypes(r.itemTypes),
    stats: convertStats(r.reforgeStats),
    ...(CATA_ABILITY[key] ? { cataStat: CATA_ABILITY[key] } : {}),
  };
}

const accessories = { upgrades: misc.talisman_upgrades, ignored: misc.ignored_talisman };

await mkdir(OUT, { recursive: true });
await writeFile(new URL('reforges.json', OUT), JSON.stringify(reforges, null, 1) + '\n');
await writeFile(new URL('accessories.json', OUT), JSON.stringify(accessories, null, 1) + '\n');
console.log(`Wrote ${Object.keys(reforges).length} reforges and ${Object.keys(accessories.upgrades).length} accessory families.`);
