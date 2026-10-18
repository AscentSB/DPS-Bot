#!/bin/sh
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
RUST=$(dirname "$HERE")
JS_SRC="$RUST/testdata/js-calc/src" exec "$HERE/run.sh" review-helpers.mjs "$@"
