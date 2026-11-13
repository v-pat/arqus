#!/bin/sh
# Drive the real arqus TUI in tmux against examples/fake_llm.rs.
#
# Every scenario uses a temporary ARQUS_HOME, a temporary project folder and
# the fake server. No real network, no real key.
#
#   sh scripts/tui-smoke.sh
#
# Set ARQUS_BIN and/or ARQUS_FAKE to reuse already built binaries.
# If tmux is missing the script prints SKIP and exits 0, so a machine without
# tmux does not fail the build.

set -u

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT" || exit 2

if ! command -v tmux >/dev/null 2>&1; then
    echo "SKIP: tmux not found (install it with: brew install tmux)"
    exit 0
fi

SECRET=test-key-abc123
SESSION="arqus-smoke-$$"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/arqus-smoke.XXXXXX") || exit 1
SCREENS="$WORK/screens.log"
SERVER_LOG="$WORK/server.log"
SCRIPT_FILE="$WORK/script.json"
LAUNCH="$WORK/launch.sh"
LAST="$WORK/last.txt"
SERVER_PID=""
BASE_URL=""
APPHOME=""
PROJ=""
FAILS=0

cleanup() {
    stop_server
    stop
    rm -rf "$WORK"
}
trap cleanup EXIT HUP INT TERM

if [ -n "${ARQUS_BIN:-}" ]; then
    BIN=$ARQUS_BIN
else
    cargo build --quiet --bin arqus || exit 1
    BIN="$ROOT/target/debug/arqus"
fi

if [ -n "${ARQUS_FAKE:-}" ]; then
    FAKE=$ARQUS_FAKE
else
    cargo build --quiet --example fake_llm || exit 1
    FAKE="$ROOT/target/debug/examples/fake_llm"
fi

. "$ROOT/scripts/tui-smoke-lib.sh"

scenario_a() {
    fresh a
    printf '[]\n' >"$SCRIPT_FILE"
    start_server || return 1
    start "" || return 1
    expect "Connect a provider" 20 || return 1
    type "open"
    expect "OpenRouter" 5 || return 1
    expect_absent "Ollama (local)" 5 || return 1
    keys Escape
    keys Escape
    expect "no provider connected" 5 || return 1
}

scenario_b() {
    fresh b
    {
        printf '[\n'
        key_reply
        printf ',\n'
        models_reply
        printf '\n]\n'
    } >"$SCRIPT_FILE"
    start_server || return 1
    start "" || return 1
    expect "Connect a provider" 20 || return 1
    type "open"
    expect "OpenRouter" 5 || return 1
    expect_absent "Ollama (local)" 5 || return 1
    keys Enter
    expect "create one:" 5 || return 1
    type "$SECRET"
    keys Enter
    expect "Key OK" 10 || return 1
    expect "Select model · openrouter" 15 || return 1
    expect "smoke-model" 5 || return 1
    keys Enter
    expect "openrouter/smoke-model" 10 || return 1
}

scenario_c() {
    fresh c
    {
        printf '[\n'
        key_reply
        printf ',\n'
        models_reply
        printf ',\n'
        printf '{"kind":"text","pieces":["Hello"," from"," the smoke test."]}\n'
        printf ']\n'
    } >"$SCRIPT_FILE"
    start_server || return 1
    start "" || return 1
    connect_openrouter || return 1
    type "hello"
    keys Enter
    expect "Hello from the smoke test." 15 || return 1
    expect "ctx" 5 || return 1
}

