# DPS Bot, Rust port

Same bot as the Node code in `../src`, written to give the same answers (same plan steps, labels, costs, order; numbers within 1e-9 relative) while using as little CPU as possible. Rules for contributors: `PORTING_NOTES.md`.

## Layout

| crate | what |
|---|---|
| `crates/core` (`dps-core`) | shared types (`model.rs`), config tables, the four NEU data files (compiled in from `../src/data`), formulas, pets, drops, requirements, JS-exact number formatting (`jsfmt`) |
| `crates/calc` (`dps-calc`) | damage model and planner. Items are interned, stat buckets are dense, `evaluate` allocates nothing; candidates are scored on rayon in list order, so the result equals a serial run. Entry point `run_dps` |
| `crates/skyblock` (`dps-skyblock`) | HTTP, Mojang, Hypixel resources and profile cache, NBT, `read_member`, price service (bazaar and auction house, decoded auctions cached by uuid), NEU `update_data` |
| `crates/bot` (`dps-bot`) | the binary: Discord gateway and HTTP interactions, `/dps` flow, embeds, sessions, usage counter, `engine.rs` (`RealEngine`), `cli.rs` |
| `oracle/` | JavaScript scripts that run the Node bot and dump golden data into `testdata/` |

`RealEngine` (`crates/bot/src/engine.rs`): Mojang lookup, Hypixel profile and item list, `read_member`, then `run_dps` on a blocking thread (never a tokio worker; the planner fans out over rayon). At most `PLAN_WORKERS` plans run at once (default 2, the rest queue), a plan is stopped through the cancel flag after `PLAN_TIMEOUT_MS` (default 3 minutes), and finished results are cached losslessly by (profile member content, job, price version), up to 512 entries.

## Build

```
cd rust
cargo build --release --locked -p dps-bot            # target/release/dps-bot
docker build -t dps-bot-rs ..                        # from the repo root: ../Dockerfile (rust:1.98-bookworm -> debian bookworm-slim, 35 MB compressed)
docker build --build-arg TARGET_CPU=x86-64-v3 -t dps-bot-rs ..   # default is x86-64-v2
```

The image runs as a non-root user, keeps the run counter in the `/app/data` volume (`USAGE_FILE=/app/data/usage.json`) and starts `dps-bot`. The Node image is still `Dockerfile.node`.

Subcommands: `dps-bot` (run), `deploy` (register /dps), `check-env`, `debug-profile <ign> [cls] [mode]` (same output as `scripts/debug-profile.mjs`), `update-data [dir]` (default `src/data`; rebuild afterwards, the data is compiled in), `price-refresh` (only the price loops, no Discord; used for idle measurements).

## Test

```
CARGO_TARGET_DIR=target/mine cargo test --release --workspace
```

`testdata/` (gitignored) is produced by `oracle/` from the Node bot; see `oracle/README.md`. Without it the parity tests that need it fail.

## Parity

- `crates/calc/tests/plan_parity.rs`: `run_dps` against the JS golden output for all 262 cases in `testdata/cases.json` (same steps in the same order, numbers within 1e-9 relative): passes.
- Also: evaluate vectors, candidate lists, planner states, `read_member` for 30 real profiles, price maps for real auction pages, embeds, jsfmt vectors, bot flow tests (all in the workspace tests, all pass).
- Live check: `dps-bot debug-profile recticc arch` and `Scraamss mage lcm` print byte-identical output to `node scripts/debug-profile.mjs` (profiles, pets read, first plan step).

## Benchmarks

Ryzen 7700X class machine (16 threads), shared with other jobs (load average about 5), so numbers vary by about 30%.

| | Node | Rust |
|---|---|---|
| CPU for all 262 golden plans | 13,797 s CPU (`cpuMs` in `testdata/golden-timing`; the oracle ran many Node processes at once, so this is inflated, an idle machine gives 6-17 s per plan, about 2,600 s in total) | 129 s with `RAYON_NUM_THREADS=1`; 210 s CPU (61 s wall) on 16 threads; 127 s CPU (67 s wall) on 4 threads |
| per plan | 6-17 s idle (53 s average in the oracle run) | 0.5 s CPU average (single thread) |
| CPU of the price refresh, no Discord (docker cgroup `usage_usec`) | first load 2.61 s, then 2.22 s over the next 10 minutes (0.37% of one core) | first load 0.71 s, then 0.80 s over the next 10 minutes (0.13% of one core) |
| memory | 546 MB peak (cgroup) for two plans | 240 MB peak for two plans; about 110 MB with only the price service running (it keeps decoded auction keys by uuid) |

So a plan costs roughly 20 times less CPU than the idle-machine Node figure (the like-for-like ratio was not re-measured on an idle machine), and idle price refreshing about 2.8 times less (the Node figure includes the worker thread's startup).
