"""Unit tests for evals/flow_validity.py."""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

from evals.flow_task import TaskDefinition
from evals.flow_validity import validate_task, TaskValidationError


def init_git_repo(path: Path) -> None:
    subprocess.run(["git", "init", "-q"], cwd=str(path), check=True)
    subprocess.run(["git", "config", "user.email", "eval@arqus.local"], cwd=str(path), check=True)
    subprocess.run(["git", "config", "user.name", "eval"], cwd=str(path), check=True)
    subprocess.run(["git", "add", "-A"], cwd=str(path), check=True)
    subprocess.run(["git", "commit", "-qm", "init"], cwd=str(path), check=True)


class TestTaskValidity(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="flow-val-test-"))
        self.repo = self.tmp / "repo"
        self.repo.mkdir()
        # Setup files: a module and tests
        (self.repo / "calc.py").write_text("def add(a, b):\n    return a - b  # bug\n")
        (self.repo / "test_reg.py").write_text("assert True\n")
        (self.repo / "test_pub.py").write_text("import calc\nassert calc.add(2, 2) == 4\n")
        (self.repo / "test_hid.py").write_text("import calc\nassert calc.add(3, 5) == 8\n")
        init_git_repo(self.repo)

        # Good patch that fixes calc.py
        self.patch = (
            "--- a/calc.py\n"
            "+++ b/calc.py\n"
            "@@ -1,2 +1,2 @@\n"
            " def add(a, b):\n"
            "-    return a - b  # bug\n"
            "+    return a + b\n"
        )

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _make_task(self, **kwargs) -> TaskDefinition:
        base = {
            "name": "test-task",
            "language": "python",
            "size": "S",
            "category": "single_function",
            "repo": str(self.repo),
            "objective": "Fix add() in calc.py",
            "public_verify": ["python3 test_pub.py"],
            "hidden_verify": ["python3 test_hid.py"],
            "regression_verify": ["python3 test_reg.py"],
            "protected": ["test_*.py"],
            "reference_patch": self.patch,
        }
        base.update(kwargs)
        return TaskDefinition.from_dict(base)

    def test_valid_task_passes_validity_check(self):
        task = self._make_task()
        # Should not raise TaskValidationError
        validate_task(task)

    def test_reject_dirty_repo(self):
        # Add an untracked dirty file
        (self.repo / "dirty.txt").write_text("uncommitted")
        task = self._make_task()
        with self.assertRaises(TaskValidationError) as ctx:
            validate_task(task)
        self.assertIn("clean", str(ctx.exception).lower())

    def test_reject_hidden_verify_passes_before_patch(self):
        # hidden_verify already passes before the patch
        task = self._make_task(hidden_verify=["python3 -c 'exit(0)'"])
        with self.assertRaises(TaskValidationError) as ctx:
            validate_task(task)
        self.assertIn("hidden_verify must fail before", str(ctx.exception).lower())

    def test_reject_hidden_verify_fails_after_patch(self):
        # hidden_verify still fails after the patch
        task = self._make_task(hidden_verify=["python3 -c 'exit(1)'"])
        with self.assertRaises(TaskValidationError) as ctx:
            validate_task(task)
        self.assertIn("hidden_verify must pass after", str(ctx.exception).lower())

    def test_reject_regression_verify_fails_before_patch(self):
        # regression_verify fails initially
        task = self._make_task(regression_verify=["python3 -c 'exit(1)'"])
        with self.assertRaises(TaskValidationError) as ctx:
            validate_task(task)
        self.assertIn("regression_verify must pass before", str(ctx.exception).lower())

    def test_reject_regression_verify_fails_after_patch(self):
        # regression_verify passes before, but fails when patch modifies calc.py
        # We write a regression test that checks if calc.py has '-'
        (self.repo / "test_reg.py").write_text("with open('calc.py') as f:\n    assert '-' in f.read()\n")
        subprocess.run(["git", "commit", "-am", "update reg"], cwd=str(self.repo), check=True)
        task = self._make_task()
        with self.assertRaises(TaskValidationError) as ctx:
            validate_task(task)
        self.assertIn("regression_verify must pass after", str(ctx.exception).lower())

    def test_reject_public_verify_passes_before_patch(self):
        # public_verify already passes before the patch
        task = self._make_task(public_verify=["python3 -c 'exit(0)'"])
        with self.assertRaises(TaskValidationError) as ctx:
            validate_task(task)
        self.assertIn("public_verify must fail before", str(ctx.exception).lower())

    def test_reject_missing_required_fields(self):
        raw = {
            "name": "bad-task",
            "language": "python",
            # missing size, category, etc.
        }
        with self.assertRaises(TaskValidationError) as ctx:
            TaskDefinition.from_dict(raw)
        self.assertIn("missing", str(ctx.exception).lower())

    def test_reject_invalid_category_or_size(self):
        with self.assertRaises(TaskValidationError) as ctx:
            self._make_task(category="unknown_category")
        self.assertIn("category", str(ctx.exception).lower())

        with self.assertRaises(TaskValidationError) as ctx:
            self._make_task(size="XL")
        self.assertIn("size", str(ctx.exception).lower())

        with self.assertRaises(TaskValidationError) as ctx:
            self._make_task(language="ruby")
        self.assertIn("language", str(ctx.exception).lower())

    def test_reject_invalid_reference_patch(self):
        # Malformed patch that cannot apply
        task = self._make_task(reference_patch="not a valid diff header\n")
        with self.assertRaises(TaskValidationError) as ctx:
            validate_task(task)
        self.assertIn("patch", str(ctx.exception).lower())


if __name__ == "__main__":
    unittest.main()

