#!/bin/sh
# Run golden.mjs for every case in testdata/cases.json in N containers at once (default 10), all pulling from one
# claim queue (testdata/golden-claims), then merge the timings (golden-summary.mjs).
#   oracle/golden-all.sh [N] [extra golden.mjs args, e.g. --force]
# Cases that already have testdata/golden/<id>.json are skipped unless --force is passed, so an interrupted run
# resumes. Logs: testdata/golden-timing/w<i>.log.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
RUST=$(dirname "$HERE")
N=${1:-10}
[ $# -gt 0 ] && shift
rm -rf "$RUST/testdata/golden-claims"
mkdir -p "$RUST/testdata/golden-claims" "$RUST/testdata/golden-timing"
pids=""
i=1
while [ "$i" -le "$N" ]; do
  "$HERE/run.sh" golden.mjs --claims /td/golden-claims --worker "$i" "$@" > "$RUST/testdata/golden-timing/w$i.log" 2>&1 &
  pids="$pids $!"
  i=$((i + 1))
done
fail=0
for pid in $pids; do wait "$pid" || fail=1; done
rm -rf "$RUST/testdata/golden-claims"
[ "$fail" -eq 0 ] || { echo "some workers failed: see testdata/golden-timing/w*.log" >&2; exit 1; }
exec "$HERE/run.sh" golden-summary.mjs
