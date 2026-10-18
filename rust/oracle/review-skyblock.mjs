// Adversarial vectors for readMember / toItemState / decodeItems (Rust crates/skyblock review).
//   oracle/run.sh review-skyblock.mjs
// Writes TD/review-skyblock/mutations.json and items.json.
//   mutations: [{ id, base, path, op, value?, expected: { member (items reduced to ids) | error } }]
//   items:     [{ id, blob? , expected: { items | error } }] ; the profile is a slim one built from `base`.
import fs from 'node:fs';
import zlib from 'node:zlib';
import path from 'node:path';
import { createRequire } from 'node:module';
const nbt = createRequire('/app/src/lib/nbt.js')('prismarine-nbt');
import { TD, freezeNow, loadSnapshot, readJson, writeJson } from './common.mjs';

const snap = loadSnapshot();
freezeNow(snap.now);
const { readMember } = await import('/app/src/api/hypixel.js');
const OUT = path.join(TD, 'review-skyblock');
fs.mkdirSync(OUT, { recursive: true });

const BASES = ['Ben_MC', 'zRsk_', 'Vandmelon__'];
const run = (profile, uuid) => {
  try {
    const m = readMember(profile, uuid, snap.resource, snap.collections);
    return { member: m };
  } catch (e) { return { error: e.message }; }
};
const reduce = (r) => {
  if (!r.member) return r;
  const m = { ...r.member };
  m.items = m.items.map((i) => i.id);
  return { member: JSON.parse(JSON.stringify(m)) };
};

