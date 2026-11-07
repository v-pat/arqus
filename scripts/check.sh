#!/bin/sh
# Full local check: formatting, lints, tests, release build, secret scan.
# Usage: scripts/check.sh
set -eu
cd "$(dirname "$0")/.."

echo "==> cargo fmt --check"
cargo fmt --check

echo "==> cargo clippy --all-targets -- -D warnings"
cargo clippy --all-targets -- -D warnings

echo "==> cargo test"
cargo test

echo "==> cargo build --release"
cargo build --release

echo "==> secret scan"
# The stage brief quotes a fake key in its own test description.
if git grep -nE "sk-(or-)?[A-Za-z0-9_-]{16,}" -- . ':(exclude)pirs-stages-12-plus.md'; then
    echo "secret scan: a possible API key is committed" >&2
    exit 1
fi

echo "all checks passed"
