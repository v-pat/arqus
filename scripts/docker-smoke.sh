#!/bin/sh
# Build the sandbox image and prove it is isolated and usable.
# Usage: scripts/docker-smoke.sh
set -eu
cd "$(dirname "$0")/.."

IMAGE="${PIRS_IMAGE:-pirs:dev}"
FAILED=0

pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAILED=1; }

echo "==> docker build -t $IMAGE ."
docker build -t "$IMAGE" .

echo "==> check: container user is not root"
if UID_OUT=$(docker run --rm --entrypoint id "$IMAGE" -u 2>&1); then
    if [ "$UID_OUT" != "0" ]; then
        pass "container runs as uid $UID_OUT, not root"
    else
        fail "container runs as root"
    fi
else
    fail "could not read the container user: $UID_OUT"
fi

echo "==> check: pirs --version works in the container"
if VERSION_OUT=$(docker run --rm "$IMAGE" --version 2>&1); then
    pass "pirs --version -> $VERSION_OUT"
else
    fail "pirs --version failed: $VERSION_OUT"
fi

echo "==> check: a mounted project folder is writable both ways"
PROJ=$(mktemp -d "${TMPDIR:-/tmp}/pirs-smoke.XXXXXX")
trap 'rm -rf "$PROJ"' EXIT INT TERM
if docker run --rm -v "$PROJ":/work --entrypoint sh "$IMAGE" -c 'echo hello > /work/from-container.txt' 2>&1; then
    if [ -f "$PROJ/from-container.txt" ] && [ "$(cat "$PROJ/from-container.txt")" = "hello" ]; then
        pass "file written in the container appears on the host"
    else
        fail "container wrote /work/from-container.txt but the host cannot see it"
    fi
else
    fail "container could not write to the mounted /work"
fi

echo "==> check: /home/agent/.ssh does not exist"
if docker run --rm --entrypoint sh "$IMAGE" -c 'test -d /home/agent/.ssh' 2>/dev/null; then
    fail "/home/agent/.ssh exists in the container"
else
    pass "/home/agent/.ssh does not exist"
fi

echo "==> check: the host home folder is not visible"
HOME_OUT=$(docker run --rm --entrypoint sh "$IMAGE" -c 'ls /Users 2>/dev/null; ls /home' 2>&1)
if [ "$HOME_OUT" = "agent" ]; then
    pass "no host home visible (/Users absent, /home holds only agent)"
else
    fail "unexpected container paths: $HOME_OUT"
fi

if [ "$FAILED" -ne 0 ]; then
    echo "smoke test FAILED" >&2
    exit 1
fi
echo "smoke test PASSED"
