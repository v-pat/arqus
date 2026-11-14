#!/usr/bin/env bash
# Test suite for scripts/audit-flow.sh.
# Creates temporary trees with individual violations and asserts that audit-flow.sh fails.
# Also creates a clean tree and tests against the repository root to assert that audit-flow.sh passes.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
AUDIT_SCRIPT="$SCRIPT_DIR/audit-flow.sh"

PASSED_COUNT=0
FAILED_COUNT=0

TEMP_BASE="$(mktemp -d "${TMPDIR:-/tmp}/audit_flow_test_XXXXXX")"
trap 'rm -rf "$TEMP_BASE"' EXIT

assert_fails() {
    local test_name="$1"
    local target_dir="$2"
    if "$AUDIT_SCRIPT" "$target_dir" >/dev/null 2>&1; then
        echo "FAIL (expected failure, but passed): $test_name" >&2
        FAILED_COUNT=$((FAILED_COUNT + 1))
    else
        echo "PASS: $test_name"
        PASSED_COUNT=$((PASSED_COUNT + 1))
    fi
}

assert_passes() {
    local test_name="$1"
    local target_dir="$2"
    if "$AUDIT_SCRIPT" "$target_dir" >/dev/null 2>&1; then
        echo "PASS: $test_name"
        PASSED_COUNT=$((PASSED_COUNT + 1))
    else
        echo "FAIL (expected pass, but failed): $test_name" >&2
        "$AUDIT_SCRIPT" "$target_dir" || true
        FAILED_COUNT=$((FAILED_COUNT + 1))
    fi
}

make_clean_tree() {
    local dir="$1"
    mkdir -p "$dir/src/flow" "$dir/tests"
    cat << 'EOF' > "$dir/src/flow/mod.rs"
pub mod cli;
pub mod engine;
EOF
    cat << 'EOF' > "$dir/src/flow/cli.rs"
pub fn run_cli() {
    println!("flow cli");
}
EOF
    cat << 'EOF' > "$dir/src/flow/tests.rs"
#[test]
fn test_cli_dummy() {}
EOF
    mkdir -p "$dir/src/flow/engine"
    cat << 'EOF' > "$dir/src/flow/engine/mod.rs"
pub mod state;
EOF
    cat << 'EOF' > "$dir/src/flow/engine/state.rs"
pub fn init_state() -> Result<(), String> {
    Ok(())
}
EOF
    cat << 'EOF' > "$dir/src/flow/engine/tests.rs"
#[test]
fn test_state_dummy() {}
EOF
    cat << 'EOF' > "$dir/tests/e2e_flow_plan.rs"
// under 300 lines
EOF
}

# 1. Clean tree passes
test_clean_tree_passes() {
    local tdir="$TEMP_BASE/clean"
    make_clean_tree "$tdir"
    assert_passes "test_clean_tree_passes" "$tdir"
}

# 2. Violation: unsafe in src/flow
test_violation_unsafe_fails() {
    local tdir="$TEMP_BASE/unsafe"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { unsafe { } }
EOF
    assert_fails "test_violation_unsafe_fails" "$tdir"
}

# 3. Violation: unwrap in src/flow
test_violation_unwrap_fails() {
    local tdir="$TEMP_BASE/unwrap"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad(x: Option<i32>) { let _ = x.unwrap(); }
EOF
    assert_fails "test_violation_unwrap_fails" "$tdir"
}

# 4. Violation: expect in src/flow
test_violation_expect_fails() {
    local tdir="$TEMP_BASE/expect"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad(x: Option<i32>) { let _ = x.expect("bad"); }
EOF
    assert_fails "test_violation_expect_fails" "$tdir"
}

# 5. Violation: panic in src/flow
test_violation_panic_fails() {
    local tdir="$TEMP_BASE/panic"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { panic!("stop"); }
EOF
    assert_fails "test_violation_panic_fails" "$tdir"
}

# 6. Violation: todo! in src/flow
test_violation_todo_macro_fails() {
    local tdir="$TEMP_BASE/todo_macro"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { todo!(); }
EOF
    assert_fails "test_violation_todo_macro_fails" "$tdir"
}

# 7. Violation: unimplemented! in src/flow
test_violation_unimplemented_fails() {
    local tdir="$TEMP_BASE/unimplemented"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { unimplemented!(); }
EOF
    assert_fails "test_violation_unimplemented_fails" "$tdir"
}

