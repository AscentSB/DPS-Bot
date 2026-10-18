# Profile-guided build

`docker build` (PGO=1, the default) builds an instrumented `dps-bot`, runs `dps-bot pgo-train train.json` on it, merges
the counters with the `llvm-profdata` that matches rustc's LLVM (`rustup component add llvm-tools`) and then builds the
shipped binary with `-C profile-use`. About 10% less CPU per `/dps` than the plain build; output is unchanged.
`--build-arg PGO=0` skips all of it.

`train.json.gz` holds the game-data resource, the price snapshot, 7 anonymous member snapshots (no names, UUIDs or
item instance ids) and 94 jobs. Regenerate it, with the gitignored `testdata/` present, via
`python3 pgo/make-train.py` from `rust/` (optionally `make-train.py out.json.gz profile...`). The profile only has to
look like real `/dps` work; refresh the file when the planner changes a lot.

Local PGO build:

    RUSTFLAGS="-C target-cpu=x86-64-v2 -C profile-generate=/tmp/pgo-raw" CARGO_TARGET_DIR=/tmp/t-gen cargo build --release -p dps-bot
    zcat pgo/train.json.gz > /tmp/train.json && /tmp/t-gen/release/dps-bot pgo-train /tmp/train.json
    llvm-profdata merge -o /tmp/pgo.profdata /tmp/pgo-raw     # same LLVM version as `rustc -vV`
    RUSTFLAGS="-C target-cpu=x86-64-v2 -C profile-use=/tmp/pgo.profdata" cargo build --release -p dps-bot
