#!/bin/sh
set -eu

if [ ! -s "${PIRS_OUT:-}" ]; then
    echo "no explanation was captured on stdout"
    exit 1
fi

changes=$(git status --porcelain)
if [ -n "$changes" ]; then
    echo "the project was modified:"
    printf '%s\n' "$changes"
    exit 1
fi
