#!/usr/bin/env bash
# scripts/build-flow-profile.sh — builds profile_stats.json from stats.jsonl
set -euo pipefail

cd "$(dirname "$0")/.."

python3 evals/flow_profile.py "$@"

