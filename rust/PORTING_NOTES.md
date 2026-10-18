# Rust port of the DPS bot: rules for everyone working on it

The JavaScript bot in `../src` is the **reference**. The Rust port in this workspace must give the **same answers**
(same plan steps, same labels, same embeds), while using as little CPU as possible: the bot runs 24/7 on a small
home server, so CPU time is power draw. RAM is plentiful: trade memory for CPU freely (caches, precomputed tables).

Repo root: `/home/andrew/Downloads/DPS-Bot`. Workspace: `/home/andrew/Downloads/DPS-Bot/rust`.

## Crates and ownership

| crate | what | ports |
|---|---|---|
| `crates/core` (`dps-core`) | shared types (`model.rs`, the contract), static data + config tables, formulas, small pure helpers, JS-compatible formatting | `src/config/*`, `src/data/*.json`, `src/calc/formulas.js`, `src/lib/{pets,drops,requirements}.js`, `cleanName` from `src/lib/nbt.js` |
| `crates/calc` (`dps-calc`) | damage model + planner | `src/calc/stats.js`, `src/calc/upgrades.js`, `src/calc/run.js`, `src/worker/*` (as rayon parallelism) |
| `crates/skyblock` (`dps-skyblock`) | HTTP, Mojang, Hypixel resources/profiles, NBT, readMember, price downloads + cache, NEU data update | `src/api/*`, `src/lib/nbt.js`, `src/worker/priceWorker.js`, `scripts/update-data.mjs` |
| `crates/bot` (`dps-bot`, the binary) | Discord gateway + HTTP interactions, `/dps` command, embeds, buttons, sessions, cooldowns, usage counter, command registration, CLI subcommands | `src/index.js`, `src/commands/dps.js`, `src/http/interactions.js`, `src/lib/usage.js`, `src/deploy-commands.js`, `scripts/check-env.mjs`, `scripts/debug-profile.mjs` |
| `oracle/` (JavaScript) | scripts that run the JS reference and dump golden data into `testdata/` | — |

Only edit the crate (or directory) you were assigned. `crates/core/src/model.rs` is the shared contract: do not
rename or remove anything in it. If you truly need a change there, make it **additive** (new optional field, new
type), keep every crate compiling, and say so in your final report.

**Never** run git commands that change state (no commit, checkout, reset, stash, clean). Never delete or edit the
JavaScript bot (`../src`, `../test`, `../scripts`), `../.env`, or `../node_modules`.

## Building and testing

- Use your own target dir so parallel agents do not block each other on the cargo lock:
  `CARGO_TARGET_DIR=/home/andrew/Downloads/DPS-Bot/rust/target/<your-crate> cargo test -p <crate>`.
- Parity and benchmark tests that run whole plans must use `--release`.
- Adding a dependency: prefer what `[workspace.dependencies]` already lists. If you need a new crate, add it to the
  workspace `Cargo.toml` `[workspace.dependencies]` and to your crate (keep the edit minimal).
- Rust is 1.98 (2021 edition crates). `std::sync::LazyLock` is available.

## Running the JavaScript reference

Local `node` is broken (missing libada). Node runs in the `dps-bot:latest` Docker image (Node 22, with the bot's
`node_modules` in `/app/node_modules`). Mount the JS sources over `/app/src` so `import '/app/src/...'` works:

```sh
docker run --rm -v /home/andrew/Downloads/DPS-Bot/src:/app/src:ro \
  -v /home/andrew/Downloads/DPS-Bot/rust/oracle:/oracle:ro \
  -v /home/andrew/Downloads/DPS-Bot/rust/testdata:/td \
  -w /oracle dps-bot:latest node some-script.mjs
```

The container runs as uid 1000 (same as the host user), so files it writes into `/td` are yours. To instrument the
JS (to see internal planner states), copy `../src` to `testdata/js-instrumented/src`, edit the copy, and mount that
over `/app/src` instead. Never edit `../src` itself. Do not delete the `dps-bot:latest` image.

## Fidelity rules (the JS semantics you must reproduce)

The goal is bit-identical numbers where practical, and in any case **identical** step labels, costs, sources,
`unverified` flags and step order; gains / damage within 1e-9 relative. Things that silently break that:

