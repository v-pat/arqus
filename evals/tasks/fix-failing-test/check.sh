#!/bin/sh
set -eu

expected=54645b4bf67baa910f6a39045216fa3aaaea1b8706746d36d1351dbfae2c804f
actual=$(python3 - <<'PY'
import hashlib
print(hashlib.sha256(open("test_calc.py", "rb").read()).hexdigest())
PY
)
if [ "$actual" != "$expected" ]; then
    echo "test_calc.py was modified"
    exit 1
fi

python3 -m unittest -q test_calc
