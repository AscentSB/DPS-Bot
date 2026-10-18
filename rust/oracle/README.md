# Oracle: golden data from the JavaScript bot

The scripts here run the JavaScript bot (`../../src`, the reference) and dump what it computes into `../testdata`
(gitignored, local only), so the Rust port can be tested against it. Formats are the ones in
`../PORTING_NOTES.md` ("Test data"); this file adds what each script does and the details of its output.

Local `node` is broken, so every script runs in the `dps-bot:latest` Docker image through `run.sh`:

```sh
cd /home/andrew/Downloads/DPS-Bot/rust
oracle/run.sh snapshot.mjs              # any script: oracle/run.sh <script> [args]
oracle/run.sh --net fetch-profiles.mjs  # --net also mounts ../.env read-only (the Hypixel API key)
JS_SRC=$PWD/testdata/js-instrumented/src oracle/run.sh evals.mjs   # run against a modified copy of src
```

`run.sh` mounts the bot's `src` read-only at `/app/src` (or `$JS_SRC`), this directory read-only at `/oracle` and
`testdata` read-write at `/td`. The container runs as uid 1000, so the files it writes are yours. Scripts find their
paths through `common.mjs` (`TD`, `JS_SRC` environment overrides).

## Pipeline (part 1)

Run in this order; each step reads what the one before wrote. Every step is deterministic: re-running it gives the
same bytes (`fetch-profiles.mjs` aside, which reads live API data).

| step | script | reads | writes |
|---|---|---|---|
| 1 | `snapshot.mjs` | `snapshot.raw.json` | `snapshot.json` |
| 2 | `fetch-profiles.mjs` (`--net`) | `profiles/*.json` (seeds), Hypixel + Mojang APIs | more `profiles/<name>.json` |
| 3 | `members.mjs` | `snapshot.json`, `profiles/*.json` | `members/<name>.json` |
| 4 | `cases.mjs` | `members/*.json` | `cases.json` |
| 5 | `evals.mjs` | `snapshot.json`, `profiles/`, `members/`, `cases.json` | `ctx/<caseId>.json`, `evals/<caseId>.jsonl` |
| 6 | `model-check/` (Rust) | everything above | nothing (a report) |

```sh
oracle/run.sh snapshot.mjs && oracle/run.sh members.mjs && oracle/run.sh cases.mjs && oracle/run.sh evals.mjs
CARGO_TARGET_DIR=target/oracle cargo run --release --manifest-path oracle/model-check/Cargo.toml
```

Steps 3 to 5 take about 20 s in all.

### 1. `snapshot.mjs`: `snapshot.json`

`snapshot.raw.json` (items resource, collections, bazaar and auction prices) plus `"now"`: the raw file's mtime in
ms, used as `Date.now()` for `readMember` (cake buffs). The file is `{"now":<ms>,` followed by the raw file minus
its first `{`, so everything else is byte for byte the raw file (the script checks the parse too).

### 2. `fetch-profiles.mjs [target=28]`: more `profiles/<name>.json`

`{ uuid, profile }` with the selected profile (`profiles.find(p => p.selected) ?? profiles[0]`), like the existing
seed files. Candidates: guild members (`/v2/guild?player=`) of the seed players and their co-op members, interleaved
across guilds. A candidate is kept when its Catacombs level (`cataLevelFromXp` from the bot) is 20 or more and it has
at least one inventory blob (the same list as `inventoryBlobs` in `src/api/hypixel.js`; some blobs missing is fine).
Kept profiles are spread over selected class and Catacombs band (20-34, 35-44, 45-49, 50) with per-bucket caps,
relaxed only if too few candidates fit. Names come from `sessionserver.mojang.com`. At most one request per second per
host; waits on 429 (Retry-After / RateLimit-Reset) and when RateLimit-Remaining runs low; stops at `target` new
profiles or 220 profile requests. The key is read from the mounted `.env` and is never printed or written; profile
contents are never printed (only names, Catacombs levels, classes, blob counts).

### 3. `members.mjs [name ...]`: `members/<name>.json`

`readMember(profile, uuid, resource, collections)` from `src/api/hypixel.js`, with `Date.now = () => snapshot.now`
set before the bot modules load, `resource = new Map(snapshot.items)`, `collections = new Map(snapshot.collections)`.
Written with `JSON.stringify` (undefined fields such as `petProfile.bank` with the bank API off are left out). A
profile `readMember` rejects gets `{ "error": message }` (none of the current profiles do). Warns if a member holds
NaN / Infinity (JSON would write null); none do.

### 4. `cases.mjs`: `cases.json`

`[{ id, profile, job }]`, `job` = `model::DpsJob` (`limit` 40 on every case, profile menus included; the bot passes
none for the profile menu, where it is unused). For every profile with a readable member:

- coin plans (upgradeMode `all`, default blessings 29/14/5, target null, anyChest false): `arch-lcm-master`,
  `bers-lcm-master`, `mage-lcm-master`, `mage-lcm-normal`, `mage-rcm-master`;
