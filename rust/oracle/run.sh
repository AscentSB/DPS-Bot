#!/bin/sh
# Run one oracle script in the dps-bot:latest image (Node 22 + the bot's node_modules).
#   oracle/run.sh snapshot.mjs
#   oracle/run.sh --net fetch-profiles.mjs   (network + the Hypixel API key from ../.env, mounted read-only)
# JS_SRC=/path/to/src overrides the JS sources mounted at /app/src (default: the bot's src, read-only).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
RUST=$(dirname "$HERE")
ROOT=$(dirname "$RUST")
SRC=${JS_SRC:-$ROOT/src}
EXTRA=""
if [ "$1" = "--net" ]; then
  shift
  EXTRA="-v $ROOT/.env:/secrets/env:ro"
fi
mkdir -p "$RUST/testdata"
exec docker run --rm -v "$SRC:/app/src:ro" -v "$HERE:/oracle:ro" -v "$RUST/testdata:/td" $EXTRA \
  -w /oracle dps-bot:latest node "$@"
