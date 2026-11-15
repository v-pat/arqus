"""End-to-end tests for runner and smoke fixtures."""

from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

from evals.flow_task import load_tasks_from_dir
from evals.flow_validity import validate_task
from evals.flow_run import build_run_schedule, run_eval_matrix


class TestFlowRunSmoke(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="flow-run-test-"))
        self.tasks_dir = Path(__file__).resolve().parent.parent / "flow" / "tasks"

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_smoke_fixtures_pass_validity_checks(self):
        # All 6 smoke fixtures under evals/flow/tasks must be valid tasks
        tasks = load_tasks_from_dir(self.tasks_dir)
        self.assertEqual(len(tasks), 6, f"Expected 6 smoke fixtures, found {len(tasks)}")
        for task in tasks:
            # Must pass validity check without raising TaskValidationError
            validate_task(task)

    def test_flow_run_schedule_and_interleaving(self):
        task_names = ["taskA", "taskB"]
        conditions = ["D0", "H1"]
        repeats = 2
        schedule = build_run_schedule(task_names, conditions, repeats)
        # Total runs = 2 * 2 * 2 = 8
        self.assertEqual(len(schedule), 8)
        # Conditions should alternate/interleave per repeat:
        # e.g. (taskA, rep1, D0), (taskA, rep1, H1), (taskB, rep1, D0), (taskB, rep1, H1) ...
        first_four = schedule[:4]
        self.assertEqual([item["task"] for item in first_four], ["taskA", "taskA", "taskB", "taskB"])
        self.assertEqual([item["condition"] for item in first_four], ["D0", "H1", "D0", "H1"])

    def test_flow_run_smoke_fixtures_end_to_end(self):
        # Test executing runner with a mock or simulated flow run on 1 smoke fixture
        tasks = load_tasks_from_dir(self.tasks_dir)
        self.assertTrue(len(tasks) > 0)
        smoke_task = tasks[0]
        out_dir = self.tmp / "output"
        out_dir.mkdir()

        # Run mock matrix
        results = run_eval_matrix(
            tasks=[smoke_task],
            conditions=["H1"],
            repeats=1,
            output_dir=out_dir,
            dry_run=True, # Dry run simulates without live model
        )
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0]["task"], smoke_task.name)
        self.assertEqual(results[0]["condition"], "H1")


if __name__ == "__main__":
    unittest.main()

