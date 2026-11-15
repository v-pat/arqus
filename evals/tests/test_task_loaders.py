"""Unit tests for task loaders in evals/flow_task.py."""

from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

from evals.flow_task import (
    load_e2e_task,
    load_planner_task,
    load_nano_task,
    load_tasks_from_dir,
    TaskDefinition,
    PlannerTask,
    NanoTask,
)


class TestTaskLoaders(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="task-load-test-"))
        self.task_dir = self.tmp / "sample_task"
        self.task_dir.mkdir()
        (self.task_dir / "setup").mkdir()
        (self.task_dir / "setup" / "hello.py").write_text("print('hi')\n")
        (self.task_dir / "patch.diff").write_text("diff --git a/hello.py b/hello.py\n")

        self.e2e_data = {
            "name": "sample_task",
            "language": "python",
            "size": "S",
            "category": "single_function",
            "repo": "setup",
            "objective": "Say hello world",
            "public_verify": ["python3 -m unittest test_pub.py"],
            "hidden_verify": ["python3 -m unittest test_hid.py"],
            "regression_verify": ["python3 -m unittest test_reg.py"],
            "protected": ["tests/"],
            "reference_patch": "patch.diff",
        }
        (self.task_dir / "task.json").write_text(json.dumps(self.e2e_data, indent=2))

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_load_e2e_task(self):
        task = load_e2e_task(self.task_dir)
        self.assertIsInstance(task, TaskDefinition)
        self.assertEqual(task.name, "sample_task")
        self.assertEqual(task.language, "python")
        self.assertEqual(task.size, "S")
        self.assertEqual(task.category, "single_function")
        self.assertEqual(task.objective, "Say hello world")
        self.assertTrue(task.repo_path.is_dir())
        self.assertIn("diff", task.reference_patch)

    def test_load_planner_task(self):
        planner = load_planner_task(self.task_dir)
        self.assertIsInstance(planner, PlannerTask)
        self.assertEqual(planner.name, "sample_task")
        self.assertEqual(planner.objective, "Say hello world")
        self.assertTrue(planner.repo_path.is_dir())
        self.assertEqual(planner.public_verify, ["python3 -m unittest test_pub.py"])

    def test_load_nano_task(self):
        nano_dir = self.tmp / "nano_task"
        nano_dir.mkdir()
        (nano_dir / "setup").mkdir()
        nano_data = {
            "name": "nano_task",
            "category": "local_bugfix",
            "repo": "setup",
            "contract": {
                "id": "T01",
                "title": "Fix bug",
                "objective": "Fix return value in calc.py",
                "context_files": ["calc.py"],
                "allowed_writes": ["calc.py"],
                "verify_command": "python3 test.py",
                "category": "local_bugfix",
            },
            "hidden_verify": ["python3 test.py"],
        }
        (nano_dir / "nano.json").write_text(json.dumps(nano_data, indent=2))
        nano = load_nano_task(nano_dir)
        self.assertIsInstance(nano, NanoTask)
        self.assertEqual(nano.name, "nano_task")
        self.assertEqual(nano.category, "local_bugfix")
        self.assertEqual(nano.contract["id"], "T01")

    def test_load_tasks_from_directory(self):
        tasks = load_tasks_from_dir(self.tmp)
        self.assertEqual(len(tasks), 1)
        self.assertEqual(tasks[0].name, "sample_task")


if __name__ == "__main__":
    unittest.main()

