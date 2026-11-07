#!/usr/bin/env python3
"""Run small coding tasks with pirs and record pass rate, time and cost.

Python 3 standard library only. See README.md for the task layout and the
tuning loop rules.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

EVALS_DIR = Path(__file__).resolve().parent
REPO_DIR = EVALS_DIR.parent
TASKS_DIR = EVALS_DIR / "tasks"
RESULTS_FILE = EVALS_DIR / "results.jsonl"
DEFAULT_BIN = Path("target/release/pirs")
TIMEOUT_SECONDS = 300
CHECK_TIMEOUT_SECONDS = 60
DEFAULT_MAX_COST = 0.50

REQUESTS_RE = re.compile(r"requests=(\d+)")
COST_RE = re.compile(r"cost=\$([0-9]+(?:\.[0-9]+)?)")


def available_tasks() -> list[str]:
    if not TASKS_DIR.is_dir():
        return []
    return sorted(
        path.name
        for path in TASKS_DIR.iterdir()
        if path.is_dir() and (path / "prompt.txt").is_file()
    )


def run_process(cmd: list[str], cwd: Path, env: dict[str, str], timeout: int):
    """Run a command in its own session. Kill the whole group on timeout."""
    proc = subprocess.Popen(
        cmd,
        cwd=str(cwd),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        errors="replace",
        start_new_session=True,
    )
    timed_out = False
    try:
        out, err = proc.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        out, err = proc.communicate()
    return proc.returncode, out, err, timed_out


def git(cwd: Path, *args: str) -> None:
    subprocess.run(
        ["git", *args],
        cwd=str(cwd),
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )


def pirs_numbers(stderr: str) -> tuple[int | None, float | None]:
    """Read requests= and cost= from the last [pirs] summary line."""
    requests = None
    cost = None
    for line in stderr.splitlines():
        if not line.startswith("[pirs]") or "requests=" not in line:
            continue
        requests = None
        cost = None
        match = REQUESTS_RE.search(line)
        if match:
            requests = int(match.group(1))
        match = COST_RE.search(line)
        if match:
            cost = float(match.group(1))
    return requests, cost


def repo_commit() -> str:
    try:
        out = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=str(REPO_DIR),
            capture_output=True,
            text=True,
            check=True,
        )
        return out.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return ""


def run_one(
    name: str, bin_path: Path, model_spec: str, model_label: str
) -> tuple[dict, str]:
    """Run one task in a fresh git project. Return (record, failure reason)."""
    task = TASKS_DIR / name
    for required in ("prompt.txt", "setup", "check.sh"):
        if not (task / required).exists():
            raise SystemExit(f"task {name}: missing {required}")
    prompt = (task / "prompt.txt").read_text().strip()

    root = Path(tempfile.mkdtemp(prefix=f"pirs-eval-{name}-"))
    work = root / "work"
    home = root / "home"
    home.mkdir()
    shutil.copytree(task / "setup", work)
    git(work, "init", "-q")
    git(work, "add", "-A")
    git(
        work,
        "-c",
        "user.email=eval@pirs.local",
        "-c",
        "user.name=pirs eval",
        "commit",
        "-qm",
        "setup",
    )

    cmd = [str(bin_path), "--no-session", "--yolo", "-p", prompt]
    if model_spec:
        cmd.extend(["--model", model_spec])
    env = dict(os.environ)
    env["PIRS_HOME"] = str(home)

    started = time.monotonic()
    exit_code, stdout, stderr, timed_out = run_process(
        cmd, work, env, TIMEOUT_SECONDS
    )
    seconds = time.monotonic() - started

    stdout_path = root / "stdout.txt"
    stderr_path = root / "stderr.txt"
    stdout_path.write_text(stdout)
    stderr_path.write_text(stderr)

    check_env = dict(env)
    check_env["PIRS_OUT"] = str(stdout_path)
    check_code, check_out, check_err, check_timed_out = run_process(
        ["sh", str(task / "check.sh")], work, check_env, CHECK_TIMEOUT_SECONDS
    )

    passed = check_code == 0 and not timed_out and not check_timed_out
    requests, cost = pirs_numbers(stderr)

    reason = ""
    if not passed:
        if timed_out:
            reason = f"pirs timed out after {TIMEOUT_SECONDS}s"
        elif check_timed_out:
            reason = f"check timed out after {CHECK_TIMEOUT_SECONDS}s"
        else:
            detail = (check_err or check_out or "").strip().splitlines()
            reason = detail[-1] if detail else f"check exited {check_code}"
        reason = f"{reason} (pirs exit {exit_code}, logs: {root})"

    record = {
        "task": name,
        "passed": passed,
        "seconds": round(seconds, 2),
        "requests": requests,
        "cost": cost,
        "model": model_label,
        "git_commit": repo_commit(),
        "ts": datetime.now(timezone.utc).isoformat(timespec="seconds"),
    }
    return record, reason


def append_result(record: dict) -> None:
    with RESULTS_FILE.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record) + "\n")


def mean(values: list[float]) -> float | None:
    return sum(values) / len(values) if values else None


def print_table(records: list[dict]) -> None:
    rows: dict[str, list[dict]] = {}
    for record in records:
        rows.setdefault(record["task"], []).append(record)

    print()
    print(f"{'task':<20} {'pass':>7} {'runs':>5} {'avg s':>8} {'avg $':>10}")
    print("-" * 54)
    total_runs = 0
    total_passes = 0
    total_cost = 0.0
    total_known_costs = 0
    total_seconds: list[float] = []
    for name in sorted(rows):
        group = rows[name]
        passes = sum(1 for record in group if record["passed"])
        costs = [record["cost"] for record in group if record["cost"] is not None]
        seconds = [record["seconds"] for record in group]
        avg_cost = mean(costs)
        cost_text = f"{avg_cost:.4f}" if avg_cost is not None else "n/a"
        total_cost += sum(costs)
        total_known_costs += len(costs)
        print(
            f"{name:<20} {passes:>4}/{len(group):<3} {len(group):>5} "
            f"{mean(seconds):>8.1f} {cost_text:>10}"
        )
        total_runs += len(group)
        total_passes += passes
        total_seconds.extend(seconds)
    print("-" * 54)
    avg_seconds = mean(total_seconds)
    total_cost_text = f"{total_cost:.4f}" if total_known_costs else "n/a"
    print(
        f"{'total':<20} {total_passes:>4}/{total_runs:<3} {total_runs:>5} "
        f"{avg_seconds:>8.1f} {total_cost_text:>10}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Run pirs evals.")
    parser.add_argument(
        "--tasks",
        help="comma separated task names (default: every task)",
    )
    parser.add_argument("--repeat", type=int, default=1, help="runs per task")
    parser.add_argument(
        "--bin",
        default=str(DEFAULT_BIN),
        help="pirs binary (default: target/release/pirs)",
    )
    parser.add_argument(
        "--model",
        default="",
        help="provider:model passed to pirs (default: use PIRS_MODEL)",
    )
    args = parser.parse_args()
    model_label = args.model or os.environ.get("PIRS_MODEL", "") or "default"

    if args.repeat < 1:
        parser.error("--repeat must be at least 1")

    all_tasks = available_tasks()
    if not all_tasks:
        print(f"error: no tasks found in {TASKS_DIR}", file=sys.stderr)
        return 2
    if args.tasks:
        selected = [part.strip() for part in args.tasks.split(",") if part.strip()]
        unknown = [name for name in selected if name not in all_tasks]
        if unknown:
            print(f"error: unknown task(s): {', '.join(unknown)}", file=sys.stderr)
            print(f"known: {', '.join(all_tasks)}", file=sys.stderr)
            return 2
    else:
        selected = all_tasks

    bin_path = Path(args.bin)
    if not bin_path.is_absolute():
        bin_path = REPO_DIR / bin_path
    if not bin_path.is_file():
        print(f"error: pirs binary not found: {bin_path}", file=sys.stderr)
        print("build it with: cargo build --release", file=sys.stderr)
        return 2

    max_cost = float(os.environ.get("EVAL_MAX_COST", DEFAULT_MAX_COST))
    records: list[dict] = []
    spent = 0.0
    runs = [(name, index) for name in selected for index in range(args.repeat)]
    stopped = False

    for position, (name, repeat_index) in enumerate(runs, start=1):
        label = f"{position}/{len(runs)} {name}"
        if args.repeat > 1:
            label += f" #{repeat_index + 1}"
        print(f"{label} ...", end=" ", flush=True)
        record, reason = run_one(name, bin_path, args.model, model_label)
        append_result(record)
        records.append(record)
        if record["cost"] is not None:
            spent += record["cost"]
        status = "PASS" if record["passed"] else "FAIL"
        cost_text = f"${record['cost']:.4f}" if record["cost"] is not None else "cost n/a"
        print(f"{status} {record['seconds']:.1f}s {cost_text}")
        if reason:
            print(f"  {reason}")
        if spent > max_cost:
            print(
                f"stopped: total cost ${spent:.4f} exceeds EVAL_MAX_COST "
                f"${max_cost:.2f}",
                file=sys.stderr,
            )
            stopped = True
            break

    print_table(records)
    if stopped:
        print(f"stopped early: cost cap ${max_cost:.2f} reached")
    return 0 if records and all(record["passed"] for record in records) else 1


if __name__ == "__main__":
    sys.exit(main())
