#!/usr/bin/env bash
# scripts/run-evals.sh — interactive eval suite runner
set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== arqus eval runner ==="
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

# Ensure API key is available
if [ -z "${ARQUS_API_KEY:-}" ]; then
    if [ -t 0 ]; then
        echo "No ARQUS_API_KEY found in environment or .env."
        echo -n "Paste your API key (input hidden): "
        read -rs ARQUS_API_KEY
        echo ""
        if [ -z "${ARQUS_API_KEY}" ]; then
            echo "error: API key cannot be empty." >&2
            exit 1
        fi
        export ARQUS_API_KEY
    else
        echo "error: ARQUS_API_KEY is required to run evals against live models." >&2
        echo "Set ARQUS_API_KEY or create .env.evals." >&2
        exit 1
    fi
fi

# Ensure model is chosen
if [ -z "${ARQUS_MODEL:-}" ]; then
    if [ -t 0 ]; then
        echo ""
        echo "Select evaluation model:"
        echo "  1) openrouter:deepseek/deepseek-chat (recommended, lowest cost)"
        echo "  2) openrouter:anthropic/claude-sonnet-4"
        echo "  3) anthropic:claude-sonnet-4"
        echo "  4) Custom (enter provider:model)"
        echo -n "Choice [1]: "
        read -r choice
        case "${choice:-1}" in
            1)
                export ARQUS_PROVIDER=openrouter
                export ARQUS_MODEL="deepseek/deepseek-chat"
                ;;
            2)
                export ARQUS_PROVIDER=openrouter
                export ARQUS_MODEL="anthropic/claude-sonnet-4"
                ;;
            3)
                export ARQUS_PROVIDER=anthropic
                export ARQUS_MODEL="claude-sonnet-4"
                ;;
            4)
                echo -n "Enter provider:model: "
                read -r custom_model
                export ARQUS_MODEL="${custom_model}"
                ;;
            *)
                echo "Invalid choice. Defaulting to openrouter:deepseek/deepseek-chat."
                export ARQUS_PROVIDER=openrouter
                export ARQUS_MODEL="deepseek/deepseek-chat"
                ;;
        esac
    else
        export ARQUS_PROVIDER="${ARQUS_PROVIDER:-openrouter}"
        export ARQUS_MODEL="deepseek/deepseek-chat"
    fi
fi

# Budget limit
export EVAL_MAX_COST="${EVAL_MAX_COST:-1.00}"

# Normalize model argument if ARQUS_MODEL has provider:model shape
MODEL_ARG=()
if [[ "$*" != *"--model"* ]] && [ -n "${ARQUS_MODEL:-}" ]; then
    if [[ "${ARQUS_MODEL}" == *":"* ]]; then
        MODEL_ARG=("--model" "${ARQUS_MODEL}")
    elif [ -n "${ARQUS_PROVIDER:-}" ]; then
        MODEL_ARG=("--model" "${ARQUS_PROVIDER}:${ARQUS_MODEL}")
    fi
fi

echo ""
echo "Model:    ${ARQUS_MODEL} (provider: ${ARQUS_PROVIDER:-default})"
echo "Max cost: \$${EVAL_MAX_COST}"
echo ""

echo "==> Building arqus (release)..."
cargo build --release

echo ""
echo "==> Running evals..."
python3 evals/run.py --bin target/release/arqus "${MODEL_ARG[@]}" "$@"

echo ""
echo "Results recorded in evals/results.jsonl"
echo "To record as baseline: copy the table above into evals/BASELINE.md"