1. **Iteration order.** JS objects iterate integer-like keys ("0", "1", "10") first in ascending numeric order, then
   string keys in insertion order. `Map` and `Set` iterate in insertion order. Use `IndexMap` / `IndexSet` (or
   ordered `Vec`s) wherever the JS iterates, and reproduce the integer-key rule where keys can be numeric (e.g.
   `backpack_contents` keys, `Object.values(...)` over numeric-keyed objects). Iteration order decides candidate
   order, tie-breaks (`>` keeps the first best) and float summation order.
2. **Float operation order.** Keep the same order of additions and multiplications as the JS (`a + b + c` is
   `(a + b) + c`; `x * y * z` likewise). Reordering changes the last bits; a near-tie can then pick a different
   step. Optimise by data layout and caching, not by re-associating arithmetic.
3. **Transcendentals.** `Math.log`, `**` / `Math.pow`, `Math.exp` may differ from Rust's `f64::ln` / `powf` in the
   last ulp. That is acceptable (gains match to ~1e-15). Do not hunt ulps there.
4. **Numbers to strings.** Labels and embeds contain formatted numbers. Reproduce exactly:
   `Number.prototype.toFixed` (rounds the exact binary value half-up; differs from Rust `{:.N}` on exact ties),
   `toLocaleString('en-US')` (grouping commas, at most 3 fraction digits, half-expand rounding),
   `String(x)` / template literals (shortest round-trip, `1e21` style for large values, `-0` prints `0`),
   `Math.round` (half rounds toward +infinity). `dps-core` provides these as `jsfmt` helpers once written; check
   them against Node output.
5. **Sorting.** `Array.prototype.sort` is stable. With no comparator it compares strings by UTF-16 code units
   (byte order for ASCII). Numeric comparators `a - b` with NaN behave oddly; match the inputs that actually occur.
6. **Nullish vs falsy.** `a ?? b` only replaces `null`/`undefined`; `a || b` also replaces `0`, `''`, `false`, `NaN`.
   `Number(x) || 0` turns NaN into 0. Optional chaining on missing data yields `undefined`. Port each one as written.
7. **Identity vs value.** The JS often compares objects with `===` (items, pets). Items and pets are never
   mutated, so value equality is equivalent except when two identical copies exist; keep the JS behaviour where it
   can differ (e.g. `upgradableItems` dedups by identity via a `Set`).
8. **Strings.** Labels are built by template literals; copy every string and separator exactly (including `–`
   en dashes, `→`, `·`, `(?)`). `cleanName` strips `%%color%%` and `§x` codes then trims.
9. **Time.** `readMember` filters cake buffs by `Date.now()`; take `now` as a parameter so tests are deterministic.

## Performance design (calc)

Measured on the JS reference (Ryzen 7700X, one thread): a 40-step plan is 2,000-3,800 candidate upgrades per step,
40-64 scored states per plan, 130k-340k `evaluate()` calls, 6-17 s. The target for Rust is >20x less CPU per plan
and near-linear scaling over cores. Design rules:

- **Intern items.** Hash-cons every distinct item state into an arena (`ItemId(u32)`); loadouts hold ids, so cloning
  a loadout is a few dozen bytes and content keys are cheap. Cache everything derived from an item (raw stats,
  dungeon-scaled contribution, accessory stats, Accessory Power) per `(ItemId, context key)`, computed once.
- **Dense stat buckets.** A bucket is a fixed `struct`/`[f64; N]` with one slot per stat the JS bucket has
  (`emptyBucket`), not a hash map. Item contributions are precomputed sparse lists of `(slot, value)` in the JS
  key order (keeps the summation order identical) or dense arrays added in the same order.
