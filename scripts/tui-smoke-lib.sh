#!/bin/sh
# Helpers for scripts/tui-smoke.sh. Sourced, never run directly.
# Globals come from the caller: SESSION, WORK, BIN, FAKE, and the rest.

screen() {
    tmux capture-pane -p -t "$SESSION" 2>/dev/null || true
}

capture() {
    screen >"$LAST"
    cat "$LAST" >>"$SCREENS"
}

wait_for() {
    _needle=$1
    _limit=${2:-10}
    _i=0
    while [ "$_i" -lt $((_limit * 10)) ]; do
        capture
        if grep -Fq -- "$_needle" "$LAST"; then
            return 0
        fi
        sleep 0.1
        _i=$((_i + 1))
    done
    return 1
}

expect() {
    if wait_for "$1" "${2:-10}"; then
        return 0
    fi
    echo "  missing text: $1"
    cat "$LAST"
    return 1
}

expect_absent() {
    _needle=$1
    _limit=${2:-5}
    _i=0
    while [ "$_i" -lt $((_limit * 10)) ]; do
        capture
        if ! grep -Fq -- "$_needle" "$LAST"; then
            return 0
        fi
        sleep 0.1
        _i=$((_i + 1))
    done
    echo "  unexpected text: $1"
    cat "$LAST"
    return 1
}

keys() {
    tmux send-keys -t "$SESSION" "$@"
}

type() {
    tmux send-keys -t "$SESSION" -l "$1"
}

stop() {
    tmux kill-session -t "$SESSION" 2>/dev/null || true
}

stop_server() {
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
        SERVER_PID=""
    fi
}

fresh() {
    stop
    stop_server
    APPHOME="$WORK/home-$1"
    PROJ="$WORK/proj-$1"
    mkdir -p "$APPHOME" "$PROJ"
    : >"$SERVER_LOG"
    : >"$LAST"
}

start_server() {
    stop_server
    _attempt=0
    while [ "$_attempt" -lt 8 ]; do
        _attempt=$((_attempt + 1))
        PORT=$((20000 + (($$ * 131 + _attempt * 977) % 20000)))
        : >"$SERVER_LOG"
        "$FAKE" "$PORT" "$SCRIPT_FILE" >"$SERVER_LOG" 2>&1 &
        SERVER_PID=$!
        _i=0
        while [ "$_i" -lt 100 ]; do
            if grep -Fq "listening on $PORT" "$SERVER_LOG" 2>/dev/null; then
                BASE_URL="http://127.0.0.1:$PORT/v1"
                return 0
            fi
            if ! kill -0 "$SERVER_PID" 2>/dev/null; then
                break
            fi
            sleep 0.05
            _i=$((_i + 1))
        done
        stop_server
    done
    echo "  fake server did not start"
    cat "$SERVER_LOG"
    return 1
}

start() {
    stop
    cat >"$LAUNCH" <<EOF
#!/bin/sh
env -i HOME='$HOME' PATH='$PATH' TERM='screen-256color' \
    ARQUS_HOME='$APPHOME' ARQUS_BASE_URL='$BASE_URL' ARQUS_DEBUG=1 '$BIN' ${1:-}
printf '\\n__ARQUS_EXITED__\\n'
exec /bin/sh -i
EOF
    tmux new-session -d -x 100 -y 30 -s "$SESSION" -c "$PROJ" "sh '$LAUNCH'"
}

connect_openrouter() {
    expect "Connect a provider" 20 || return 1
    type "open"
    expect "OpenRouter" 5 || return 1
    expect_absent "Ollama (local)" 5 || return 1
    keys Enter
    expect "create one:" 5 || return 1
    type "$SECRET"
    keys Enter
    expect "Key OK" 10 || return 1
    expect "smoke-model" 15 || return 1
    keys Enter
    expect "openrouter/smoke-model" 10 || return 1
}

models_reply() {
    cat <<'EOF'
{"kind":"json","body":{"data":[
  {"id":"smoke-model","name":"Smoke Model","context_length":128000,
   "pricing":{"prompt":"0.0000001","completion":"0.0000002"},
   "supported_parameters":["tools"]},
  {"id":"smoke-chat","name":"Smoke Chat","context_length":8192,
   "pricing":{"prompt":"0","completion":"0"},"supported_parameters":[]},
  {"id":"smoke-mini","name":"Smoke Mini","context_length":4096}
]}}
EOF
}

key_reply() {
    cat <<'EOF'
{"kind":"json","body":{"data":{"limit_remaining":9.5,"limit":10}}}
EOF
}

