"""Evaluation runner for arqus flow.

Runs conditions D0 and H1 to H5 with repeats and interleaving.
Python 3 standard library only.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

from evals.flow_task import TaskDefinition, load_tasks_from_dir
from evals.flow_validity import _is_git_clean


def build_run_schedule(
    task_names: list[str], conditions: list[str], repeats: int
) -> list[dict[str, Any]]:
    """Build interleaved schedule across tasks, conditions, and repeats."""
    schedule = []
    for rep in range(1, repeats + 1):
        for task in task_names:
            for cond in conditions:
                schedule.append({"task": task, "condition": cond, "repeat": rep})
    return schedule


def run_process(cmd: list[str], cwd: Path, env: dict[str, str], timeout: int = 300):
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


def _prepare_work_repo(task: TaskDefinition, root: Path) -> Path:
    work = root / "work"
    shutil.copytree(task.repo_path, work, symlinks=True)
    if not (work / ".git").is_dir():
        subprocess.run(["git", "init", "-q"], cwd=str(work), check=True)
        subprocess.run(["git", "config", "user.email", "eval@arqus.local"], cwd=str(work), check=True)
        subprocess.run(["git", "config", "user.name", "eval"], cwd=str(work), check=True)
        subprocess.run(["git", "add", "-A"], cwd=str(work), check=True)
        subprocess.run(["git", "commit", "-qm", "setup"], cwd=str(work), check=True)
    return work


def _configure_settings_for_condition(
    condition: str, home_dir: Path, alt_planner: str = "", alt_worker: str = ""
) -> None:
    settings_file = home_dir / "settings.json"
    settings: dict[str, Any] = {}
    if settings_file.is_file():
        try:
            settings = json.loads(settings_file.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            pass

    flow_cfg = settings.setdefault("flow", {})
    limits = flow_cfg.setdefault("limits", {})

    if condition == "H2":
        limits["max_redesigns"] = 0
    elif condition == "H3":
        limits["max_worker_attempts"] = 1
        limits["max_redesigns"] = 0
    elif condition == "H4" and alt_planner:
        flow_cfg.setdefault("orchestrator", {})["model"] = alt_planner
    elif condition == "H5" and alt_worker:
        flow_cfg.setdefault("routing", {})["default_worker"] = alt_worker

    settings_file.write_text(json.dumps(settings, indent=2), encoding="utf-8")


def _read_flow_usage(work: Path) -> tuple[int, float, list[dict[str, Any]]]:
    flow_dir = work / ".arqus" / "flow"
    if not flow_dir.is_dir():
        return 0, 0.0, []
    run_dirs = [d for d in flow_dir.iterdir() if d.is_dir() and d.name != "worktree"]
    if not run_dirs:
        return 0, 0.0, []
    latest_run = max(run_dirs, key=lambda d: d.stat().st_mtime)
    usage_file = latest_run / "usage.jsonl"
    if not usage_file.is_file():
        return 0, 0.0, []

    records = []
    cloud_tokens = 0
    cost_usd = 0.0
    with usage_file.open("r", encoding="utf-8") as f:
        for line in f:
            if not line.strip():
                continue
            try:
                rec = json.loads(line)
                records.append(rec)
                if rec.get("role") in ("planner", "redesign", "takeover"):
                    tot = rec.get("input", 0) + rec.get("output", 0) + rec.get("cache_read", 0) + rec.get("cache_write", 0)
                    cloud_tokens += tot
                    if rec.get("cost_usd") is not None:
                        cost_usd += float(rec["cost_usd"])
            except json.JSONDecodeError:
                pass
    return cloud_tokens, cost_usd, records


def run_one_task(
    task: TaskDefinition,
    condition: str,
    repeat: int,
    bin_path: Path,
    env_base: dict[str, str],
    timeout: int = 300,
    dry_run: bool = False,
    alt_planner: str = "",
    alt_worker: str = "",
) -> dict[str, Any]:
    root = Path(tempfile.mkdtemp(prefix=f"eval-{condition}-{task.name}-"))
    work = _prepare_work_repo(task, root)
    home = root / "home"
    home.mkdir()

    env = dict(env_base)
    env["ARQUS_HOME"] = str(home)
    _configure_settings_for_condition(condition, home, alt_planner, alt_worker)

    cmd = (
        [str(bin_path), "--yolo", "-p", task.objective]
        if condition == "D0"
        else [str(bin_path), "flow", "run", "--repo", ".", task.objective]
    )

    started = time.monotonic()
    timed_out = False
    exit_code = 0
    if not dry_run:
        exit_code, stdout, stderr, timed_out = run_process(cmd, work, env, timeout)
        (root / "stdout.txt").write_text(stdout, encoding="utf-8")
        (root / "stderr.txt").write_text(stderr, encoding="utf-8")
    else:
        # Dry-run mock: apply patch to simulate completion
        patch_file = root / "ref.patch"
        patch_file.write_text(task.reference_patch, encoding="utf-8")
        subprocess.run(["git", "apply", str(patch_file)], cwd=str(work), check=False)
    seconds = time.monotonic() - started

    # Copy hidden test files after model run
    if task.hidden_dir and task.hidden_dir.is_dir():
        for item in task.hidden_dir.iterdir():
            target = work / item.name
            if item.is_dir():
                shutil.copytree(item, target, dirs_exist_ok=True)
            else:
                shutil.copy2(item, target)

    # Verification: pass if regression and hidden both pass
    reg_ok = all(
        subprocess.run(["sh", "-c", c], cwd=str(work), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
        for c in task.regression_verify
    )
    hid_ok = all(
        subprocess.run(["sh", "-c", c], cwd=str(work), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
        for c in task.hidden_verify
    )
    passed = reg_ok and hid_ok and not timed_out and (dry_run or exit_code == 0)

    cloud_tokens, cost_usd, _ = _read_flow_usage(work) if not dry_run else (100, 0.001, [])
    shutil.rmtree(root, ignore_errors=True)

    return {
        "task": task.name,
        "condition": condition,
        "repeat": repeat,
        "passed": passed,
        "seconds": round(seconds, 2),
        "cloud_tokens": cloud_tokens,
        "cost_usd": cost_usd,
        "regression": not reg_ok,
        "scope_violations": 0,
    }


def run_eval_matrix(
    tasks: list[TaskDefinition],
    conditions: list[str],
    repeats: int = 1,
    bin_path: Path | None = None,
    output_dir: Path | None = None,
    dry_run: bool = False,
    alt_planner: str = "",
    alt_worker: str = "",
) -> list[dict[str, Any]]:
    task_map = {t.name: t for t in tasks}
    schedule = build_run_schedule(list(task_map.keys()), conditions, repeats)
    results = []

    for item in schedule:
        task = task_map[item["task"]]
        cond = item["condition"]
        rep = item["repeat"]
        rec = run_one_task(
            task=task,
            condition=cond,
            repeat=rep,
            bin_path=bin_path or Path("target/release/arqus"),
            env_base=dict(os.environ),
            dry_run=dry_run,
            alt_planner=alt_planner,
            alt_worker=alt_worker,
        )
        results.append(rec)

    if output_dir:
        output_dir.mkdir(parents=True, exist_ok=True)
        (output_dir / "results.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description="Run arqus flow evaluation suite.")
    parser.add_argument("--tasks-dir", type=Path, default=Path("evals/flow/tasks"))
    parser.add_argument("--conditions", default="D0,H1", help="Comma separated conditions (e.g. D0,H1,H2)")
    parser.add_argument("--repeat", type=int, default=3, help="Repeats per task and condition")
    parser.add_argument("--bin", type=Path, default=Path("target/release/arqus"))
    parser.add_argument("--out-dir", type=Path, default=Path("evals/results"))
    parser.add_argument("--dry-run", action="store_true", help="Simulate without live arqus calls")
    args = parser.parse_args()

    tasks = load_tasks_from_dir(args.tasks_dir)
    if not tasks:
        print(f"error: no tasks found in {args.tasks_dir}", file=sys.stderr)
        return 1

    conditions = [c.strip() for c in args.conditions.split(",") if c.strip()]
    results = run_eval_matrix(
        tasks=tasks,
        conditions=conditions,
        repeats=args.repeat,
        bin_path=args.bin,
        output_dir=args.out_dir,
        dry_run=args.dry_run,
    )
    print(f"Completed {len(results)} runs. Results saved to {args.out_dir / 'results.json'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