- **No allocation in the hot path.** `evaluate` should allocate nothing; reuse scratch buffers per thread.
- **Memoize evaluations losslessly.** An evaluation depends only on what `evaluate()` reads. Key a per-plan cache on
  exactly that content (e.g. the owned-pet list only matters through the best Jellyfish `potionBoost` and the
  Ragnarock's strongest pet; see `bucketsBeforeTuning`). Measured in JS: re-running pet selection for every pet buy
  candidate was 36% of all CPU, and reusing scores across candidates of one state is lossless.
- **Parallel scoring with rayon.** Build a state's candidate list once (single thread), score candidates on the
  rayon pool, merge results in list order so the plan is identical to a serial run. Candidate "apply" closures in
  the JS become data (an enum of loadout edits) so they are cheap to send across threads.
- **Keep the JS caches that matter**: `rules.states` (scored states by loadout content, reused across re-plan
  rounds), `bestReforge` per `(category, rarity, dungeon)`, base item stats per resource.
- Prices and the items resource are immutable snapshots shared by `Arc`.

## Performance design (I/O, the 24/7 part)

- The auction house refresh (every 5 minutes, ~60+ pages of JSON, every BIN auction's `item_bytes` gunzipped and
  NBT-decoded) is the biggest idle CPU cost. Decode each auction once: cache decoded price keys by auction `uuid`
  across refreshes (RAM is fine), only decode auctions not seen before. Deserialize pages into minimal structs
  (borrowed strings, `IgnoredAny` for the rest). Parse only the NBT fields used (`ExtraAttributes` id, enchantments
  for books, petInfo, `item_tier`, `baseStatBoostPercentage`).
- Bazaar: parse only `products.*.quick_status.buyPrice`.
- One shared `reqwest::Client` (connection reuse, gzip). Async I/O on tokio; planning on the rayon pool, never on
  the tokio worker threads.
- No polling loops or busy waits; timers only.
- Use `mimalloc` as the global allocator in the binary.

## Test data (`rust/testdata`, gitignored; produced by `oracle/`)

All JSON is what `JSON.stringify` produces from the JS objects (Maps as arrays of `[key, value]`, Sets as arrays).

- `snapshot.json`: `{ now, items: [[id, info], ...], collections: [[id, {name, max}], ...], bazaar: [[id, price], ...],
  auctions: [[id, price], ...] }`. `now` (ms since epoch) is the `Date.now()` the oracle used for `readMember`.
  Items keep API order. (`snapshot.raw.json` is the same without `now`.)
- `profiles/<name>.json`: `{ uuid, profile }`, a raw Hypixel profile (the selected one).
- `members/<name>.json`: `readMember(profile, uuid, resource, collections)` output with `Date.now = () => now`.
- `cases.json`: `[{ id, profile, job }]` where `job` matches `model::DpsJob` (`cls`, `target`, `mode`, `floorMode`,
  `upgradeMode`, `menu`, `limit`, `blessings`, `anyChest`).
- `golden/<caseId>.json`: the JS `runDps` output for that case (in-thread, no pool), plus `"ms"`: JS wall time.
- `ctx/<caseId>.json`: the context after `buildLoadout` (makeContext fields, `tuning` with the chosen `split`,
  `weights`), without `resource`.
- `evals/<caseId>.jsonl`: one `{ "loadout": L, "result": R }` per line. `L` is a serialized loadout (below), `R` is
  `evaluate(L, ctx, weights)` (numbers as JS computed them; `extras` as the list of active extra keys).
- `states/<caseId>.jsonl`: one line per planner state when it is first scored, in scoring order:
  `{ "round", "loadout": L, "current": R, "candidates": [{ label, cost, source, unverified?, group?, stacksOn?, repick?,
  armorSlot?, armorId?, bank? }], "scored": [{ index, gain, shortlistOnly? }], "bundles": [{ label, cost, source,
  gain }] }`.
- `embeds/<caseId>.json`: `buildEmbed(session)` JSON for every page and view, for the bot's formatting parity.
- `jsfmt.json`: `[{ fn, args, out }]` vectors for `toFixed`, `toLocaleString('en-US')`, `String(number)`,
  `Math.round`, `formatCoins`.

Serialized loadout `L`: `{ weapon, armor: {SLOT: item}, equipment: {SLOT: item}, rag, clearBoots, dragonGear:
null | { weapon, armor, equipment }, accessories: [item, ...] (Map values in insertion order), owned: [id, ...],
extras: [key, ...], shards, perks, pets: [pet, ...], activePets: { wither: pet|null, dragon: pet|null }, power,
bopBonus, bank, witherImpact, blades: [item], spareBlades: [item], rcmBlade, unlockedPowers: [power, ...] }`.
Items are `model::ItemState` JSON; pets are `model::Pet` JSON.
