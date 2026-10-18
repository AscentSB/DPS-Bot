// testdata/embeds/<caseId>.json: what the bot's /dps message shows (buildEmbed + buttons from src/commands/dps.js)
// for every page of the coins and the profile view of a session built from golden data, plus
// testdata/command.json (the /dps registration body) and testdata/escape-markdown.json. Needs the instrumented
// copy (instrument.mjs exports buttons()) and golden/ (golden-all.sh):
//   JS_SRC=$PWD/testdata/js-instrumented/src oracle/run.sh embeds.mjs [caseId ...]
// Without ids it runs EMBED_CASES below.
//
// A session is built the way execute() and handleButton() build it: { ...job (cls, target, mode, floorMode,
// blessings, anyChest), upgradeMode, name, member, coins, profile, view, page } with
//   name    : the profile's IGN (the file name, which is the Mojang name), so escapeMarkdown matters for names with '_';
//   member  : { cataLevel } from members/<name>.json (the only member field buildEmbed / buttons read);
//   coins   : { [planKey]: golden/<caseId>.json without "ms" } (planKey = upgradeMode + ':anyChest' when anyChest);
//   profile : the profile menu handleButton would show: runDps({ cls, target, mode, floorMode, blessings, anyChest,
//             member, menu: 'profile' }).profile. It is the matching golden profile case when cases.json has one
//             (same cls, target, mode, floorMode, blessings, anyChest), else computed here the same way (cheap).
// One entry per view and page: { id, session, embed: buildEmbed(s).toJSON(), components: buttons(id, s).map(r =>
// r.toJSON()) }, coins pages first. Each file is { case, profileSource: <profile caseId> | 'computed', entries }.
// The session holds everything buildEmbed and buttons read, so an entry can be checked on its own.
//
// embeds/synthetic__empty-plan.json: one real session with its plan steps and profile rows emptied (a maxed profile:
// "No priced upgrade ..." / "No profile upgrade ..."), same format, case 'synthetic'.
// command.json: data.toJSON() of the slash command. escape-markdown.json: [{ in, out }] = discord.js
// escapeMarkdown(in) for every profile name and every string of 'a' / '_' / '1' up to length 7 (IGNs are letters,
// digits and '_'; only '_' is markdown).
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { MEMBERS_DIR, SRC, TD, readJson, writeJson } from './common.mjs';
import { loadBot } from './harness.mjs';

export const EMBED_CASES = [
  'Ben_MC__arch-lcm-master-all-coins',
  'Ben_MC__mage-rcm-master-all-coins',
  'Ben_MC__bers-lcm-master-all-coins',
  'Ben_MC__mage-lcm-master-max-coins',
  'Ben_MC__arch-lcm-master-max-coins',
  'Ben_MC__arch-lcm-master-all-anychest-coins',
  'Ben_MC__arch-lcm-master-all-wither-coins',
  'Ben_MC__arch-lcm-master-all-dragon-coins',
  'Ben_MC__arch-lcm-master-all-b19.12.0-coins',
  'Ben_MC__mage-lcm-normal-all-coins',
  'MR_R3dh3ad__arch-lcm-master-all-coins',
  'MR_R3dh3ad__mage-lcm-master-max-coins',
  'MR_R3dh3ad__bers-lcm-master-all-anychest-coins',
  'MR_R3dh3ad__mage-lcm-normal-all-coins',
  'recticc__arch-lcm-master-all-coins',
  'recticc__mage-lcm-master-all-coins',
  'recticc__mage-rcm-master-all-coins',
  'recticc__bers-lcm-master-max-coins',
  'Toy_Mads__arch-lcm-master-all-coins',
  'J_W0LF__mage-lcm-normal-all-coins',
  'spang_b__arch-lcm-master-all-coins',
  'Vandmelon____mage-rcm-master-all-coins',
  'zRsk___arch-lcm-master-all-coins',
  'zRsk___mage-rcm-master-all-coins',
  'Scraamss__arch-lcm-master-all-coins',
  'Scraamss__mage-lcm-master-all-b19.12.0-coins',
  'PaccoTullo__mage-rcm-master-all-coins',
];

const bot = await loadBot();
const dps = await import(`${SRC}/commands/dps.js`);
if (typeof dps.buttons !== 'function') throw new Error('buttons() is not exported: run with JS_SRC=<instrumented copy> (instrument.mjs).');
// discord.js as the bot resolves it (from /app/src up to /app/node_modules; this directory has no node_modules).
const { escapeMarkdown } = createRequire(`${SRC}/commands/dps.js`)('discord.js');
const { buildEmbed, buttons, pageCount, data } = dps;

const ids = process.argv.length > 2 ? process.argv.slice(2) : EMBED_CASES;
const cases = readJson(path.join(TD, 'cases.json'));
const byId = new Map(cases.map((c) => [c.id, c]));
const EMBEDS_DIR = path.join(TD, 'embeds');
fs.mkdirSync(EMBEDS_DIR, { recursive: true });

