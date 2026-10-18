#!/bin/sh
# Random loadouts + JS results for dps-calc's fuzz test (testdata/fuzz). Usage: oracle/calc-fuzz.sh [perCase=40]
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
RUST=$(dirname "$HERE")
ROOT=$(dirname "$RUST")
DST="$RUST/testdata/js-calc"
if [ ! -d "$DST/src" ]; then
  rm -rf "$DST"; mkdir -p "$DST"; cp -r "$ROOT/src" "$DST/src"
  printf '\nexport { bestTuningSplit, score, cloneLoadout, setSlot, setDragonSlot, mainItem, dragonItem, slotName, replaceItem, upgradableItems, excludedItem, dragonItems };\n' >> "$DST/src/calc/upgrades.js"
fi
JS_SRC="$DST/src" exec "$HERE/run.sh" calc-fuzz.mjs "$@"