# 8. Violation: dbg! in src/flow
test_violation_dbg_fails() {
    local tdir="$TEMP_BASE/dbg"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { dbg!(42); }
EOF
    assert_fails "test_violation_dbg_fails" "$tdir"
}

# 9. Violation: #[allow( in src/flow
test_violation_allow_attr_fails() {
    local tdir="$TEMP_BASE/allow"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
#[allow(dead_code)]
pub fn bad() {}
EOF
    assert_fails "test_violation_allow_attr_fails" "$tdir"
}

# 10. Violation: TODO comment in src/flow
test_violation_todo_comment_fails() {
    local tdir="$TEMP_BASE/todo_comment"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
// TODO: implement later
EOF
    assert_fails "test_violation_todo_comment_fails" "$tdir"
}

# 11. Violation: FIXME comment in src/flow
test_violation_fixme_comment_fails() {
    local tdir="$TEMP_BASE/fixme_comment"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
// FIXME: broken
EOF
    assert_fails "test_violation_fixme_comment_fails" "$tdir"
}

# 12. Violation: tokio::spawn in src/flow
test_violation_tokio_spawn_fails() {
    local tdir="$TEMP_BASE/tokio_spawn"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { tokio::spawn(async {}); }
EOF
    assert_fails "test_violation_tokio_spawn_fails" "$tdir"
}

# 13. Violation: join! in src/flow
test_violation_join_fails() {
    local tdir="$TEMP_BASE/join"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { tokio::join!(); }
EOF
    assert_fails "test_violation_join_fails" "$tdir"
}

# 14. Violation: std::thread in src/flow
test_violation_std_thread_fails() {
    local tdir="$TEMP_BASE/std_thread"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { std::thread::sleep(std::time::Duration::from_secs(1)); }
EOF
    assert_fails "test_violation_std_thread_fails" "$tdir"
}

# 15. Violation: sh -c in src/flow
test_violation_sh_c_fails() {
    local tdir="$TEMP_BASE/sh_c"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { let _ = "sh -c echo"; }
EOF
    assert_fails "test_violation_sh_c_fails" "$tdir"
}

# 16. Violation: Command::new("sh") in src/flow
test_violation_command_sh_fails() {
    local tdir="$TEMP_BASE/command_sh"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { let _ = std::process::Command::new("sh"); }
EOF
    assert_fails "test_violation_command_sh_fails" "$tdir"
}

# 17. Violation: println! in non-cli file
test_violation_println_fails() {
    local tdir="$TEMP_BASE/println_bad"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
pub fn bad() { println!("cannot log here"); }
EOF
    assert_fails "test_violation_println_fails" "$tdir"
}

# 18. Allowed: println! in cli.rs
test_println_in_cli_passes() {
    local tdir="$TEMP_BASE/println_cli"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/cli.rs"
pub fn another_cli() { println!("allowed here"); }
EOF
    assert_passes "test_println_in_cli_passes" "$tdir"
}

# 19. Violation: forbidden import session
test_violation_forbidden_import_session_fails() {
    local tdir="$TEMP_BASE/import_session"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
use crate::session::Session;
EOF
    assert_fails "test_violation_forbidden_import_session_fails" "$tdir"
}

# 20. Violation: forbidden import tui
test_violation_forbidden_import_tui_fails() {
    local tdir="$TEMP_BASE/import_tui"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
use crate::tui::App;
EOF
    assert_fails "test_violation_forbidden_import_tui_fails" "$tdir"
}

# 21. Violation: forbidden import agent
test_violation_forbidden_import_agent_fails() {
    local tdir="$TEMP_BASE/import_agent"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
use crate::agent::Agent;
EOF
    assert_fails "test_violation_forbidden_import_agent_fails" "$tdir"
}

# 22. Violation: forbidden import runtime
test_violation_forbidden_import_runtime_fails() {
    local tdir="$TEMP_BASE/import_runtime"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
use crate::runtime;
EOF
    assert_fails "test_violation_forbidden_import_runtime_fails" "$tdir"
}

# 23. Violation: forbidden import compaction
test_violation_forbidden_import_compaction_fails() {
    local tdir="$TEMP_BASE/import_compaction"
    make_clean_tree "$tdir"
    cat << 'EOF' >> "$tdir/src/flow/engine/state.rs"
use crate::compaction;
EOF
    assert_fails "test_violation_forbidden_import_compaction_fails" "$tdir"
}

