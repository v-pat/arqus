"""Tests for nano eval runner."""

from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

from evals.flow_task import NanoTask
from evals.flow_run_nano import run_nano_eval
from evals.flow_profile import build_profile_stats


class TestFlowRunNano(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="test-nano-run-"))
        self.repo = self.tmp / "repo"
        self.repo.mkdir()
        (self.repo / "app.py").write_text("def run():\n    return False\n")
        (self.repo / "test_hid.py").write_text("import app\nassert app.run() is True\n")
        self.patch = (
            "--- a/app.py\n"
            "+++ b/app.py\n"
            "@@ -1,2 +1,2 @@\n"
            " def run():\n"
            "-    return False\n"
            "+    return True\n"
        )
        self.task = NanoTask(
            name="nano_test_1",
            category="single_function",
            repo=str(self.repo),
            repo_path=self.repo,
            contract={
                "id": "T01",
                "objective": "Make run() return True in app.py",
                "allowed_writes": ["app.py"],
                "verify_command": "python3 test_hid.py",
                "category": "single_function",
            },
            hidden_verify=["python3 test_hid.py"],
            reference_patch=self.patch,
        )

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_nano_run_dry_run(self):
        out_dir = self.tmp / "out"
        stats_file = self.tmp / "stats.jsonl"
        results = run_nano_eval(
            tasks=[self.task],
            worker_name="test-worker",
            stats_path=stats_file,
            out_dir=out_dir,
            dry_run=True,
        )
        self.assertEqual(len(results), 1)
        self.assertTrue(results[0]["passed"])
        self.assertEqual(results[0]["category"], "single_function")
        self.assertTrue(stats_file.is_file())
        lines = [line.strip() for line in stats_file.read_text(encoding="utf-8").splitlines() if line.strip()]
        self.assertEqual(len(lines), 1)
        stat = json.loads(lines[0])
        self.assertEqual(stat["category"], "single_function")
        self.assertTrue(stat["first_attempt_pass"])

    def test_nano_stats_recording_and_profile_compatibility(self):
        stats_file = self.tmp / "stats.jsonl"
        # Run 5 tasks to meet min_samples threshold in profile
        tasks = [
            NanoTask(
                name=f"nano_{i}",
                category="boilerplate",
                repo=str(self.repo),
                repo_path=self.repo,
                contract={
                    "id": f"T0{i}",
                    "objective": "Make run() return True",
                    "allowed_writes": ["app.py"],
                    "verify_command": "python3 test_hid.py",
                    "category": "boilerplate",
                },
                hidden_verify=["python3 test_hid.py"],
                reference_patch=self.patch,
            )
            for i in range(1, 6)
        ]
        run_nano_eval(
            tasks=tasks,
            worker_name="mock-worker",
            stats_path=stats_file,
            out_dir=self.tmp / "out",
            dry_run=True,
        )
        profile = build_profile_stats(stats_file)
        self.assertIn("boilerplate", profile.get("categories", {}))
        self.assertEqual(profile["categories"]["boilerplate"]["first_attempt_pass_rate"], 1.0)
        self.assertEqual(profile["categories"]["boilerplate"]["n"], 5)


if __name__ == "__main__":
    unittest.main()
