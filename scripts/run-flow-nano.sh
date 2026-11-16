#!/usr/bin/env bash
# scripts/run-flow-nano.sh — nano evaluation runner and profiler
set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== arqus flow nano-eval runner ==="
echo ""

WORKER="${1:-local-default}"
STATS_FILE="${HOME}/.arqus/flow/stats.jsonl"
OUT_DIR="evals/results_nano_$(date +%Y%m%d_%H%M%S)"

echo "==> Running nano contracts for worker '${WORKER}'..."
python3 evals/flow_run_nano.py --worker "${WORKER}" --stats-file "${STATS_FILE}" --out-dir "${OUT_DIR}" "$@"

echo ""
echo "==> Updating worker capability profile..."
PROFILE_OUTPUT="${HOME}/.arqus/flow/profile_stats.json"
python3 evals/flow_profile.py "${STATS_FILE}" --out "${PROFILE_OUTPUT}"

echo "Updated profile at ${PROFILE_OUTPUT}"

