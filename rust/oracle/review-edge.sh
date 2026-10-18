#!/bin/sh
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
RUST=$(dirname "$HERE")
DST="$RUST/testdata/js-calc"
JS_SRC="$DST/src" exec "$HERE/run.sh" review-edge.mjs "$@"
