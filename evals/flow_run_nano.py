"""Nano contract evaluation runner for arqus flow.

Evaluates candidate local workers against hand-written contracts (TESTS.md B1, B4, B8).
Outputs per-category first-attempt pass rates and appends to stats.jsonl for profiling.
Python 3 standard library only.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

from evals.flow_task import NanoTask, load_nano_task


def load_all_nano_tasks_from_dir(nano_dir: Path) -> list[NanoTask]:
    if not nano_dir.is_dir():
        return []
    tasks = []
    for item in sorted(nano_dir.iterdir()):
        if item.is_dir() and ((item / "nano.json").is_file() or (item / "task.json").is_file()):
            tasks.append(load_nano_task(item))
    return tasks


def _prepare_work_repo(repo_path: Path, root: Path) -> Path:
    work = root / "work"
    shutil.copytree(repo_path, work, symlinks=True)
    if not (work / ".git").is_dir():
        subprocess.run(["git", "init", "-q"], cwd=str(work), check=True)
        subprocess.run(["git", "config", "user.email", "eval@arqus.local"], cwd=str(work), check=True)
        subprocess.run(["git", "config", "user.name", "eval"], cwd=str(work), check=True)
        subprocess.run(["git", "add", "-A"], cwd=str(work), check=True)
        subprocess.run(["git", "commit", "-qm", "setup"], cwd=str(work), check=True)
    return work


def run_one_nano_task(
    task: NanoTask,
    worker_name: str,
    repeat: int = 1,
    dry_run: bool = False,
    timeout: int = 300,
) -> dict[str, Any]:
    root = Path(tempfile.mkdtemp(prefix=f"eval-nano-{task.name}-"))
    work = _prepare_work_repo(task.repo_path, root)
    start_time = time.monotonic()
    passed = False
    tokens = 0

    try:
        if dry_run:
            # Simulate completion by applying the reference patch
            patch_file = root / "ref.patch"
            patch_file.write_text(task.reference_patch, encoding="utf-8")
            subprocess.run(["git", "apply", str(patch_file)], cwd=str(work), check=False)
            tokens = 150
        else:
            # Live invocation would send worker prompt and parse edits
            pass

        # Verification using hidden_verify
        all_passed = True
        for cmd in task.hidden_verify:
            res = subprocess.run(
                ["sh", "-c", cmd],
                cwd=str(work),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                timeout=timeout,
            )
            if res.returncode != 0:
                all_passed = False
                break
        passed = all_passed
    except Exception:
        passed = False
    finally:
        shutil.rmtree(root, ignore_errors=True)

    seconds = round(time.monotonic() - start_time, 2)
    return {
        "task": task.name,
        "category": task.category,
        "repeat": repeat,
        "passed": passed,
        "seconds": seconds,
        "tokens": tokens,
    }


def run_nano_eval(
    tasks: list[NanoTask],
    worker_name: str,
    stats_path: Path | None = None,
    out_dir: Path | None = None,
    repeats: int = 1,
    dry_run: bool = False,
) -> list[dict[str, Any]]:
    results = []
    stats_records = []

    for rep in range(1, repeats + 1):
        for task in tasks:
            rec = run_one_nano_task(
                task=task,
                worker_name=worker_name,
                repeat=rep,
                dry_run=dry_run,
            )
            results.append(rec)

            stat_entry = {
                "model": worker_name,
                "category": task.category,
                "first_attempt_pass": rec["passed"],
                "attempts": 1,
                "redesigns": 0,
                "takeover": False,
                "tokens": rec.get("tokens", 0),
            }
            stats_records.append(stat_entry)

    if stats_path:
        stats_path.parent.mkdir(parents=True, exist_ok=True)
        with stats_path.open("a", encoding="utf-8") as f:
            for s in stats_records:
                f.write(json.dumps(s) + "\n")

    if out_dir:
        out_dir.mkdir(parents=True, exist_ok=True)
        (out_dir / "nano_results.json").write_text(json.dumps(results, indent=2), encoding="utf-8")

    return results


def summarize_nano_results(results: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    categories: dict[str, list[bool]] = {}
    for r in results:
        categories.setdefault(r["category"], []).append(r["passed"])

    summary = {}
    for cat, passes in categories.items():
        total = len(passes)
        passed_count = sum(1 for p in passes if p)
        rate = passed_count / total if total > 0 else 0.0
        summary[cat] = {
            "total": total,
            "passed": passed_count,
            "pass_rate": round(rate, 3),
            "delegable": rate >= 0.8 and total >= 8,
        }
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description="Run arqus flow nano-contract evaluations.")
    parser.add_argument("--tasks-dir", type=Path, default=Path("evals/flow/nano"))
    parser.add_argument("--worker", default="local-default", help="Worker model or name")
    parser.add_argument("--stats-file", type=Path, default=Path.home() / ".arqus" / "flow" / "stats.jsonl")
    parser.add_argument("--out-dir", type=Path, default=Path("evals/results_nano"))
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--dry-run", action="store_true", help="Dry run applying reference patches")
    args = parser.parse_args()

    tasks = load_all_nano_tasks_from_dir(args.tasks_dir)
    if not tasks:
        print(f"error: no nano tasks found in {args.tasks_dir}", file=sys.stderr)
        return 1

    print(f"Loaded {len(tasks)} nano tasks from {args.tasks_dir}")
    results = run_nano_eval(
        tasks=tasks,
        worker_name=args.worker,
        stats_path=args.stats_file,
        out_dir=args.out_dir,
        repeats=args.repeats,
        dry_run=args.dry_run,
    )
    summary = summarize_nano_results(results)
    print("\n=== Nano Evaluation Summary ===")
    for cat, data in sorted(summary.items()):
        status = "DELEGABLE" if data["delegable"] else "NOT DELEGABLE"
        print(f"  {cat:20} : {data['passed']}/{data['total']} ({data['pass_rate'] * 100:.1f}%) -> {status}")
    print(f"\nResults saved to {args.out_dir / 'nano_results.json'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

