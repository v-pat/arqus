"""Tests for the 30-task judging set (TESTS.md B3, B4)."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

from evals.flow_task import load_tasks_from_dir
from evals.flow_validity import validate_task


class TestJudgingSet(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.judging_dir = Path(__file__).resolve().parent.parent / "flow" / "judging"
        cls.holdout_file = cls.judging_dir / "holdout.json"
        cls.tasks = load_tasks_from_dir(cls.judging_dir)

    def test_judging_set_count_and_holdout(self):
        self.assertEqual(len(self.tasks), 30, f"Expected 30 tasks, found {len(self.tasks)}")
        self.assertTrue(self.holdout_file.is_file(), "Missing holdout.json")
        holdout_data = json.loads(self.holdout_file.read_text(encoding="utf-8"))
        self.assertIn("holdout", holdout_data)
        holdout_ids = holdout_data["holdout"]
        self.assertEqual(len(holdout_ids), 6, "Expected exactly 6 held-out tasks")
        task_names = {t.name for t in self.tasks}
        for hid in holdout_ids:
            self.assertIn(hid, task_names, f"Holdout task {hid} not found in judging tasks")

    def test_judging_set_size_distribution(self):
        sizes = [t.size for t in self.tasks]
        s_count = sizes.count("S")
        m_count = sizes.count("M")
        l_count = sizes.count("L")
        self.assertEqual(s_count, 10, f"Expected 10 S tasks, got {s_count}")
        self.assertEqual(m_count, 12, f"Expected 12 M tasks, got {m_count}")
        self.assertEqual(l_count, 8, f"Expected 8 L tasks, got {l_count}")

    def test_judging_set_language_distribution(self):
        langs = [t.language for t in self.tasks]
        for expected_lang in ["rust", "python", "go", "typescript"]:
            cnt = langs.count(expected_lang)
            self.assertGreaterEqual(cnt, 6, f"Expected >= 6 {expected_lang} tasks, got {cnt}")

    def test_judging_set_categories_represented(self):
        cats = {t.category for t in self.tasks}
        expected_cats = {
            "boilerplate",
            "single_function",
            "tests",
            "docs",
            "local_refactor",
            "local_bugfix",
            "multi_file",
        }
        for ec in expected_cats:
            self.assertIn(ec, cats, f"Category {ec} missing from judging set")

    def test_judging_set_all_tasks_pass_validity(self):
        self.assertGreaterEqual(len(self.tasks), 1, "No judging tasks loaded")
        for task in self.tasks:
            with self.subTest(task=task.name):
                validate_task(task)


if __name__ == "__main__":
    unittest.main()

