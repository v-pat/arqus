#!/usr/bin/env bash
# scripts/run-flow-evals.sh — flow eval suite runner
set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== arqus flow eval runner ==="
echo ""

# Source credentials from .env.evals or .env if present
if [ -f ".env.evals" ]; then
    echo "Loading configuration from .env.evals..."
    set -a
    # shellcheck disable=SC1091
    source ".env.evals"
    set +a
elif [ -f ".env" ]; then
    echo "Loading configuration from .env..."
    set -a
    # shellcheck disable=SC1091
    source ".env"
    set +a
fi

# Ensure orchestrator role key is available
if [ -z "${ARQUS_ORCHESTRATOR_KEY:-}" ] && [ -z "${ARQUS_API_KEY:-}" ]; then
    if [ -t 0 ]; then
        echo "No ARQUS_ORCHESTRATOR_KEY found in environment or .env."
        echo -n "Paste your orchestrator API key (input hidden): "
        read -rs ARQUS_ORCHESTRATOR_KEY
        echo ""
        if [ -z "${ARQUS_ORCHESTRATOR_KEY}" ]; then
            echo "error: Orchestrator API key cannot be empty." >&2
            exit 1
        fi
        export ARQUS_ORCHESTRATOR_KEY
    else
        echo "warning: No ARQUS_ORCHESTRATOR_KEY set; dry-run or mock mode only." >&2
    fi
fi

echo "==> Building arqus (release)..."
cargo build --release

echo ""
echo "==> Running flow evals..."
RESULTS_DIR="evals/results_$(date +%Y%m%d_%H%M%S)"
python3 evals/flow_run.py --bin target/release/arqus --out-dir "${RESULTS_DIR}" "$@"

echo ""
echo "==> Generating report..."
python3 evals/flow_report.py "${RESULTS_DIR}"

echo ""
echo "Report written to evals/FLOW_REPORT.md and results appended to evals/flow_results.jsonl"

