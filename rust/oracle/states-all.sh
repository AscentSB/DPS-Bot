#!/bin/sh
# Run states.mjs for each of its DEFAULT_CASES in its own container, P at a time (default 6), against the
# instrumented copy (instrument.mjs builds it). Logs: testdata/states/<caseId>.log.
#   oracle/states-all.sh [P] [extra states.mjs args, e.g. --no-verify]
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
RUST=$(dirname "$HERE")
P=${1:-6}
[ $# -gt 0 ] && shift
export JS_SRC="$RUST/testdata/js-instrumented/src"
[ -f "$JS_SRC/calc/upgrades.js" ] || "$HERE/run.sh" instrument.mjs
mkdir -p "$RUST/testdata/states"
"$HERE/run.sh" states.mjs --list | tr -d '\r' > "$RUST/testdata/states/.cases"
export HERE RUST
EXTRA="$*"
export EXTRA
fail=0
xargs -P "$P" -I{} sh -c '"$HERE/run.sh" states.mjs $EXTRA "{}" > "$RUST/testdata/states/{}.log" 2>&1 || { echo "FAILED {}"; exit 1; }' \
  < "$RUST/testdata/states/.cases" || fail=1
for id in $(cat "$RUST/testdata/states/.cases"); do tail -n 2 "$RUST/testdata/states/$id.log" | head -n 1; done
rm -f "$RUST/testdata/states/.cases"
exit $fail
