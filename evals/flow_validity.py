"""Task validity checker for flow evaluation tasks.

Enforces rules from document 6 (TESTS.md B3).
Python 3 standard library only.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from evals.flow_task import TaskDefinition, NanoTask


class TaskValidationError(Exception):
    """Raised when a task fails schema validation or validity checks."""
    pass


def _run_cmd(cmd: str, cwd: Path) -> int:
    try:
        res = subprocess.run(
            ["sh", "-c", cmd],
            cwd=str(cwd),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=60,
        )
        return res.returncode
    except (subprocess.TimeoutExpired, OSError):
        return 1


def _is_git_clean(repo_path: Path) -> bool:
    if not (repo_path / ".git").is_dir():
        # Fixture directory without embedded .git: check if modified relative to HEAD if tracked
        try:
            res = subprocess.run(
                ["git", "diff", "HEAD", "--", "."],
                cwd=str(repo_path),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            return len(res.stdout.strip()) == 0
        except (subprocess.CalledProcessError, OSError):
            return True

    try:
        res = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=str(repo_path),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=True,
        )
        return len(res.stdout.strip()) == 0
    except (subprocess.CalledProcessError, OSError):
        return False


def validate_task(task: TaskDefinition) -> None:
    """Validate a task against the 4 rules from TESTS.md B3."""
    if not task.repo_path.exists():
        raise TaskValidationError(f"Repo path does not exist: {task.repo_path}")

    # Check 1: Repo must be clean
    if not _is_git_clean(task.repo_path):
        raise TaskValidationError(
            f"Task '{task.name}': fixture repository {task.repo_path} is not clean (uncommitted changes or untracked files)"
        )

    # Work in a throwaway copy
    tmp_root = Path(tempfile.mkdtemp(prefix=f"task-val-{task.name}-"))
    work_dir = tmp_root / "work"
    try:
        shutil.copytree(task.repo_path, work_dir, symlinks=True)
        if not (work_dir / ".git").is_dir():
            subprocess.run(["git", "init", "-q"], cwd=str(work_dir), check=True)
            subprocess.run(["git", "config", "user.email", "eval@arqus.local"], cwd=str(work_dir), check=True)
            subprocess.run(["git", "config", "user.name", "eval"], cwd=str(work_dir), check=True)
            subprocess.run(["git", "add", "-A"], cwd=str(work_dir), check=True)
            subprocess.run(["git", "commit", "-qm", "init"], cwd=str(work_dir), check=True)

        # Check 4: public_verify must fail before reference patch
        if task.public_verify:
            all_passed = True
            for cmd in task.public_verify:
                if _run_cmd(cmd, work_dir) != 0:
                    all_passed = False
                    break
            if all_passed:
                raise TaskValidationError(
                    f"Task '{task.name}': public_verify must fail before reference patch"
                )

        # Check 3a: regression_verify must pass before reference patch
        for cmd in task.regression_verify:
            if _run_cmd(cmd, work_dir) != 0:
                raise TaskValidationError(
                    f"Task '{task.name}': regression_verify must pass before reference patch ({cmd})"
                )

        # Copy hidden test files in if present
        if task.hidden_dir and task.hidden_dir.is_dir():
            for item in task.hidden_dir.iterdir():
                target = work_dir / item.name
                if item.is_dir():
                    shutil.copytree(item, target, dirs_exist_ok=True)
                else:
                    shutil.copy2(item, target)

        # Check 2a: hidden_verify must fail before reference patch
        if task.hidden_verify:
            all_passed = True
            for cmd in task.hidden_verify:
                if _run_cmd(cmd, work_dir) != 0:
                    all_passed = False
                    break
            if all_passed:
                raise TaskValidationError(
                    f"Task '{task.name}': hidden_verify must fail before reference patch"
                )

        # Apply reference patch
        patch_file = tmp_root / "ref.patch"
        patch_file.write_text(task.reference_patch, encoding="utf-8")
        try:
            apply_res = subprocess.run(
                ["git", "apply", str(patch_file)],
                cwd=str(work_dir),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                check=True,
            )
        except (subprocess.CalledProcessError, OSError) as e:
            err = getattr(e, "stderr", str(e))
            raise TaskValidationError(
                f"Task '{task.name}': failed to apply reference_patch: {err}"
            )

        # Check 2b: hidden_verify must pass after reference patch
        for cmd in task.hidden_verify:
            if _run_cmd(cmd, work_dir) != 0:
                raise TaskValidationError(
                    f"Task '{task.name}': hidden_verify must pass after reference patch ({cmd})"
                )

        # Check 3b: regression_verify must pass after reference patch
        for cmd in task.regression_verify:
            if _run_cmd(cmd, work_dir) != 0:
                raise TaskValidationError(
                    f"Task '{task.name}': regression_verify must pass after reference patch ({cmd})"
                )

    finally:
        shutil.rmtree(tmp_root, ignore_errors=True)


def validate_nano_task(task: NanoTask) -> None:
    """Validate a nano contract task (TESTS.md B4)."""
    if not task.repo_path.exists():
        raise TaskValidationError(f"Repo path does not exist: {task.repo_path}")

    if not _is_git_clean(task.repo_path):
        raise TaskValidationError(
            f"Nano task '{task.name}': fixture repository {task.repo_path} is not clean"
        )

    contract = task.contract
    if not isinstance(contract, dict):
        raise TaskValidationError(f"Nano task '{task.name}': contract must be a dict")
    if "id" not in contract or not str(contract["id"]).startswith("T"):
        raise TaskValidationError(f"Nano task '{task.name}': contract must have valid ID starting with T")
    if not contract.get("objective"):
        raise TaskValidationError(f"Nano task '{task.name}': contract must have an objective")
    if not contract.get("allowed_writes") or not isinstance(contract["allowed_writes"], list):
        raise TaskValidationError(f"Nano task '{task.name}': contract allowed_writes must be non-empty list")
    if not contract.get("verify_command"):
        raise TaskValidationError(f"Nano task '{task.name}': contract must have a verify_command")
    if not task.hidden_verify:
        raise TaskValidationError(f"Nano task '{task.name}': hidden_verify must not be empty")

    tmp_root = Path(tempfile.mkdtemp(prefix=f"nano-val-{task.name}-"))
    work_dir = tmp_root / "work"
    try:
        shutil.copytree(task.repo_path, work_dir, symlinks=True)
        if not (work_dir / ".git").is_dir():
            subprocess.run(["git", "init", "-q"], cwd=str(work_dir), check=True)
            subprocess.run(["git", "config", "user.email", "eval@arqus.local"], cwd=str(work_dir), check=True)
            subprocess.run(["git", "config", "user.name", "eval"], cwd=str(work_dir), check=True)
            subprocess.run(["git", "add", "-A"], cwd=str(work_dir), check=True)
            subprocess.run(["git", "commit", "-qm", "init"], cwd=str(work_dir), check=True)

        # hidden_verify must fail before reference patch
        all_passed = True
        for cmd in task.hidden_verify:
            if _run_cmd(cmd, work_dir) != 0:
                all_passed = False
                break
        if all_passed:
            raise TaskValidationError(
                f"Nano task '{task.name}': hidden_verify must fail before reference patch"
            )

        # Apply reference patch
        patch_file = tmp_root / "ref.patch"
        patch_file.write_text(task.reference_patch, encoding="utf-8")
        try:
            subprocess.run(
                ["git", "apply", str(patch_file)],
                cwd=str(work_dir),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                check=True,
            )
        except (subprocess.CalledProcessError, OSError) as e:
            err = getattr(e, "stderr", str(e))
            raise TaskValidationError(
                f"Nano task '{task.name}': failed to apply reference_patch: {err}"
            )

        # hidden_verify must pass after reference patch
        for cmd in task.hidden_verify:
            if _run_cmd(cmd, work_dir) != 0:
                raise TaskValidationError(
                    f"Nano task '{task.name}': hidden_verify must pass after reference patch ({cmd})"
                )
    finally:
        shutil.rmtree(tmp_root, ignore_errors=True)


