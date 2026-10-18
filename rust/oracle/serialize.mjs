// JSON shapes for the JS planner's objects, as rust/PORTING_NOTES.md "Test data" describes them. Shared by every
// oracle script that dumps loadouts, evaluate() results, contexts or candidates (part 2 of the oracle reuses it).
//
// Items and pets are written as JSON.stringify writes them (undefined fields dropped), which is model::ItemState /
// model::Pet. Maps become arrays of values or [key, value] pairs as noted per field, Sets become arrays, both in
// insertion order.

const orNull = (v) => (v === undefined ? null : v);

// A loadout (buildLoadout / cloneLoadout shape) as L:
// { weapon, armor: {SLOT: item}, equipment: {SLOT: item}, rag, clearBoots, dragonGear: null | { weapon, armor,
//   equipment }, accessories: [item, ...] (Map values in insertion order), owned: [id, ...], extras: [key, ...],
//   shards, perks, pets: [pet, ...], activePets: { wither: pet|null, dragon: pet|null }, power, bopBonus, bank,
//   witherImpact, blades: [item], spareBlades: [item], rcmBlade, unlockedPowers: [power, ...] }
// Fields a JS loadout lacks (buildLoadout's first object has no dragonGear / power / blades yet) come out as null or
// [], the values the JS code reads them as. `bank` is null only on a loadout whose bank was never set (the JS then
// reads ctx.petProfile.bank); every loadout after buildLoadout has a number there.
//
// `alias` (only present when non-empty): the JS compares gear by object identity (slotName, replaceItem, clearBoots ===
// armor.BOOTS), which JSON cannot show. alias maps a gear slot to the earlier slot holding the SAME object, slots
// named 'weapon', 'rag', 'armor.SLOT', 'clearBoots', 'equipment.SLOT', 'dragon.weapon', 'dragon.armor.SLOT',
// 'dragon.equipment.SLOT' and visited in that order, e.g. { clearBoots: 'armor.BOOTS' }. A reader using value equality
// for gear (as the Rust port does) gets the same behaviour; deserializeLoadout re-creates the identity.
function gearSlots(l) {
  const out = [['weapon', l.weapon], ['rag', l.rag]];
  for (const [k, v] of Object.entries(l.armor)) out.push([`armor.${k}`, v]);
  out.push(['clearBoots', l.clearBoots]);
  for (const [k, v] of Object.entries(l.equipment)) out.push([`equipment.${k}`, v]);
  const d = l.dragonGear;
  if (d) {
    out.push(['dragon.weapon', d.weapon]);
    for (const [k, v] of Object.entries(d.armor)) out.push([`dragon.armor.${k}`, v]);
    for (const [k, v] of Object.entries(d.equipment)) out.push([`dragon.equipment.${k}`, v]);
  }
  return out;
}
function aliasesOf(l) {
  const seen = new Map();
  const alias = {};
  for (const [slot, item] of gearSlots(l)) {
    if (!item || typeof item !== 'object') continue;
    if (seen.has(item)) alias[slot] = seen.get(item);
    else seen.set(item, slot);
  }
  return alias;
}

export function serializeLoadout(l) {
  const d = l.dragonGear;
  const alias = aliasesOf(l);
  return {
    ...(Object.keys(alias).length ? { alias } : {}),
    weapon: orNull(l.weapon),
    armor: { ...l.armor },
    equipment: { ...l.equipment },
    rag: orNull(l.rag),
    clearBoots: orNull(l.clearBoots),
    dragonGear: d ? { weapon: orNull(d.weapon), armor: { ...d.armor }, equipment: { ...d.equipment } } : null,
    accessories: [...(l.accessories?.values() ?? [])],
    owned: [...(l.owned ?? [])],
    extras: [...(l.extras ?? [])],
    shards: { ...l.shards },
    perks: { ...l.perks },
    pets: [...(l.pets ?? [])],
    activePets: { wither: orNull(l.activePets?.wither), dragon: orNull(l.activePets?.dragon) },
    power: orNull(l.power),
    bopBonus: orNull(l.bopBonus),
    bank: orNull(l.bank),
    witherImpact: Boolean(l.witherImpact),
    blades: [...(l.blades ?? [])],
    spareBlades: [...(l.spareBlades ?? [])],
    rcmBlade: orNull(l.rcmBlade),
    unlockedPowers: [...(l.unlockedPowers ?? [])],
  };
}

