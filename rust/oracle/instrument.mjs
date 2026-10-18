// Builds testdata/js-instrumented/src: a copy of the bot's src (mounted read-only at /app/src) with a few
// additive hooks for the oracle. The bot's own src is never edited.
//   oracle/run.sh instrument.mjs
// Then run a script against the copy:  JS_SRC=$PWD/testdata/js-instrumented/src oracle/run.sh states.mjs
//
// Edits (each must match exactly once, or the script fails; the JS files have CRLF line endings):
// - src/calc/upgrades.js greedyPlan candidatesFor: after the bundle scores, calls
//   globalThis.__oracleState?.({ round, loadout, current, ups, banned, scored, bundles, bundleScores, ctx, weights,
//   prices }) once per planner state (candidatesFor runs once per state: state.candidates ??= ...). Nothing changes
//   when the hook is unset.
// - src/calc/upgrades.js planUpgrades: globalThis.__oracleRound = 0 before the first greedyPlan, = round + 1 before
//   each re-plan, so the hook knows the re-plan round.
// - src/commands/dps.js: `function buttons(id, s)` is exported (embeds.mjs builds the components with it).
import fs from 'node:fs';
import path from 'node:path';
import { TD } from './common.mjs';

const FROM = process.env.JS_SRC_ORIG ?? '/app/src';
const TO = path.join(TD, 'js-instrumented', 'src');

const EDITS = {
  'calc/upgrades.js': [
    ['    const bundleScores = scoreUpgrades(bundles, loadout, current, ctx, weights, banned).filter((x) => !x.shortlistOnly);\r\n',
      '    const bundleScores = scoreUpgrades(bundles, loadout, current, ctx, weights, banned).filter((x) => !x.shortlistOnly);\r\n'
      + '    // ORACLE: report each state the first time it is scored (rust/oracle/states.mjs).\r\n'
      + '    globalThis.__oracleState?.({ round: globalThis.__oracleRound, loadout, current, ups, banned, scored, bundles, bundleScores, ctx, weights, prices });\r\n'],
    ['  let plan = await greedyPlan(startLoadout, ctx, prices, weights, limit, rules, scorer, null, isCancelled);\r\n',
      '  globalThis.__oracleRound = 0; // ORACLE: re-plan round of the states scored next\r\n'
      + '  let plan = await greedyPlan(startLoadout, ctx, prices, weights, limit, rules, scorer, null, isCancelled);\r\n'],
    ['    plan = await greedyPlan(startLoadout, ctx, prices, weights, limit, rules, scorer, plan, isCancelled);\r\n',
      '    globalThis.__oracleRound = round + 1; // ORACLE\r\n'
      + '    plan = await greedyPlan(startLoadout, ctx, prices, weights, limit, rules, scorer, plan, isCancelled);\r\n'],
  ],
  'commands/dps.js': [
    ['\r\nfunction buttons(id, s) {\r\n', '\r\nexport function buttons(id, s) { // ORACLE: exported for rust/oracle/embeds.mjs\r\n'],
  ],
};

fs.rmSync(TO, { recursive: true, force: true });
fs.mkdirSync(path.dirname(TO), { recursive: true });
fs.cpSync(FROM, TO, { recursive: true });
for (const [file, edits] of Object.entries(EDITS)) {
  const p = path.join(TO, file);
  let text = fs.readFileSync(p, 'utf8');
  for (const [from, to] of edits) {
    const n = text.split(from).length - 1;
    if (n !== 1) throw new Error(`${file}: expected 1 match, found ${n}: ${JSON.stringify(from.slice(0, 80))}`);
    text = text.replace(from, () => to);
  }
  fs.writeFileSync(p, text);
  console.log(`${file}: ${edits.length} edits`);
}
console.log(`instrumented copy: ${TO}`);