// ---- mutations ----
const PATHS = [
  'inventory', 'inventory.inv_contents', 'inventory.inv_contents.data', 'inventory.bag_contents', 'inventory.bag_contents.talisman_bag',
  'inventory.backpack_contents', 'inventory.inv_armor', 'loadout', 'loadout.armor', 'loadout.equipment',
  'dungeons', 'dungeons.dungeon_types', 'dungeons.dungeon_types.catacombs.experience', 'dungeons.player_classes', 'dungeons.player_classes.archer',
  'dungeons.player_classes.mage.experience', 'dungeons.secrets',
  'player_data', 'player_data.experience', 'player_data.experience.SKILL_COMBAT', 'player_data.experience.SKILL_FORAGING', 'player_data.perks', 'player_data.temp_stat_buffs',
  'slayer', 'slayer.slayer_bosses', 'slayer.slayer_bosses.wolf', 'slayer.slayer_bosses.wolf.xp', 'slayer.slayer_bosses.zombie.xp',
  'pets_data', 'pets_data.pets',
  'accessory_bag_storage', 'accessory_bag_storage.selected_power', 'accessory_bag_storage.unlocked_powers', 'accessory_bag_storage.tuning',
  'accessory_bag_storage.tuning.slot_0', 'accessory_bag_storage.highest_magical_power',
  'attributes', 'attributes.stacks', 'rift', 'rift.access.consumed_prism', 'leveling', 'leveling.experience', 'leveling.bop_bonus',
  'foraging.songs.harp', 'foraging.songs', 'quests.harp_quest', 'collection', 'collection.GOLD_INGOT', 'profile', 'profile.bank_account', 'perks',
  '@banking', '@banking.balance', '@cute_name',
];
const VALUES = [
  ['del'], ['set', null], ['set', 0], ['set', 'x'], ['set', '12'], ['set', []], ['set', {}], ['set', true], ['set', false], ['set', -5], ['set', 1e9], ['set', { a: 1 }], ['set', [1, 2]], ['set', ''],
];
function setPath(root, p, op, value) {
  const keys = p.split('.');
  let cur = root;
  for (let i = 0; i < keys.length - 1; i++) {
    if (cur[keys[i]] == null || typeof cur[keys[i]] !== 'object') { if (op === 'del') return; cur[keys[i]] = {}; }
    cur = cur[keys[i]];
  }
  const last = keys[keys.length - 1];
  if (op === 'del') delete cur[last]; else cur[last] = structuredClone(value);
}
const muts = [];
const extra = [
  // hand-written shapes
  ['tuning-shapes', 'accessory_bag_storage.tuning.slot_0', { health: '5', strength: 10, critical_damage: [3], critical_chance: 0, bogus: 4, intelligence: -3, speed: 'abc', attack_speed: 7.5 }],
  ['tuning-zero', 'accessory_bag_storage.tuning.slot_0', { strength: 0, health: 0 }],
  ['tuning-arr', 'accessory_bag_storage.tuning.slot_0', [5, 6]],
  ['cakes', 'player_data.temp_stat_buffs', [{ stat_id: 'strength', amount: 5, expire_at: 9e15 }, { stat_id: 'critical_damage', amount: '7', expire_at: 1 }, { stat_id: 'intelligence', amount: 'x' }, { stat_id: 'walk_speed', amount: 3, expire_at: null }, { stat_id: 'nope', amount: 3 }, { stat_id: 5, amount: 3 }, { stat_id: 'critical_chance', amount: 2, expire_at: 'abc' }]],
  ['cakes-obj', 'player_data.temp_stat_buffs', { a: { stat_id: 'strength', amount: 5 } }],
  ['pets-shapes', 'pets_data.pets', [{ type: 'ENDER_DRAGON', tier: 'LEGENDARY' }, { type: 'WITHER', tier: 'EPIC', heldItem: null }, { type: 'WITHER', tier: 'EPIC', heldItem: 'PET_ITEM_X' }, { type: 5, tier: true }, {}]],
  ['pets-null-el', 'pets_data.pets', [{ type: 'A', tier: 'B' }, null]],
  ['power-case', 'accessory_bag_storage.selected_power', 'SIGHTED'],
  ['powers-mixed', 'accessory_bag_storage.unlocked_powers', ['Sighted', 5, null, 'ÀB', true, ['a', 'b'], { a: 1 }]],
  ['stacks-str', 'attributes.stacks', { ARACHNO: '40', ENDERMAN: 'x', LIZARD_KING: 3 }],
  ['harp-both', 'foraging.songs.harp', { song_hymn_joy_perfect_completions: null, song_minuet_perfect_completions: '2', song_brahms_perfect_completions: 0 }],
  ['harp-quest', 'quests.harp_quest', { song_hymn_joy_perfect_completions: 1, song_frere_jacques_perfect_completions: 'x', song_jeopardy_perfect_completions: [1] }],
  ['skills-str', 'player_data.experience', { SKILL_COMBAT: '5000000', SKILL_FORAGING: null, SKILL_ALCHEMY: 'x', SKILL_X: 10, OTHER: 5, SKILL_: 1 }],
  ['slayer-strs', 'slayer.slayer_bosses', { wolf: { xp: '2000' }, zombie: { xp: null }, spider: {}, enderman: 5 }],
  ['perks-mixed', 'player_data.perks', { a: 1, b: 'x', c: true, d: null, e: [1], f: 2.5 }],
  ['perks-legacy', 'perks', { a: 3 }],
  ['collection-mixed', 'collection', { GOLD_INGOT: '5', LOG: 'x', COAL: null, X: [1] }],
  ['coop', '@members', null], // filled below
  ['bank-both', '@banking.balance', '1000'],
  ['bank-nan', '@banking.balance', 'abc'],
  ['bp-weird', 'inventory.backpack_contents', { 1: { type: 0, data: 'AAAA' }, 0: null, x: { data: null } }],
  ['bop', 'leveling.bop_bonus', 'EXTRA_INFLICTION'],
  ['bop-num', 'leveling.bop_bonus', 5],
  ['lvl-neg', 'leveling.experience', -50],
  ['lvl-str', 'leveling.experience', '12345'],
  ['cata-str', 'dungeons.dungeon_types.catacombs.experience', '5000000'],
  ['cata-huge', 'dungeons.dungeon_types.catacombs.experience', 1e15],
  ['cata-neg', 'dungeons.dungeon_types.catacombs.experience', -5],
  ['secrets-str', 'dungeons.secrets', '77'],
  ['hmp-str', 'accessory_bag_storage.highest_magical_power', '800'],
  ['prism-str', 'rift.access.consumed_prism', 'true'],
];
for (const base of BASES) {
  const { uuid, profile } = readJson(path.join(TD, 'profiles', `${base}.json`));
  const add = (id, p, op, value) => {
    const prof = structuredClone(profile);
    const target = p.startsWith('@') ? prof : prof.members[uuid];
    setPath(target, p.startsWith('@') ? p.slice(1) : p, op, value);
    const r = reduce(run(prof, uuid));
    muts.push({ id: `${base}:${id}`, base, path: p, op, value, expected: r });
  };
  add('identity', 'zz.zz', 'del');
  for (const p of PATHS) for (const [op, v] of VALUES) add(`${p}:${op}:${JSON.stringify(v)}`, p, op, v);
  for (const [name, p, v] of extra) if (name !== 'coop') add(name, p, 'set', v);
  // other members
  const others = Object.keys(profile.members).filter((k) => k !== uuid);
  const co = (id, f) => {
    const prof = structuredClone(profile);
    f(prof);
    muts.push({ id: `${base}:${id}`, base, path: '@custom', op: id, expected: reduce(run(prof, uuid)) });
  };
  if (others.length) {
    const o = others[0];
    co('other-left', (p) => { p.members[o].profile = { ...(p.members[o].profile ?? {}), deletion_notice: { timestamp: 1 } }; });
    co('other-unconfirmed', (p) => { p.members[o].coop_invitation = { confirmed: false }; });
    co('other-unconfirmed2', (p) => { p.members[o].profile = { ...(p.members[o].profile ?? {}), coop_invitation: { confirmed: false } }; });
    co('other-confirmed-true', (p) => { p.members[o].coop_invitation = { confirmed: true }; });
    co('other-no-collection', (p) => { delete p.members[o].collection; });
    co('other-collection-weird', (p) => { p.members[o].collection = { GOLD_INGOT: 'x', COAL: '7', Z: null }; });
    co('other-null', (p) => { p.members[o] = null; });
    co('solo-bank-personal', (p) => { for (const k of others) delete p.members[k]; p.members[uuid].profile = { bank_account: 500 }; delete p.banking; });
    co('solo-bank-both', (p) => { for (const k of others) delete p.members[k]; p.members[uuid].profile = { bank_account: 500 }; p.banking = { balance: 900 }; });
    co('coop-bank-personal', (p) => { p.members[uuid].profile = { bank_account: 500 }; delete p.banking; });
  }
  co('no-members', (p) => { delete p.members; });
  co('wrong-uuid', (p) => { p.members = { other: p.members[uuid] }; });
  co('member-str', (p) => { p.members[uuid] = 'x'; });
  co('member-empty', (p) => { p.members[uuid] = {}; });
  co('profile-null-fields', (p) => { p.banking = null; p.members[uuid].profile = null; });
  co('member-keys-numeric', (p) => { p.members[uuid].inventory.backpack_contents = { 10: p.members[uuid].inventory.backpack_contents?.['0'] ?? {}, 2: p.members[uuid].inventory.backpack_contents?.['1'] ?? {} }; });
}

