// Regenerates src/data/*.json from the NotEnoughUpdates community repo.
// Run with: npm run update-data
import { writeFile, mkdir } from 'node:fs/promises';

const NEU = 'https://raw.githubusercontent.com/NotEnoughUpdates/NotEnoughUpdates-REPO/master/constants';
const OUT = new URL('../src/data/', import.meta.url);

const STAT_KEYS = {
  strength: 'strength',
  crit_damage: 'critDamage',
  crit_chance: 'critChance',
  ability_damage: 'abilityDamage',
  intelligence: 'intelligence',
  speed: 'speed',
  walk_speed: 'speed',
  attack_speed: 'attackSpeed',
  bonus_attack_speed: 'attackSpeed',
  ferocity: 'ferocity',
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

const [basic, stones, misc, petMeta, petNums] = await Promise.all([
  getJson('reforges.json'), getJson('reforgestones.json'), getJson('misc.json'), getJson('pets.json'), getJson('petnums.json'),
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

// Fragged (STARRED_) items: base item + dungeon boss fragments, from NEU recipes.
const hypixel = await (await fetch('https://api.hypixel.net/v2/resources/skyblock/items')).json();
const starred = hypixel.items.map((i) => i.id).filter((id) => id.startsWith('STARRED_'));
const fragged = {};
await Promise.all(starred.map(async (id) => {
  const res = await fetch(`https://raw.githubusercontent.com/NotEnoughUpdates/NotEnoughUpdates-REPO/master/items/${id}.json`);
  if (!res.ok) return;
  const recipe = (await res.json()).recipe;
  if (!recipe) return;
  const counts = {};
  for (const cell of Object.values(recipe)) {
    if (!cell) continue;
    const [item, qty] = cell.split(':');
    counts[item] = (counts[item] ?? 0) + Number(qty || 1);
  }
  const base = id.replace(/^STARRED_/, '');
  if (!counts[base]) return;
  delete counts[base];
  fragged[base] = { id, parts: Object.entries(counts).map(([item, qty]) => ({ id: item, qty })) };
}));

// NPC shops: what every NPC sells and its cost per item, from the npc_shop recipes in NEU's items/*_NPC.json.
// { ITEM_ID: [{ npc, coins, items?: [{ id, qty }] }] }, files in path order, offers in file order. Free offers and
// pets / enchanted books (ids with ';', which never match a price key) are left out.
const NEU_TREE = 'https://api.github.com/repos/NotEnoughUpdates/NotEnoughUpdates-REPO/git/trees/master?recursive=1';
const tree = await (await fetch(NEU_TREE)).json();
if (!Array.isArray(tree?.tree) || tree.truncated) throw new Error('NEU repo file list: incomplete');
const npcFiles = tree.tree.map((x) => x.path).filter((p) => /^items\/[^/]+_NPC\.json$/.test(p)).sort();
const npcData = new Array(npcFiles.length).fill(null);
let nextNpc = 0;
await Promise.all(Array.from({ length: 16 }, async () => {
  while (nextNpc < npcFiles.length) {
    const i = nextNpc++;
    const res = await fetch(`https://raw.githubusercontent.com/NotEnoughUpdates/NotEnoughUpdates-REPO/master/${npcFiles[i]}`);
    if (res.ok) npcData[i] = await res.json();
  }
}));
// "ID:qty" (qty defaults to 1)
const shopPart = (s) => {
  const i = String(s).lastIndexOf(':');
  return i < 0 ? [String(s), 1] : [s.slice(0, i), Number(s.slice(i + 1))];
};
const npcShop = {};
for (const npc of npcData) {
  // "§9Combat Merchant (NPC)" -> "Combat Merchant", "Alchemist (Rift NPC)" -> "Alchemist (Rift)"
  const name = String(npc?.displayname ?? '').replace(/§./g, '').trim().replace(/ \(NPC\)$/, '').replace(/ \(Rift NPC\)$/, ' (Rift)');
  if (!name) continue;
  for (const r of npc.recipes ?? []) {
    if (r.type !== 'npc_shop' || !r.result) continue;
    const [id, n] = shopPart(r.result);
    if (id.includes(';') || !(n > 0)) continue;
    let coins = 0;
    const items = [];
    for (const c of r.cost ?? []) {
      const [cid, q] = shopPart(c);
      if (cid === 'SKYBLOCK_COIN') coins += q;
      else items.push({ id: cid, qty: q / n });
    }
    if (!(coins >= 1) && !items.length) continue;
    const offer = { npc: name, coins: coins / n, ...(items.length ? { items } : {}) };
    const list = (npcShop[id] ??= []);
    if (!list.some((o) => JSON.stringify(o) === JSON.stringify(offer))) list.push(offer);
  }
}
const npcShopSorted = Object.fromEntries(Object.keys(npcShop).sort().map((k) => [k, npcShop[k]]));

// Pets: leveling tables and stat numbers at level 1 and max for every pet with a DPS stat.
const PET_STATS = { STRENGTH: 'strength', CRIT_DAMAGE: 'critDamage', CRIT_CHANCE: 'critChance', INTELLIGENCE: 'intelligence', SPEED: 'speed', BONUS_ATTACK_SPEED: 'attackSpeed', ABILITY_DAMAGE: 'abilityDamage', FEROCITY: 'ferocity' };
const petStats = {};
for (const [type, rarities] of Object.entries(petNums)) {
  for (const [rarity, data] of Object.entries(rarities)) {
    if (!data?.['1'] || !data?.['100']) continue;
    const pick = (nums) => Object.fromEntries(Object.entries(nums ?? {}).filter(([k]) => PET_STATS[k]).map(([k, v]) => [PET_STATS[k], v]));
    (petStats[type] ??= {})[rarity] = {
      min: pick(data['1'].statNums), max: pick(data['100'].statNums),
      otherMin: data['1'].otherNums ?? [], otherMax: data['100'].otherNums ?? [],
      ...(data.stats_levelling_curve ? { curve: data.stats_levelling_curve } : {}),
    };
  }
}
const pets = {
  levels: petMeta.pet_levels,
  rarityOffset: petMeta.pet_rarity_offset,
  custom: Object.fromEntries(Object.entries(petMeta.custom_pet_leveling).map(([k, v]) => [k, { levels: v.pet_levels ?? [], maxLevel: v.max_level ?? 100, type: v.type ?? 0 }])),
  stats: petStats,
};

await mkdir(OUT, { recursive: true });
await writeFile(new URL('pets.json', OUT), JSON.stringify(pets) + '\n');
await writeFile(new URL('reforges.json', OUT), JSON.stringify(reforges, null, 1) + '\n');
await writeFile(new URL('accessories.json', OUT), JSON.stringify(accessories, null, 1) + '\n');
await writeFile(new URL('fragged.json', OUT), JSON.stringify(fragged, null, 1) + '\n');
await writeFile(new URL('npcShop.json', OUT), JSON.stringify(npcShopSorted, null, 1) + '\n');
console.log(`Wrote ${Object.keys(reforges).length} reforges, ${Object.keys(accessories.upgrades).length} accessory families, ${Object.keys(fragged).length} fragged items, ${Object.keys(npcShopSorted).length} NPC shop items.`);
