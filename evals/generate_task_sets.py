"""Generator and validator for the 30-task judging set and 60-contract nano set.

Builds all required directories, setup repos, test files, and reference patches.
Runs git commit on setup repos and verifies validity before completing.
Python 3 standard library only.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent
JUDGING_DIR = BASE_DIR / "flow" / "judging"
NANO_DIR = BASE_DIR / "flow" / "nano"


def run_cmd(cmd: list[str], cwd: Path) -> None:
    res = subprocess.run(cmd, cwd=str(cwd), stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode != 0:
        raise RuntimeError(f"Command failed ({' '.join(cmd)}) in {cwd}:\n{res.stderr}")


def init_git_repo(repo_dir: Path) -> None:
    git_dir = repo_dir / ".git"
    if git_dir.is_dir():
        shutil.rmtree(git_dir)
    run_cmd(["git", "init", "-q"], repo_dir)
    run_cmd(["git", "config", "user.email", "eval@arqus.local"], repo_dir)
    run_cmd(["git", "config", "user.name", "eval"], repo_dir)
    run_cmd(["git", "add", "-A"], repo_dir)
    run_cmd(["git", "commit", "-qm", "initial commit"], repo_dir)


def make_patch(before_file: str, before_content: str, after_content: str) -> str:
    """Create a unified diff string between before and after content."""
    import difflib
    lines1 = before_content.splitlines(keepends=True)
    lines2 = after_content.splitlines(keepends=True)
    diff = difflib.unified_diff(lines1, lines2, fromfile=f"a/{before_file}", tofile=f"b/{before_file}")
    return "".join(diff)


def create_judging_task(
    name: str,
    language: str,
    size: str,
    category: str,
    objective: str,
    setup_files: dict[str, str],
    reference_files: dict[str, str],
    public_verify: list[str],
    hidden_verify: list[str],
    regression_verify: list[str],
    hidden_files: dict[str, str] | None = None,
    protected: list[str] | None = None,
) -> None:
    task_dir = JUDGING_DIR / name
    if task_dir.is_dir():
        shutil.rmtree(task_dir)
    task_dir.mkdir(parents=True, exist_ok=True)
    setup_dir = task_dir / "setup"
    setup_dir.mkdir(parents=True, exist_ok=True)

    # Write initial files into setup
    for fname, content in setup_files.items():
        fpath = setup_dir / fname
        fpath.parent.mkdir(parents=True, exist_ok=True)
        fpath.write_text(content, encoding="utf-8")

    # Initialize git in setup
    init_git_repo(setup_dir)

    # Write hidden files if any
    hidden_dir = task_dir / "hidden"
    if hidden_files:
        hidden_dir.mkdir(parents=True, exist_ok=True)
        for fname, content in hidden_files.items():
            fpath = hidden_dir / fname
            fpath.parent.mkdir(parents=True, exist_ok=True)
            fpath.write_text(content, encoding="utf-8")

    # Generate reference.patch
    patch_parts = []
    for fname, after_content in reference_files.items():
        before_content = setup_files.get(fname, "")
        p = make_patch(fname, before_content, after_content)
        if p:
            patch_parts.append(p)
    patch_text = "\n".join(patch_parts)
    if not patch_text.endswith("\n"):
        patch_text += "\n"
    (task_dir / "reference.patch").write_text(patch_text, encoding="utf-8")

    # Write task.json
    task_json = {
        "name": name,
        "language": language,
        "size": size,
        "category": category,
        "repo": "setup",
        "objective": objective,
        "public_verify": public_verify,
        "hidden_verify": hidden_verify,
        "regression_verify": regression_verify,
        "protected": protected or ["test_*.py", "*_test.*"],
        "reference_patch": "reference.patch",
    }
    (task_dir / "task.json").write_text(json.dumps(task_json, indent=2), encoding="utf-8")


def create_nano_task(
    name: str,
    category: str,
    objective: str,
    context_files: list[str],
    allowed_writes: list[str],
    verify_command: str,
    setup_files: dict[str, str],
    reference_files: dict[str, str],
    hidden_verify: list[str],
) -> None:
    task_dir = NANO_DIR / name
    if task_dir.is_dir():
        shutil.rmtree(task_dir)
    task_dir.mkdir(parents=True, exist_ok=True)
    setup_dir = task_dir / "setup"
    setup_dir.mkdir(parents=True, exist_ok=True)

    for fname, content in setup_files.items():
        fpath = setup_dir / fname
        fpath.parent.mkdir(parents=True, exist_ok=True)
        fpath.write_text(content, encoding="utf-8")

    init_git_repo(setup_dir)

    patch_parts = []
    for fname, after_content in reference_files.items():
        before_content = setup_files.get(fname, "")
        p = make_patch(fname, before_content, after_content)
        if p:
            patch_parts.append(p)
    patch_text = "\n".join(patch_parts)
    if not patch_text.endswith("\n"):
        patch_text += "\n"
    (task_dir / "reference.patch").write_text(patch_text, encoding="utf-8")

    contract = {
        "id": "T01",
        "title": f"Nano contract: {name}",
        "objective": objective,
        "category": category,
        "context_files": context_files,
        "allowed_writes": allowed_writes,
        "verify_command": verify_command,
        "estimated_changed_lines": 25,
    }

    nano_json = {
        "name": name,
        "category": category,
        "repo": "setup",
        "contract": contract,
        "hidden_verify": hidden_verify,
        "reference_patch": "reference.patch",
    }
    (task_dir / "nano.json").write_text(json.dumps(nano_json, indent=2), encoding="utf-8")