- profile menus: `arch-lcm-master`, `mage-lcm-master`.

Variant profiles (the 2 seeds plus 4 others: the lowest and highest Catacombs level and two evenly spaced over the
distinct levels between) also get coin plans with upgradeMode `max` (arch, bers, mage LCM), anyChest (arch, bers;
Catacombs 36+ only), target `wither` and `dragon` (arch), and blessings 19/12/0 (arch, mage LCM). If coin plans
exceed 220, the variant Berserker max plans go first, then the other max plans.

Case id: `<profile>__<cls>-<mode>-<floorMode>-<upgradeMode>[-anychest][-<target>][-b<power>.<wisdom>.<time>]-<menu>`.
Player names can contain `_` (`Vandmelon__`, `zRsk_`), so take the profile from the `profile` field, never by
splitting the id on `__`.

### 5. `evals.mjs [--count 30] [--lines 200] [--min-sample 50] [--dry] [caseId ...]`: `ctx/`, `evals/`

Picks `--count` coin cases (or the ids given) by a greedy, deterministic cover of features: class/set/floor, that
crossed with a Catacombs band (<35, 35-47, 48+), each variant kind (max, anyChest, target, blessings) and the
profile, each worth less every time an already picked case has it. The current pick is 6 cases each of `arch-lcm-
master`, `bers-lcm-master`, `mage-lcm-master`, `mage-lcm-normal`, `mage-rcm-master`, over Catacombs 21 to 50,
covering max mode (arch, bers, mage), anyChest (arch, bers), target wither and dragon, and custom blessings (arch,
mage). `--dry` prints the pick only.

Each case is set up as `runDps` does it (`harness.mjs` `prepareCase`): `makeContext` with the member's fields,
`weightsFor(cls, target, mode, ctx.floorMode)`, `buildLoadout(member, ctx, weights)`, then `ctx.weights = weights`
(as `planUpgrades` sets it). Prices: `priceLookup` over the snapshot's bazaar and auction tables.

- `ctx/<caseId>.json`: the context without `resource` (`serializeCtx`): every `makeContext` field (`blessings` as
  `blessingsWith` returns it, `{ power: { level, stats }, ... }`), `tuning` with the chosen `split`, `weights`.
- `evals/<caseId>.jsonl`: one JSON object per line, `{ kind, label, loadout: L, result: R }`, with
  `R = evaluate(L, ctx, weights)` (`extras` as active extra keys) and L as in the notes (`serializeLoadout`). `kind`
  and `label` are extra keys for debugging:
  - `start`: the start loadout (label `start`), always the first line.
  - `apply`: `up.apply(start)` for a candidate of `availableUpgrades(start, ctx, prices)`, label = its label.
  - `repick`: right after its `apply` line, for a candidate with `repick` whose `apply` gains > 1e-9 (as
    `scoreUpgrades` does): `selectPower(selectPet(next))`.

  Candidates, in candidate-list order: every one in group `pet`, every repick candidate that gains, then an even
  sample of the repick candidates that do not gain (a third of what is left of `--lines`) and of all other candidates,
  for about `--lines` lines and at least `--min-sample` sampled lines per case. Candidates with cost <= 0 are skipped
  (`scoreUpgrades` never scores them). The losing repick candidates are sampled rather than all written because a
  Mage case has over 1,000 of them and a line is about 50 KB (the accessory bag is most of it).

Checks before writing (the script exits 1 and lists them if any fail): every line's L and the ctx JSON are read back
into JS objects (`deserializeLoadout`) and evaluated again, and the result must be the same JSON as R (so L and ctx
hold everything `evaluate()` reads); accessories are keyed by their own id; no NaN / Infinity anywhere; the start
(ctx, loadout and its evaluation) built from the live `readMember` output equals the one built from
`members/<name>.json`.

### 6. `model-check/`: model.rs round trip

A small Rust program (its own workspace; it includes `crates/core/src/model.rs` by path, so it builds while the rest
of dps-core is in progress). It parses every testdata file and round-trips JSON -> model type -> JSON for `Member`,
`DpsJob` (cases), `ItemState` and `Pet` (every item and pet in every eval line), `Tuning`, `Weights`, `PetProfile`,
`ProfileStats` and `Collections` (ctx files), and checks `ItemInfo`, collections and price tables in the snapshot
deserialize. It prints, per field path, fields the model drops, fields it writes where the JS has none, and values
that change. Exit code 1 on a deserialize error or a changed value.

## Pipeline (part 2)

Both scripts need the instrumented JS copy (`run.sh instrument.mjs` once; it writes `testdata/js-instrumented/src`,
which adds the state hook to `candidatesFor` and exports `buttons()` from `commands/dps.js`) and `golden/`
(`golden-all.sh`). Run them with `JS_SRC=$PWD/testdata/js-instrumented/src`.

### 7. `states.mjs` / `states-all.sh [P]`: `states/<caseId>.jsonl` + `.ctx.json`