scenario_d() {
    fresh d
    {
        printf '[\n'
        key_reply
        printf ',\n'
        models_reply
        printf ',\n'
        printf '{"kind":"tool","name":"bash","args":{"command":"echo smoke > out.txt"}},\n'
        printf '{"kind":"text","pieces":["Wrote out.txt."]}\n'
        printf ']\n'
    } >"$SCRIPT_FILE"
    start_server || return 1
    start "" || return 1
    connect_openrouter || return 1
    type "run the smoke tool"
    keys Enter
    expect "Run command?" 10 || return 1
    expect "echo smoke > out.txt" 5 || return 1
    keys "y"
    expect "Wrote out.txt." 15 || return 1
    expect "✓" 5 || return 1
    if [ ! -f "$PROJ/out.txt" ]; then
        echo "  missing $PROJ/out.txt"
        return 1
    fi
    if ! grep -qx "smoke" "$PROJ/out.txt"; then
        echo "  out.txt has the wrong content"
        cat "$PROJ/out.txt"
        return 1
    fi
}

scenario_e() {
    fresh e
    {
        printf '[\n'
        key_reply
        printf ',\n'
        models_reply
        printf ',\n'
        printf '{"kind":"tool","name":"bash","args":{"command":"echo smoke > out.txt"}},\n'
        printf '{"kind":"text","pieces":["Denied."," No file written."]}\n'
        printf ']\n'
    } >"$SCRIPT_FILE"
    start_server || return 1
    start "" || return 1
    connect_openrouter || return 1
    type "run the smoke tool"
    keys Enter
    expect "Run command?" 10 || return 1
    keys "n"
    expect "Denied. No file written." 15 || return 1
    expect "✗" 5 || return 1
    if [ -f "$PROJ/out.txt" ]; then
        echo "  out.txt must not exist after a deny"
        return 1
    fi
}

scenario_f() {
    fresh f
    printf '[]\n' >"$SCRIPT_FILE"
    start_server || return 1
    start "" || return 1
    expect "Connect a provider" 20 || return 1
    keys Escape
    expect "no provider connected" 5 || return 1
    type "/mo"
    expect "/models" 5 || return 1
    keys Escape
    keys "C-p"
    expect "Commands" 5 || return 1
    keys Escape
    expect_absent "Commands" 5 || return 1
}

scenario_g() {
    fresh gh
    {
        printf '[\n'
        key_reply
        printf ',\n'
        models_reply
        printf ',\n'
        printf '{"kind":"text","pieces":["Saved"," and quit."]}\n'
        printf ']\n'
    } >"$SCRIPT_FILE"
    start_server || return 1
    start "" || return 1
    connect_openrouter || return 1
    type "hello"
    keys Enter
    expect "Saved and quit." 15 || return 1
    keys "C-d"
    expect "__ARQUS_EXITED__" 10 || return 1
    type 'echo SHELL_$(echo ALIVE)'
    keys Enter
    expect "SHELL_ALIVE" 5 || return 1
    if grep -Fq "$SECRET" "$SCREENS"; then
        echo "  the secret appeared on a captured screen"
        return 1
    fi
    if [ ! -f "$APPHOME/debug.log" ]; then
        echo "  ARQUS_DEBUG=1 should have written $APPHOME/debug.log"
        return 1
    fi
    if grep -Fq "$SECRET" "$APPHOME/debug.log"; then
        echo "  the secret appeared in debug.log"
        return 1
    fi
}

scenario_h() {
    printf '[]\n' >"$SCRIPT_FILE"
    start_server || return 1
    start "" || return 1
    expect "openrouter · smoke-model" 20 || return 1
    expect_absent "Connect a provider" 2 || return 1
    expect_absent "Select model" 2 || return 1
}

run_scenario() {
    _name=$1
    echo "scenario $_name"
    if "$_name"; then
        echo "PASS: $_name"
    else
        echo "FAIL: $_name"
        screen
        FAILS=$((FAILS + 1))
    fi
}

run_scenario scenario_a
run_scenario scenario_b
run_scenario scenario_c
run_scenario scenario_d
run_scenario scenario_e
run_scenario scenario_f
run_scenario scenario_g
run_scenario scenario_h

if [ "$FAILS" -ne 0 ]; then
    echo "$FAILS scenario(s) failed"
    exit 1
fi
echo "all scenarios passed"
exit 0
