#!/bin/sh
# Dumps testdata/picks/*.json for dps-calc's tests (see calc-picks.mjs). Runs against a copy of the bot's src with a
# few internal functions exported (the bot's own files are not touched):
#   oracle/calc-picks.sh
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
RUST=$(dirname "$HERE")
ROOT=$(dirname "$RUST")
DST="$RUST/testdata/js-calc"
rm -rf "$DST"
mkdir -p "$DST"
cp -r "$ROOT/src" "$DST/src"
printf '\nexport { bestTuningSplit, score, cloneLoadout, setSlot, setDragonSlot, mainItem, dragonItem, slotName, replaceItem, upgradableItems, excludedItem, dragonItems };\n' >> "$DST/src/calc/upgrades.js"
JS_SRC="$DST/src" exec "$HERE/run.sh" calc-picks.mjs "$@"
