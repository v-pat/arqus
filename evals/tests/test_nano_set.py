"""Tests for the 60-contract nano set (TESTS.md B4)."""

from __future__ import annotations

import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

from evals.flow_task import load_nano_task, NanoTask
from evals.flow_validity import validate_nano_task


def load_all_nano_tasks(nano_dir: Path) -> list[NanoTask]:
    if not nano_dir.is_dir():
        return []
    tasks = []
    for item in sorted(nano_dir.iterdir()):
        if item.is_dir() and ((item / "nano.json").is_file() or (item / "task.json").is_file()):
            tasks.append(load_nano_task(item))
    return tasks


class TestNanoSet(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.nano_dir = Path(__file__).resolve().parent.parent / "flow" / "nano"
        cls.tasks = load_all_nano_tasks(cls.nano_dir)

    def test_nano_set_count(self):
        self.assertEqual(len(self.tasks), 60, f"Expected 60 nano tasks, found {len(self.tasks)}")

    def test_nano_set_category_distribution(self):
        cats = [t.category for t in self.tasks]
        delegable_cats = [
            "boilerplate",
            "single_function",
            "tests",
            "docs",
            "local_refactor",
            "local_bugfix",
            "multi_file",
        ]
        for dc in delegable_cats:
            cnt = cats.count(dc)
            self.assertGreaterEqual(cnt, 8, f"Expected >= 8 tasks for {dc}, got {cnt}")

    def test_nano_set_contract_schemas(self):
        for task in self.tasks:
            c = task.contract
            self.assertIn("id", c)
            self.assertTrue(c["id"].startswith("T"), f"Task ID must start with T: {c['id']}")
            self.assertIn("objective", c)
            self.assertIn("allowed_writes", c)
            self.assertIsInstance(c["allowed_writes"], list)
            self.assertGreater(len(c["allowed_writes"]), 0)
            self.assertIn("verify_command", c)
            self.assertGreater(len(task.hidden_verify), 0)

    def test_nano_set_all_contracts_pass_validity(self):
        self.assertGreaterEqual(len(self.tasks), 1, "No nano tasks loaded")
        for task in self.tasks:
            with self.subTest(task=task.name):
                validate_nano_task(task)


if __name__ == "__main__":
    unittest.main()

