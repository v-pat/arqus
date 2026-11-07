#!/bin/sh
set -eu

python3 - <<'PY'
from pathlib import Path

expected = """# Release 1.2.0

## Changes
- Add --upper flag
- Fix typo in docs
"""
path = Path("RELEASE_NOTES.md")
if not path.is_file():
    raise SystemExit("RELEASE_NOTES.md is missing")
actual = path.read_text()
if actual.strip() != expected.strip():
    raise SystemExit(f"RELEASE_NOTES.md has the wrong format: {actual!r}")
PY
