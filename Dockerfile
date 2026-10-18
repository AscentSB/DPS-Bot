# Rust build of the bot. The previous Node image is kept as Dockerfile.node.
#   docker build -t dps-bot-rs .
#   docker build --build-arg TARGET_CPU=native -t dps-bot-rs .   # fastest, only runs on the build machine's CPU
#   docker build --build-arg PGO=0 -t dps-bot-rs .               # skip profile-guided optimization (faster build, ~10% more CPU per /dps)
# PGO=1 (default) builds an instrumented dps-bot, replays rust/pgo/train.json.gz through it (`dps-bot pgo-train`, no
# network or secrets needed), then builds the real binary with that profile. See rust/pgo/README.md.
ARG RUST_VERSION=1.98

FROM rust:${RUST_VERSION}-bookworm AS build
# x86-64-v2 runs on every x86-64 CPU since about 2009; set TARGET_CPU=x86-64-v3 for AVX2 machines.
ARG TARGET_CPU=x86-64-v2
ENV RUSTFLAGS="-C target-cpu=${TARGET_CPU}" CARGO_TARGET_DIR=/target
WORKDIR /build
# The crates embed src/data/*.json at compile time, so the context needs both folders.
COPY rust ./rust
COPY src/data ./src/data
WORKDIR /build/rust
ARG PGO=1
RUN set -e; \
    if [ "$PGO" = 1 ]; then \
      rustup component add llvm-tools; \
      mkdir -p /pgo; \
      export RUSTFLAGS="$RUSTFLAGS -C profile-generate=/pgo/raw"; \
      CARGO_TARGET_DIR=/target-gen cargo build --release --locked -p dps-bot; \
      gunzip -c pgo/train.json.gz > /pgo/train.json; \
      RAYON_NUM_THREADS=2 /target-gen/release/dps-bot pgo-train /pgo/train.json; \
      "$(rustc --print target-libdir)/../bin/llvm-profdata" merge -o /pgo/merged.profdata /pgo/raw; \
      rm -rf /target-gen /pgo/raw; \
      export RUSTFLAGS="-C target-cpu=${TARGET_CPU} -C profile-use=/pgo/merged.profdata"; \
    fi; \
    cargo build --release --locked -p dps-bot; \
    strip -o /dps-bot /target/release/dps-bot

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 1000 --create-home --home-dir /app dps \
 && mkdir -p /app/data && chown dps:dps /app/data
ENV USAGE_FILE=/app/data/usage.json
WORKDIR /app
COPY --from=build /dps-bot /usr/local/bin/dps-bot
# usage.json (the /dps run counter) lives in /app/data; mount a volume there to keep it across restarts.
VOLUME /app/data
USER dps

CMD ["dps-bot"]
