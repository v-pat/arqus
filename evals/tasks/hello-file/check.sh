#!/bin/sh
set -eu

test -f hello.txt || {
    echo "hello.txt is missing"
    exit 1
}

python3 - <<'PY'
from pathlib import Path

text = Path("hello.txt").read_text()
if text not in ("hello", "hello\n"):
    raise SystemExit(f"hello.txt has the wrong content: {text!r}")
PY
