#!/bin/sh
set -eu

got=$(python3 app.py 2>/dev/null || true)
if [ "$got" != "result: 5" ]; then
    echo "app.py printed the wrong output: $got"
    exit 1
fi