// ---- items ----
const T = nbt;
const ea = (obj) => T.comp(obj);
const item = (extra, { display = true } = {}) => T.comp({
  Count: T.byte(1), id: T.short(5), Damage: T.short(0),
  tag: T.comp({ ...(display ? { display: T.comp({ Name: T.string('§aName') }) } : {}), ...(extra ? { ExtraAttributes: extra } : {}) }),
});
const S = T.string, I = T.int, B = T.byte, L = T.long, D = T.double, F = T.float, SH = T.short;
const list = (type, arr) => ({ type: 'list', value: { type, value: arr } });
const strs = (a) => list('string', a);
const itemsCases = {
  noEA: item(null),
  emptyItem: T.comp({}),
  eaNoId: item(ea({ upgrade_level: I(3) })),
  eaEmptyId: item(ea({ id: S('') })),
  eaIdInt: item(ea({ id: I(5) })),
  plain: item(ea({ id: S('HYPERION') })),
  full: item(ea({
    id: S('HYPERION'), upgrade_level: I(7), rarity_upgrades: B(1), hot_potato_count: B(15), art_of_war_count: B(1), modifier: S('heroic'),
    enchantments: T.comp({ sharpness: I(7), ultimate_wise: I(5), PROSECUTE: I(6), '10': I(1), '2': I(2) }),
    gems: T.comp({ JASPER_0: S('PERFECT'), COMBAT_0: T.comp({ quality: S('FLAWLESS') }), COMBAT_0_gem: S('SAPPHIRE'), unlocked_slots: strs(['COMBAT_0', 'JASPER_0']), ruby_1: S('FINE'), NOQ_2: T.comp({}), X_1_2: S('ROUGH'), 'Y_': S('FINE'), Z_a1: S('FINE'), num_3: I(4) }),
    talisman_enrichment: S('attack_speed'), ability_scroll: strs(['IMPLOSION_SCROLL', 'WITHER_SHIELD_SCROLL']), item_tier: I(7), baseStatBoostPercentage: I(50),
  })),
  starsDungeonLevel: item(ea({ id: S('HYPERION'), dungeon_item_level: I(4) })),
  starsBoth: item(ea({ id: S('HYPERION'), upgrade_level: I(2), dungeon_item_level: I(4) })),
  starsStr: item(ea({ id: S('HYPERION'), upgrade_level: S('3') })),
  starsStrBad: item(ea({ id: S('HYPERION'), upgrade_level: S('abc') })),
  starsFloat: item(ea({ id: S('HYPERION'), upgrade_level: D(3.5) })),
  starsNeg: item(ea({ id: S('HYPERION'), upgrade_level: I(-2) })),
  starsBig: item(ea({ id: S('HYPERION'), upgrade_level: I(20) })),
  starsMaster3: item(ea({ id: S('HYPERION'), upgrade_level: I(8) })),
  starsLong: item(ea({ id: S('HYPERION'), upgrade_level: L([0, 5]) })),
  starsList: item(ea({ id: S('HYPERION'), upgrade_level: list('int', [5]) })),
  dungeonItem: item(ea({ id: S('ASPECT_OF_THE_END'), dungeon_item: B(1) })),
  dungeonItem0: item(ea({ id: S('ASPECT_OF_THE_END'), dungeon_item: B(0) })),
  recombStr: item(ea({ id: S('HYPERION'), rarity_upgrades: S('1') })),
  recombNeg: item(ea({ id: S('HYPERION'), rarity_upgrades: I(-1) })),
  hpbFloat: item(ea({ id: S('HYPERION'), hot_potato_count: D(5.9) })),
  hpbStr: item(ea({ id: S('HYPERION'), hot_potato_count: S('7') })),
  aow0: item(ea({ id: S('HYPERION'), art_of_war_count: I(0) })),
  book1: item(ea({ id: S('ENCHANTED_BOOK'), enchantments: T.comp({ ultimate_wise: I(5) }) })),
  book1Upper: item(ea({ id: S('ENCHANTED_BOOK'), enchantments: T.comp({ ULTIMATE_WISE: I(5) }) })),
  book2: item(ea({ id: S('ENCHANTED_BOOK'), enchantments: T.comp({ ultimate_wise: I(5), sharpness: I(7) }) })),
  book0: item(ea({ id: S('ENCHANTED_BOOK'), enchantments: T.comp({}) })),
  bookNone: item(ea({ id: S('ENCHANTED_BOOK') })),
  bookLong: item(ea({ id: S('ENCHANTED_BOOK'), enchantments: T.comp({ ultimate_wise: L([0, 5]) }) })),
  bookDouble: item(ea({ id: S('ENCHANTED_BOOK'), enchantments: T.comp({ ultimate_wise: D(5.5) }) })),
  bookStr: item(ea({ id: S('ENCHANTED_BOOK'), enchantments: T.comp({ ultimate_wise: S('5') }) })),
  enchList: item(ea({ id: S('HYPERION'), enchantments: list('int', [1, 2]) })),
  enchLong: item(ea({ id: S('HYPERION'), enchantments: T.comp({ sharpness: L([0, 7]) }) })),
  enchDouble: item(ea({ id: S('HYPERION'), enchantments: T.comp({ sharpness: D(7.5) }) })),
  enchNeg: item(ea({ id: S('HYPERION'), enchantments: T.comp({ sharpness: I(-1) }) })),
  enchStr: item(ea({ id: S('HYPERION'), enchantments: T.comp({ sharpness: S('7') }) })),
  petJson: item(ea({ id: S('PET'), petInfo: S('{"type":"ENDER_DRAGON","tier":"LEGENDARY","exp":25353230,"heldItem":"PET_ITEM_TIER_BOOST","candyUsed":0}') })),
  petJsonNoTier: item(ea({ id: S('PET'), petInfo: S('{"type":"ENDER_DRAGON","exp":5}') })),
  petJsonBad: item(ea({ id: S('PET'), petInfo: S('{bad') })),
  petJsonEmpty: item(ea({ id: S('PET'), petInfo: S('') })),
  petJsonNull: item(ea({ id: S('PET'), petInfo: S('null') })),
  petJsonArr: item(ea({ id: S('PET'), petInfo: S('[1,2]') })),
  petJsonNum: item(ea({ id: S('PET'), petInfo: S('5') })),
  petJsonStr: item(ea({ id: S('PET'), petInfo: S('"abc"') })),
  petJsonExpStr: item(ea({ id: S('PET'), petInfo: S('{"type":"WITHER","tier":"EPIC","exp":"500"}') })),
  petJsonTypeNum: item(ea({ id: S('PET'), petInfo: S('{"type":5,"tier":"EPIC"}') })),
  petJsonHeldNull: item(ea({ id: S('PET'), petInfo: S('{"type":"WITHER","tier":"EPIC","exp":1,"heldItem":null}') })),
  petComp: item(ea({ id: S('PET'), petInfo: T.comp({ type: S('WITHER'), tier: S('EPIC'), exp: D(5000), heldItem: S('X') }) })),
  petCompLong: item(ea({ id: S('PET'), petInfo: T.comp({ type: S('WITHER'), tier: S('EPIC'), exp: L([0, 5]) }) })),
  petNone: item(ea({ id: S('PET') })),
  notPetInfo: item(ea({ id: S('HYPERION'), petInfo: S('{"type":"WITHER","tier":"EPIC"}') })),
  tier: item(ea({ id: S('HYPERION'), item_tier: I(5), baseStatBoostPercentage: I(30) })),
  tierStr: item(ea({ id: S('HYPERION'), item_tier: S('5'), baseStatBoostPercentage: S('45') })),
  tierNoBoost: item(ea({ id: S('HYPERION'), item_tier: I(5) })),
  tierZero: item(ea({ id: S('HYPERION'), item_tier: I(0) })),
  tierBadBoost: item(ea({ id: S('HYPERION'), item_tier: I(5), baseStatBoostPercentage: S('abc') })),
  tierDouble: item(ea({ id: S('HYPERION'), item_tier: D(6.5), baseStatBoostPercentage: D(12.5) })),
  tierLong: item(ea({ id: S('HYPERION'), item_tier: L([0, 3]) })),
  gemsStr: item(ea({ id: S('HYPERION'), gems: S('x') })),
  gemsList: item(ea({ id: S('HYPERION'), gems: list('string', ['a']) })),
  gemsEmpty: item(ea({ id: S('HYPERION'), gems: T.comp({}) })),
  gemsGemFirst: item(ea({ id: S('HYPERION'), gems: T.comp({ A_0_gem: S('RUBY'), A_0: S('FINE'), B_1: T.comp({ quality: S('FINE'), uuid: S('u') }), unlocked_slots: strs([]) }) })),
  gemsUnlockedInts: item(ea({ id: S('HYPERION'), gems: T.comp({ unlocked_slots: list('int', [1, 2]) }) })),
  gemsUnlockedNotList: item(ea({ id: S('HYPERION'), gems: T.comp({ unlocked_slots: S('x') }) })),
  gemsQualityNum: item(ea({ id: S('HYPERION'), gems: T.comp({ A_0: I(5), B_0: T.comp({ quality: I(5) }), C_0: T.comp({ quality: S('') }), D_0: S(''), E_0: T.comp({ quality: list('string', ['a']) }) }) })),
  gemsGemNonStr: item(ea({ id: S('HYPERION'), gems: T.comp({ A_0: S('FINE'), A_0_gem: I(3) }) })),
  scrollsLists: item(ea({ id: S('HYPERION'), ability_scroll: list('int', [1]) })),
  scrollsStr: item(ea({ id: S('HYPERION'), ability_scroll: S('X') })),
  scrollsEmpty: item(ea({ id: S('HYPERION'), ability_scroll: strs([]) })),
  modNum: item(ea({ id: S('HYPERION'), modifier: I(5) })),
  enrichNum: item(ea({ id: S('HYPERION'), talisman_enrichment: I(5) })),
  unknownId: item(ea({ id: S('TOTALLY_UNKNOWN_ITEM'), upgrade_level: I(2) })),
  nameFmt: item(ea({ id: S('NECRON_HELMET') })),
  unicodeId: item(ea({ id: S('ÄÖ_ITEM'), enchantments: T.comp({ 'ÄB': I(2) }) })),
  eaDup: item(ea({ id: S('HYPERION'), upgrade_level: I(2) })),
  eaNotComp: item(S('x')),
  tagNotComp: T.comp({ Count: B(1), tag: S('x') }),
  bytesArrays: item(ea({ id: S('HYPERION'), ba: { type: 'byteArray', value: [1, 2, 3] }, ia: { type: 'intArray', value: [1, 2] }, la: { type: 'longArray', value: [[0, 1]] }, fl: F(1.5), sh: SH(3) })),
  nested: item(ea({ id: S('HYPERION'), a: T.comp({ b: T.comp({ c: list('compound', [{ x: I(1) }]) }) }) })),
};
// Build one inventory blob per case (a single item), plus combined blobs.
const gz = (buf) => zlib.gzipSync(buf).toString('base64');
const blobOf = (items) => gz(nbt.writeUncompressed(T.comp({ i: list('compound', items.map((c) => c.value)) }), 'big'));
const baseProf = readJson(path.join(TD, 'profiles', 'Ben_MC.json'));
const slim = (data) => {
  const prof = structuredClone(baseProf.profile);
  const m = prof.members[baseProf.uuid];
  m.inventory = { inv_contents: { type: 0, data } };
  delete m.loadout;
  return prof;
};
const items = [];
const addItem = (id, data) => {
  const prof = slim(data);
  items.push({ id, data, expected: run(prof, baseProf.uuid) });
};
for (const [k, v] of Object.entries(itemsCases)) addItem(`one:${k}`, blobOf([v]));
// the full set in one blob, in different orders
const KNOWN = new Set(['eaIdInt', 'starsStrBad', 'starsFloat', 'starsLong', 'hpbFloat', 'bookLong', 'enchList', 'enchLong', 'enchDouble', 'enchStr', 'petJsonArr', 'petJsonNum', 'petJsonStr', 'petJsonExpStr', 'petJsonTypeNum', 'petCompLong', 'tierLong', 'gemsList', 'gemsUnlockedInts', 'gemsQualityNum', 'gemsGemNonStr', 'scrollsLists', 'modNum', 'enrichNum']);
const all = Object.entries(itemsCases).filter(([k]) => !KNOWN.has(k)).map(([, v]) => v);
addItem('all', blobOf(all));
addItem('all-rev', blobOf([...all].reverse()));
// root variants
const root = (c) => gz(nbt.writeUncompressed(c, 'big'));
addItem('root-no-i', root(T.comp({ x: I(1) })));
addItem('root-i-ints', root(T.comp({ i: list('int', [1, 2]) })));
addItem('root-i-empty', root(T.comp({ i: list('end', []) })));
addItem('root-i-int', root(T.comp({ i: I(5) })));
addItem('root-i-twice', gz(Buffer.from(nbt.writeUncompressed(T.comp({ i: list('compound', [itemsCases.plain.value]) }), 'big'))));
addItem('root-i-nested-list', root(T.comp({ i: list('list', [{ type: 'compound', value: [itemsCases.plain.value] }]) })));
addItem('root-i-emptycomp', root(T.comp({ i: list('compound', [{}, itemsCases.plain.value, {}]) })));
// base64 / gzip variants
const good = blobOf([itemsCases.plain]);
const raw = Buffer.from(good, 'base64');
addItem('b64-newlines', good.replace(/(.{20})/g, '$1\n'));
addItem('b64-spaces', good.replace(/(.{20})/g, '$1 '));
addItem('b64-nopad', good.replace(/=+$/, ''));
addItem('b64-urlsafe', good.replace(/\+/g, '-').replace(/\//g, '_'));
addItem('b64-trailing-junk', good + '!!!');
addItem('b64-multi-member', Buffer.concat([raw, zlib.gzipSync(Buffer.from('zzzz'))]).toString('base64'));
addItem('b64-trailing-bytes', Buffer.concat([raw, Buffer.from([1, 2, 3])]).toString('base64'));
addItem('b64-truncated', raw.subarray(0, raw.length - 8).toString('base64'));
addItem('b64-garbage', 'AAAAAAAAAAAA');
addItem('b64-notgzip', Buffer.from('hello world, this is not gzip').toString('base64'));
addItem('nbt-truncated', gz(nbt.writeUncompressed(T.comp({ i: list('compound', [itemsCases.full.value]) }), 'big').subarray(0, 120)));
addItem('nbt-unknown-tag', gz(Buffer.from([10, 0, 0, 99, 0, 1, 120, 0])));
addItem('nbt-root-not-comp', gz(Buffer.from([8, 0, 0, 0, 1, 65])));
addItem('data-number', 5);
addItem('data-obj', { a: 1 });
addItem('data-true', true);
addItem('data-empty', '');
// separate data in several places
{
  const prof = slim(blobOf([itemsCases.plain]));
  const m = prof.members[baseProf.uuid];
  m.inventory.inv_armor = { data: blobOf([itemsCases.full]) };
  m.inventory.backpack_contents = { 0: { data: blobOf([itemsCases.petJson]) }, 1: { data: blobOf([itemsCases.book1]) } };
  m.loadout = { armor: { 0: { 0: { data: blobOf([itemsCases.tier]) }, x: { data: blobOf([itemsCases.recombNeg]) } } }, equipment: { a: { b: { data: blobOf([itemsCases.nameFmt]) } } } };
  items.push({ id: 'multi-blob', prof, expected: run(prof, baseProf.uuid) });
}
{
  const prof = slim(blobOf([itemsCases.plain]));
  const m = prof.members[baseProf.uuid];
  m.inventory = { backpack_contents: { 1: { data: blobOf([itemsCases.full]) } } };
  m.loadout = null;
  items.push({ id: 'only-backpack', prof, expected: run(prof, baseProf.uuid) });
  const prof2 = structuredClone(prof);
  prof2.members[baseProf.uuid].inventory = null;
  prof2.members[baseProf.uuid].loadout = { armor: { 0: { 0: { data: blobOf([itemsCases.full]) } } } };
  items.push({ id: 'only-loadout', prof: prof2, expected: run(prof2, baseProf.uuid) });
  const prof3 = structuredClone(prof);
  prof3.members[baseProf.uuid].inventory = { inv_contents: { data: null }, ender_chest_contents: { data: '' }, bag_contents: { talisman_bag: { data: blobOf([itemsCases.full]) } } };
  items.push({ id: 'only-talisman', prof: prof3, expected: run(prof3, baseProf.uuid) });
  const prof4 = structuredClone(prof);
  prof4.members[baseProf.uuid].inventory = { inv_contents: { data: null } };
  items.push({ id: 'all-null-data', prof: prof4, expected: run(prof4, baseProf.uuid) });
  const prof5 = structuredClone(prof);
  prof5.members[baseProf.uuid].inventory = [];
  prof5.members[baseProf.uuid].loadout = { armor: { 0: { 0: { data: blobOf([itemsCases.full]) } } } };
  items.push({ id: 'inventory-array-with-loadout', prof: prof5, expected: run(prof5, baseProf.uuid) });
  const prof6 = structuredClone(prof);
  prof6.members[baseProf.uuid].inventory = { inv_contents: { data: blobOf([itemsCases.full]) }, backpack_contents: 'abc' };
  items.push({ id: 'backpack-string', prof: prof6, expected: run(prof6, baseProf.uuid) });
  const prof7 = structuredClone(prof);
  prof7.members[baseProf.uuid].inventory = { inv_contents: { data: blobOf([itemsCases.full]) } };
  prof7.members[baseProf.uuid].loadout = { armor: ['a'], equipment: { x: ['b'] } };
  items.push({ id: 'loadout-weird', prof: prof7, expected: run(prof7, baseProf.uuid) });
}
writeJson(path.join(OUT, 'mutations.json'), { uuids: Object.fromEntries(BASES.map((b) => [b, readJson(path.join(TD, 'profiles', `${b}.json`)).uuid])), cases: muts });
writeJson(path.join(OUT, 'items.json'), { uuid: baseProf.uuid, base: 'Ben_MC', cases: items });
const errs = [...muts, ...items].filter((c) => c.expected.error).length;
console.log(`mutations ${muts.length}, items ${items.length}, JS errors ${errs}`);
