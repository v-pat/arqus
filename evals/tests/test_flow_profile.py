"""Unit tests for evals/flow_profile.py."""

from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

from evals.flow_profile import build_profile_stats, generate_profile_file


class TestFlowProfile(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="profile-test-"))
        self.stats_file = self.tmp / "stats.jsonl"
        self.out_file = self.tmp / "profile_stats.json"

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _write_stats(self, records: list[dict]) -> None:
        lines = [json.dumps(r) for r in records]
        self.stats_file.write_text("\n".join(lines) + "\n")

    def test_profile_stats_ignores_categories_with_n_under_5(self):
        records = [
            # 4 records for "single_function"
            {"model": "m1", "category": "single_function", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "single_function", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "single_function", "first_attempt_pass": False, "attempts": 2, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "single_function", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
        ]
        self._write_stats(records)
        stats = build_profile_stats(self.stats_file)
        # Category "single_function" has n=4 < 5, so categories should be empty
        self.assertNotIn("single_function", stats.get("categories", {}))

    def test_profile_stats_includes_categories_with_n_ge_5(self):
        records = [
            # 5 records for "boilerplate": 4 pass, 1 fail
            {"model": "m1", "category": "boilerplate", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "boilerplate", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "boilerplate", "first_attempt_pass": False, "attempts": 2, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "boilerplate", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "boilerplate", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
        ]
        self._write_stats(records)
        stats = build_profile_stats(self.stats_file)
        self.assertIn("boilerplate", stats.get("categories", {}))
        cat = stats["categories"]["boilerplate"]
        self.assertEqual(cat["n"], 5)
        self.assertAlmostEqual(cat["first_attempt_pass_rate"], 0.8)

    def test_profile_stats_calculates_exact_pass_rate(self):
        # 6 records for "local_bugfix": 3 pass, 3 fail
        records = [
            {"model": "m1", "category": "local_bugfix", "first_attempt_pass": (i % 2 == 0), "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100}
            for i in range(6)
        ]
        self._write_stats(records)
        stats = build_profile_stats(self.stats_file)
        cat = stats["categories"]["local_bugfix"]
        self.assertEqual(cat["n"], 6)
        self.assertAlmostEqual(cat["first_attempt_pass_rate"], 0.5)

    def test_profile_stats_filter_by_model(self):
        records = [
            # 5 records for model m1
            {"model": "m1", "category": "docs", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "docs", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "docs", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "docs", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m1", "category": "docs", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            # 5 records for model m2
            {"model": "m2", "category": "docs", "first_attempt_pass": False, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m2", "category": "docs", "first_attempt_pass": False, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m2", "category": "docs", "first_attempt_pass": False, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m2", "category": "docs", "first_attempt_pass": False, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
            {"model": "m2", "category": "docs", "first_attempt_pass": False, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100},
        ]
        self._write_stats(records)
        stats_m1 = build_profile_stats(self.stats_file, model_filter="m1")
        self.assertEqual(stats_m1["categories"]["docs"]["first_attempt_pass_rate"], 1.0)

        stats_m2 = build_profile_stats(self.stats_file, model_filter="m2")
        self.assertEqual(stats_m2["categories"]["docs"]["first_attempt_pass_rate"], 0.0)

    def test_profile_stats_output_json_schema(self):
        records = [
            {"model": "m1", "category": "tests", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "tokens": 100}
            for _ in range(5)
        ]
        self._write_stats(records)
        generate_profile_file(self.stats_file, self.out_file)
        self.assertTrue(self.out_file.exists())
        data = json.loads(self.out_file.read_text())
        self.assertIn("categories", data)
        self.assertIn("tests", data["categories"])
        self.assertIn("n", data["categories"]["tests"])
        self.assertIn("first_attempt_pass_rate", data["categories"]["tests"])


if __name__ == "__main__":
    unittest.main()

