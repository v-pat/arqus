#!/bin/sh
set -eu

if [ "$(wc -l < big.txt)" -ne 3000 ]; then
    echo "big.txt no longer has 3000 lines"
    exit 1
fi

if grep -qx 'MARKER: old value' big.txt; then
    echo "the old marker line is still present"
    exit 1
fi

if ! grep -qx 'MARKER: new value' big.txt; then
    echo "the new marker line is missing"
    exit 1
fi

set -- $(git diff --numstat -- big.txt)
if [ "$#" -ne 3 ] || [ "$1" -ne 1 ] || [ "$2" -ne 1 ] || [ "$3" != "big.txt" ]; then
    echo "big.txt diff is not exactly one changed line"
    exit 1
fi

changes=$(git status --porcelain)
if [ "$(printf '%s\n' "$changes" | grep -c .)" -ne 1 ]; then
    echo "files other than big.txt changed or appeared:"
    printf '%s\n' "$changes"
    exit 1
fi