// L back into a JS loadout (Maps / Sets rebuilt, accessories keyed by item id as the JS keys them, active pets
// pointing at the equal entry of `pets`). Used to check that L holds everything evaluate() reads.
export function deserializeLoadout(L) {
  const same = (a, b) => a && b && a.type === b.type && a.tier === b.tier && (a.heldItem ?? null) === (b.heldItem ?? null);
  const pets = L.pets.map((p) => ({ ...p }));
  const active = (p) => (p ? pets.find((x) => same(x, p)) ?? { ...p } : null);
  const out = {
    ...L,
    dragonGear: L.dragonGear ? { weapon: L.dragonGear.weapon, armor: { ...L.dragonGear.armor }, equipment: { ...L.dragonGear.equipment } } : null,
    accessories: new Map(L.accessories.map((a) => [a.id, a])),
    owned: new Set(L.owned),
    extras: new Set(L.extras),
    pets,
    activePets: { wither: active(L.activePets.wither), dragon: active(L.activePets.dragon) },
    bank: L.bank ?? undefined,
    unlockedPowers: new Set(L.unlockedPowers),
  };
  delete out.alias;
  // Re-create the identities the JS compares with ===.
  const get = (path) => path.split('.').reduce((o, k) => (k === 'dragon' ? o.dragonGear : o[k]), out);
  const put = (path, v) => {
    const parts = path.split('.');
    const last = parts.pop();
    parts.reduce((o, k) => (k === 'dragon' ? o.dragonGear : o[k]), out)[last] = v;
  };
  for (const [slot, from] of Object.entries(L.alias ?? {})) put(slot, get(from));
  return out;
}

// An evaluate() result as R: every field as JS computed it, `extras` as the list of active extra keys (in the order
// activeExtras returned them).
export function serializeEval(r) {
  return { ...r, extras: (r.extras ?? []).map((e) => e.key) };
}

// The planner context without the items resource (makeContext fields, tuning with the chosen split, weights and
// anything else set on it). Maps / Sets inside become arrays.
export function serializeCtx(ctx) {
  const { resource, ...rest } = ctx;
  return plain(rest);
}

// A candidate from availableUpgrades without its apply function:
// { label, cost, source, unverified?, group?, stacksOn?, repick?, armorSlot?, armorId?, bank? }.
export function serializeCandidate(up) {
  const out = { label: up.label, cost: up.cost, source: up.source };
  for (const k of ['unverified', 'group', 'stacksOn', 'repick', 'armorSlot', 'armorId', 'bank']) if (up[k] !== undefined) out[k] = up[k];
  return out;
}

// Deep copy with Maps as [key, value] arrays and Sets as arrays (JSON.stringify would write {} for both).
export function plain(v) {
  if (v instanceof Map) return [...v].map(([k, x]) => [k, plain(x)]);
  if (v instanceof Set) return [...v].map(plain);
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === 'object') {
    const out = {};
    for (const [k, x] of Object.entries(v)) if (x !== undefined && typeof x !== 'function') out[k] = plain(x);
    return out;
  }
  return v;
}

// Paths of numbers JSON cannot hold (NaN, Infinity): JSON.stringify writes them as null, which a Rust f64 field
// rejects. Used to check dumps before writing them.
export function nonFinitePaths(v, at = '$', out = []) {
  if (typeof v === 'number') { if (!Number.isFinite(v)) out.push(`${at}=${v}`); return out; }
  if (v instanceof Map || v instanceof Set) return nonFinitePaths([...v], at, out);
  if (Array.isArray(v)) v.forEach((x, i) => nonFinitePaths(x, `${at}[${i}]`, out));
  else if (v && typeof v === 'object') for (const [k, x] of Object.entries(v)) nonFinitePaths(x, `${at}.${k}`, out);
  return out;
}
