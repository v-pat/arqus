#!/usr/bin/env bash
# scripts/switch.sh — quick provider switcher for development
# Usage: source scripts/switch.sh {openrouter|anthropic|ollama}

if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
    echo "Notice: switch.sh must be sourced to set environment variables in your current shell."
    echo "Usage: source scripts/switch.sh {openrouter|anthropic|ollama}"
    exit 1
fi

case "${1:-}" in
  openrouter)
    export ARQUS_PROVIDER=openrouter
    export ARQUS_BASE_URL=https://openrouter.ai/api/v1
    echo "[arqus] switched to openrouter (${ARQUS_BASE_URL})"
    ;;
  anthropic)
    export ARQUS_PROVIDER=anthropic
    export ARQUS_BASE_URL=https://api.anthropic.com/v1
    echo "[arqus] switched to anthropic (${ARQUS_BASE_URL})"
    ;;
  ollama)
    export ARQUS_PROVIDER=openai
    export ARQUS_BASE_URL=http://127.0.0.1:11434/v1
    export ARQUS_API_KEY="${ARQUS_API_KEY:-ollama}"
    echo "[arqus] switched to local ollama (${ARQUS_BASE_URL})"
    ;;
  *)
    echo "Usage: source scripts/switch.sh {openrouter|anthropic|ollama}"
    ;;
esac

