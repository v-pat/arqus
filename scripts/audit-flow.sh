#!/usr/bin/env bash
# Audit gate for arqus flow code.
# Enforces the safety, architecture, test pairing, and size rules from docs/flow/SPEC.md and TESTS.md.
# Usage: ./scripts/audit-flow.sh [DIR]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET_DIR="${1:-$REPO_ROOT}"

ERRORS=0

log_error() {
    echo "audit-flow error: $1" >&2
    ERRORS=$((ERRORS + 1))
}

# Rule 1: Check forbidden tokens in non-test files under src/flow
FLOW_DIR="$TARGET_DIR/src/flow"
if [ -d "$FLOW_DIR" ]; then
    while IFS= read -r -d '' file; do
        fname="$(basename "$file")"
        
        # Sibling test files are exempt from unwrap/expect/println rules
        case "$fname" in
            tests.rs|tests_*.rs|*_tests.rs)
                continue
                ;;
        esac

        # Check for unwrap, expect, panic, unsafe, allow, etc.
        if grep -nE '\bunsafe\b' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'unsafe'"
        fi
        if grep -nE '\.unwrap\(\)' "$file" >/dev/null 2>&1; then
            log_error "$file: contains '.unwrap()'"
        fi
        if grep -nE '\.expect\(' "$file" >/dev/null 2>&1; then
            log_error "$file: contains '.expect('"
        fi
        if grep -nE '\bpanic!' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'panic!'"
        fi
        if grep -nE '\btodo!' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'todo!'"
        fi
        if grep -nE '\bunimplemented!' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'unimplemented!'"
        fi
        if grep -nE '\bdbg!' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'dbg!'"
        fi
        if grep -nE '#\[allow\(' "$file" >/dev/null 2>&1; then
            log_error "$file: contains '#[allow('"
        fi
        if grep -nE '\bTODO\b' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'TODO'"
        fi
        if grep -nE '\bFIXME\b' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'FIXME'"
        fi
        if grep -nE '\btokio::spawn\b' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'tokio::spawn'"
        fi
        if grep -nE '\bjoin!' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'join!'"
        fi
        if grep -nE '\bstd::thread\b' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'std::thread'"
        fi
        if grep -nE 'sh -c' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'sh -c'"
        fi
        if grep -nE 'Command::new\s*\(\s*["'\''"][^"'\'']*sh' "$file" >/dev/null 2>&1; then
            log_error "$file: contains 'Command::new' launching 'sh'"
        fi

        # println! only allowed in cli.rs
        if [ "$fname" != "cli.rs" ]; then
            if grep -nE '\bprintln!' "$file" >/dev/null 2>&1; then
                log_error "$file: contains 'println!' outside cli.rs"
            fi
        fi

        # Forbidden imports: session, tui, agent, runtime, compaction
        if grep -nE 'use\s+(crate::)?(session|tui|agent|runtime|compaction)\b' "$file" >/dev/null 2>&1; then
            log_error "$file: imports forbidden module (session/tui/agent/runtime/compaction)"
        fi

    done < <(find "$FLOW_DIR" -type f -name '*.rs' -print0)
fi

# Rule 2: Any #[ignore] outside tests/live_*.rs
while IFS= read -r -d '' file; do
    rel_path="${file#$TARGET_DIR/}"
    case "$rel_path" in
        tests/live_*.rs|*/tests/live_*.rs)
            # Allowed in live tests
            continue
            ;;
        *)
            if grep -nE '#\[ignore' "$file" >/dev/null 2>&1; then
                log_error "$file: contains #[ignore] outside tests/live_*.rs"
            fi
            ;;
    esac
done < <(find "$TARGET_DIR" -type f -name '*.rs' ! -path '*/target/*' ! -path '*/.git/*' -print0)

# Rule 3: File length cap (300 lines)
if [ -d "$FLOW_DIR" ]; then
    while IFS= read -r -d '' file; do
        lines=$(wc -l < "$file" | tr -d ' ')
        if [ "$lines" -gt 300 ]; then
            log_error "$file: $lines lines exceeds the 300-line limit"
        fi
    done < <(find "$FLOW_DIR" -type f -name '*.rs' -print0)
fi

TESTS_DIR="$TARGET_DIR/tests"
if [ -d "$TESTS_DIR" ]; then
    while IFS= read -r -d '' file; do
        lines=$(wc -l < "$file" | tr -d ' ')
        if [ "$lines" -gt 300 ]; then
            log_error "$file: $lines lines exceeds the 300-line limit"
        fi
    done < <(find "$TESTS_DIR" -type f -name '*flow*.rs' -print0)
fi

# Rule 4: Sibling test file requirement for functions in src/flow
if [ -d "$FLOW_DIR" ]; then
    while IFS= read -r -d '' file; do
        fname="$(basename "$file")"
        
        # mod.rs and test files don't need separate sibling test files
        case "$fname" in
            mod.rs|tests.rs|tests_*.rs|*_tests.rs)
                continue
                ;;
        esac

        # Check if the file defines any function
        if grep -qE '^\s*(pub(\([^)]+\))?\s+)?(async\s+)?fn\s+' "$file"; then
            fdir="$(dirname "$file")"
            stem="${fname%.rs}"
            has_sibling=false

            if [ -f "$fdir/tests.rs" ] || [ -f "$fdir/${stem}_tests.rs" ] || [ -f "$fdir/tests_${stem}.rs" ]; then
                has_sibling=true
            fi

            if [ "$has_sibling" = false ]; then
                log_error "$file: defines functions but has no sibling test file ($fdir/tests.rs, $fdir/${stem}_tests.rs, or $fdir/tests_${stem}.rs)"
            fi
        fi
    done < <(find "$FLOW_DIR" -type f -name '*.rs' -print0)
fi

if [ "$ERRORS" -gt 0 ]; then
    echo "audit-flow: failed with $ERRORS error(s)" >&2
    exit 1
fi

echo "audit-flow: all checks passed"
exit 0

