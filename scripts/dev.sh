#!/usr/bin/env bash
# scripts/dev.sh — run arqus in development mode with .env loaded
set -euo pipefail

cd "$(dirname "$0")/.."

ENV_FILE="${ARQUS_ENV:-.env}"
if [ -f "$ENV_FILE" ]; then
    set -a
    # shellcheck disable=SC1090
    source "$ENV_FILE"
    set +a
fi

exec cargo run -- "$@"