Every planner state of a coin plan in scoring order, from `runDps` run in-thread (as golden). The line format is in
the header of `states.mjs` and in PORTING_NOTES (`round`, `loadout`, `current`, `banned`, `candidates`, `scored` with
`changes`, `bundles`). `round` 0 is the first greedy plan, k > 0 the k-th re-plan (only states not scored before).
Checks before a file is accepted: the plan equals `golden/<caseId>.json`, ctx identical at every state, and (unless
`--no-verify`) each line rebuilt from its JSON gives the same `current`, `candidates` and `scored`.
`states-all.sh` runs DEFAULT_CASES, P containers at a time (default 6, about 1 GB RAM each for Mage). Logs in
`states/<caseId>.log`.

Object identity: the JS compares gear with `===` (`slotName`, `replaceItem`). `serializeLoadout` therefore adds an
`alias` map, only when non-empty, such as `{ "clearBoots": "armor.BOOTS" }` (this slot is the same object as that
earlier slot; slots visited weapon, rag, armor.*, clearBoots, equipment.*, dragon.*). It matters: a group name is
`BOOTS:gem:COMBAT_0` when clearBoots is armor.BOOTS and `clearBoots:gem:COMBAT_0` for a separate copy. Value equality
of the two items (the Rust port) reproduces the JS here, since the JS only shares an object when it is the same
item. `evals/` was written before `alias` existed and has none; rerunning `evals.mjs` would add it where needed.

### 8. `embeds.mjs`: `embeds/<caseId>.json`, `command.json`, `escape-markdown.json`

See the header of `embeds.mjs`: per case `{ case, profileSource, entries: [{ id, session, embed, components }] }`,
coins pages then profile pages. `synthetic__empty-plan.json` is a maxed profile. `command.json` is the /dps
registration body; `escape-markdown.json` is `[{ in, out }]` for escapeMarkdown.

## Modules

- `common.mjs`: paths (`TD`, `SRC`, testdata dirs), `PLAN_STEPS`, `DEFAULT_BLESSINGS`, `readJson` / `writeJson`,
  `profileNames()` (sorted), `loadSnapshot()` (Maps like the bot holds them), `freezeNow(ms)`.
- `harness.mjs`: `loadBot()` (freezes `Date.now` at snapshot.now, then imports `calc/upgrades.js`, `calc/stats.js`,
  `api/prices.js`, `api/hypixel.js`, `calc/run.js` from `JS_SRC`; returns `{ snap, upgrades, stats, prices,
  readMember, runDps }`) and `prepareCase(bot, member, job)` -> `{ ctx, weights, loadout }`.
- `serialize.mjs` (shared with part 2): `serializeLoadout(l)` -> L, `deserializeLoadout(L)`, `serializeEval(r)` -> R,
  `serializeCtx(ctx)`, `serializeCandidate(up)` -> `{ label, cost, source, unverified?, group?, stacksOn?, repick?,
  armorSlot?, armorId?, bank? }` (the states format), `plain(v)` (Maps -> `[k, v]` arrays, Sets -> arrays, functions
  and undefined dropped), `nonFinitePaths(v)`.

Scripts named `jsfmt.mjs`, `core-*.mjs`, `skyblock-*.mjs` and `bot-*.mjs` belong to the agents porting those crates
and are described in their own headers.

## Current data (2026-10-16)

- `snapshot.json`: now = 1792202153433 (2026-10-17T01:55:53Z); 5,680 items, 90 collections, 1,830 bazaar and
  3,058 auction prices.
- `profiles/`: 30 (2 seeds + 28 from two guilds), 45 MB. Catacombs 21 to 50 (5 under 35, 11 at 50); selected class
  10 mage, 6 archer, 5 berserk, 7 other, plus the seeds. Most have no `wardrobe_contents` blob (inventory API partly
  off or the field moved).
- `members/`: 30, all readable (no `{ error }` file), 5.9 MB.
- `cases.json`: 262 cases, 202 coin plans and 60 profile menus.
- `ctx/`: 30; `evals/`: 30 files, 6,810 lines (30 start, 6,299 apply, 481 repick), 364 MB.

## Part 2 data (2026-10-16)

- `states/`: 15 cases, 541 MB, 666 states (recticc arch family re-plans 7 rounds, 64 states; Mage cases 40 to 64):
  recticc arch/bers/mage lcm master/mage lcm normal/mage rcm/mage max/arch anychest/arch wither/arch b19.12.0
  blessings, Ben_MC arch (null target), arch dragon and mage rcm, Scraamss arch and mage (low Catacombs), tolv mage.
  All verified (plan == golden, rebuilt from JSON).
- `embeds/`: 28 files (27 cases + synthetic), 167 entries, 2.1 MB; names with underscores: Ben_MC, MR_R3dh3ad, J_W0LF,
  Toy_Mads, spang_b, Vandmelon____, zRsk___. Includes all/max/anyChest, wither/dragon targets, a 0-step plan.
- `command.json` 2.7 KB, `escape-markdown.json` 3,309 vectors (120 KB).
