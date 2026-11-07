#!/bin/sh
set -eu

if grep -rn 'calc_total' --include='*.py' .; then
    echo "the old name calc_total still appears"
    exit 1
fi

python3 -m unittest -q test_all
