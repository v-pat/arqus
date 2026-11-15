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
# Exclude markdown stage specs and auth test fixtures that test fake key masking/parsing.
if git grep -nE "sk-(or-)?[A-Za-z0-9_-]{16,}" -- . ':(exclude)*stages*.md' ':(exclude)arqus-tui-starter' ':(exclude)*connect_tests.rs' ':(exclude)*auth.rs'; then
    echo "secret scan: a possible API key is committed" >&2
    exit 1
fi

echo "==> audit flow"
./scripts/audit-flow.sh
./scripts/test-audit-flow.sh

if command -v python3 >/dev/null 2>&1; then
    echo "==> evals python unit tests"
    python3 -m unittest discover -s evals/tests -p "test_*.py"
fi

echo "all checks passed"