# 24. Violation: #[ignore] outside tests/live_*.rs
test_violation_ignore_outside_live_fails() {
    local tdir="$TEMP_BASE/ignore_outside"
    make_clean_tree "$tdir"
    cat << 'EOF' > "$tdir/tests/e2e_flow_dummy.rs"
#[ignore]
#[test]
fn ignored_test() {}
EOF
    assert_fails "test_violation_ignore_outside_live_fails" "$tdir"
}

# 25. Allowed: #[ignore] inside tests/live_*.rs
test_ignore_in_live_tests_passes() {
    local tdir="$TEMP_BASE/ignore_live"
    make_clean_tree "$tdir"
    cat << 'EOF' > "$tdir/tests/live_flow.rs"
#[ignore]
#[test]
fn live_test() {}
EOF
    assert_passes "test_ignore_in_live_tests_passes" "$tdir"
}

# 26. Violation: file over 300 lines in src/flow
test_violation_file_over_300_lines_in_flow_fails() {
    local tdir="$TEMP_BASE/flow_300"
    make_clean_tree "$tdir"
    for i in $(seq 1 305); do
        echo "// line $i" >> "$tdir/src/flow/engine/state.rs"
    done
    assert_fails "test_violation_file_over_300_lines_in_flow_fails" "$tdir"
}

# 27. Violation: file over 300 lines in tests/*flow*
test_violation_file_over_300_lines_in_tests_flow_fails() {
    local tdir="$TEMP_BASE/tests_flow_300"
    make_clean_tree "$tdir"
    for i in $(seq 1 305); do
        echo "// line $i" >> "$tdir/tests/e2e_flow_plan.rs"
    done
    assert_fails "test_violation_file_over_300_lines_in_tests_flow_fails" "$tdir"
}

# 28. Violation: missing sibling tests file
test_violation_missing_sibling_tests_fails() {
    local tdir="$TEMP_BASE/missing_sibling"
    make_clean_tree "$tdir"
    rm -f "$tdir/src/flow/engine/tests.rs"
    assert_fails "test_violation_missing_sibling_tests_fails" "$tdir"
}

# 29. Allowed: sibling tests file as <name>_tests.rs
test_sibling_tests_passes() {
    local tdir="$TEMP_BASE/sibling_tests"
    make_clean_tree "$tdir"
    rm -f "$tdir/src/flow/engine/tests.rs"
    cat << 'EOF' > "$tdir/src/flow/engine/state_tests.rs"
#[test]
fn test_state() {}
EOF
    assert_passes "test_sibling_tests_passes" "$tdir"
}

# 30. Clean repo audit passes on the actual repository
test_real_repo_audit_passes() {
    assert_passes "test_real_repo_audit_passes" "$REPO_ROOT"
}

echo "Running audit-flow test suite..."
test_clean_tree_passes
test_violation_unsafe_fails
test_violation_unwrap_fails
test_violation_expect_fails
test_violation_panic_fails
test_violation_todo_macro_fails
test_violation_unimplemented_fails
test_violation_dbg_fails
test_violation_allow_attr_fails
test_violation_todo_comment_fails
test_violation_fixme_comment_fails
test_violation_tokio_spawn_fails
test_violation_join_fails
test_violation_std_thread_fails
test_violation_sh_c_fails
test_violation_command_sh_fails
test_violation_println_fails
test_println_in_cli_passes
test_violation_forbidden_import_session_fails
test_violation_forbidden_import_tui_fails
test_violation_forbidden_import_agent_fails
test_violation_forbidden_import_runtime_fails
test_violation_forbidden_import_compaction_fails
test_violation_ignore_outside_live_fails
test_ignore_in_live_tests_passes
test_violation_file_over_300_lines_in_flow_fails
test_violation_file_over_300_lines_in_tests_flow_fails
test_violation_missing_sibling_tests_fails
test_sibling_tests_passes
test_real_repo_audit_passes

echo "=========================================="
echo "Tests passed: $PASSED_COUNT, failed: $FAILED_COUNT"
if [ "$FAILED_COUNT" -gt 0 ]; then
    exit 1
fi
echo "All audit-flow tests passed!"

