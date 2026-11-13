#!/bin/sh
set -eu

if [ ! -s "${ARQUS_OUT:-}" ]; then
    echo "no output was captured"
    exit 1
fi

if ! grep -q '137' "$ARQUS_OUT"; then
    echo "the answer 137 is missing from stdout"
    exit 1
fi
