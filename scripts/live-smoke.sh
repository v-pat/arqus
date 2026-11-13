#!/usr/bin/env bash
# Stage 17 live smoke test. Never prints the API key.
#
# Required in your own terminal:
#   export ARQUS_API_KEY=...
#   export ARQUS_LIVE_MODEL=<a cheap paid model id>
#
# Optional:
#   ARQUS_LIVE=1 is set by this script.
set -uo pipefail

if [ -z "${ARQUS_API_KEY:-}" ]; then
    echo "error: ARQUS_API_KEY is not set" >&2
    exit 2
fi
if [ -z "${ARQUS_LIVE_MODEL:-}" ]; then
    echo "error: ARQUS_LIVE_MODEL is not set" >&2
    exit 2
fi

export ARQUS_LIVE=1

log="$(mktemp "${TMPDIR:-/tmp}/arqus-live-smoke.XXXXXX")"
trap 'rm -f "$log"' EXIT

cargo test --test live_openrouter -- --ignored --test-threads=1 --nocapture 2>&1 | tee "$log"
status="${PIPESTATUS[0]}"

awk '
    {
        for (i = 1; i <= NF; i++) {
            if ($i ~ /^cost=\$/) {
                value = $i
                sub(/^cost=\$/, "", value)
                total += value
                count += 1
            }
        }
    }
    END {
        printf "live smoke: %d cost line(s), total cost $%.4f\n", count, total
    }
' "$log"

if [ "$status" -ne 0 ]; then
    echo "live smoke: FAILED (exit $status)" >&2
    exit "$status"
fi

echo "live smoke: all six live tests passed"