const golden = (id) => {
  const { ms, ...g } = readJson(path.join(TD, 'golden', `${id}.json`));
  return g;
};
const sameBlessings = (a, b) => a.power === b.power && a.wisdom === b.wisdom && a.time === b.time;
const planKey = (s) => `${s.upgradeMode}${s.anyChest ? ':anyChest' : ''}`;

// The profile menu handleButton computes for this session's job, from cases.json when it has that job.
async function profileFor(c, member) {
  const j = c.job;
  const match = cases.find((p) => p.profile === c.profile && p.job.menu === 'profile' && p.job.cls === j.cls
    && (p.job.target ?? null) === (j.target ?? null) && p.job.mode === j.mode && p.job.floorMode === j.floorMode
    && sameBlessings(p.job.blessings, j.blessings) && Boolean(p.job.anyChest) === Boolean(j.anyChest));
  // handleButton's job: { cls, target, mode, floorMode, blessings, anyChest, member } + menu 'profile' (no
  // upgradeMode, no limit), run in the plan worker with the resource and frozen prices.
  const computed = (await bot.runDps({ cls: j.cls, target: j.target, mode: j.mode, floorMode: j.floorMode, blessings: j.blessings, anyChest: j.anyChest, member, menu: 'profile', resource: bot.snap.resource, prices: bot.prices })).profile;
  if (!match) return { profile: computed, source: 'computed' };
  const g = golden(match.id).profile;
  if (JSON.stringify(g) !== JSON.stringify(computed)) throw new Error(`${c.id}: profile golden ${match.id} differs from the profile menu the bot would compute`);
  return { profile: g, source: match.id };
}

function entriesFor(id, base) {
  const out = [];
  for (const view of ['coins', 'profile']) {
    const s0 = { ...base, view, page: 0 };
    for (let page = 0; page < pageCount(s0); page++) {
      const s = { ...s0, page };
      out.push({ id, session: s, embed: buildEmbed(s).toJSON(), components: buttons(id, s).map((r) => r.toJSON()) });
    }
  }
  return out;
}

let n = 0;
let entryCount = 0;
let firstBase = null;
for (const caseId of ids) {
  const c = byId.get(caseId);
  if (!c) throw new Error(`No case ${caseId}`);
  if (c.job.menu !== 'coins') throw new Error(`${caseId} is not a coin plan`);
  const memberFull = readJson(path.join(MEMBERS_DIR, `${c.profile}.json`));
  const { profile, source } = await profileFor(c, memberFull);
  const j = c.job;
  const base = {
    cls: j.cls, target: j.target ?? null, mode: j.mode, floorMode: j.floorMode, blessings: j.blessings, anyChest: j.anyChest,
    upgradeMode: j.upgradeMode, name: c.profile, member: { cataLevel: memberFull.cataLevel },
  };
  base.coins = { [planKey(base)]: golden(caseId) };
  base.profile = profile;
  firstBase ??= base;
  // Interaction ids are snowflakes; any fixed string works, distinct per file.
  const id = String(1300000000000000000n + BigInt(n));
  const entries = entriesFor(id, base);
  writeJson(path.join(EMBEDS_DIR, `${caseId}.json`), { case: caseId, profileSource: source, entries });
  console.log(`${caseId}: ${entries.length} entries (profile: ${source}), title ${JSON.stringify(entries[0].embed.title)}`);
  n++;
  entryCount += entries.length;
}

// A maxed profile: nothing left to buy or level.
if (firstBase) {
  const key = Object.keys(firstBase.coins)[0];
  const coins = firstBase.coins[key];
  const base = { ...firstBase, coins: { [key]: { ...coins, plan: { ...coins.plan, steps: [], endDamage: coins.plan.startDamage } } }, profile: { ...firstBase.profile, rows: [] } };
  const entries = entriesFor('1299999999999999999', base);
  writeJson(path.join(EMBEDS_DIR, 'synthetic__empty-plan.json'), { case: 'synthetic', profileSource: 'synthetic', entries });
  entryCount += entries.length;
}

writeJson(path.join(TD, 'command.json'), data.toJSON());

const names = new Set(cases.map((c) => c.profile));
const alphabet = ['a', '_', '1'];
let layer = [''];
for (let len = 1; len <= 7; len++) {
  layer = layer.flatMap((p) => alphabet.map((ch) => p + ch));
  for (const s of layer) names.add(s);
}
const vectors = [...names].map((s) => ({ in: s, out: escapeMarkdown(s) }));
writeJson(path.join(TD, 'escape-markdown.json'), vectors);
console.log(`embeds: ${n} cases + synthetic, ${entryCount} entries; command.json; escape-markdown.json (${vectors.length} vectors)`);
