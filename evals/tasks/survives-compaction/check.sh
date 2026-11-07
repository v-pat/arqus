#!/bin/sh
set -eu

python3 - <<'PY'
from pathlib import Path

for step in range(1, 6):
    path = Path(f"step{step}.txt")
    if not path.is_file():
        raise SystemExit(f"{path} is missing")
    expected = "\n".join(f"step {number}" for number in range(1, step + 1))
    actual = path.read_text().strip()
    if actual != expected:
        raise SystemExit(f"{path} has the wrong content: {actual!r}")
PY
