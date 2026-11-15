"""Unit tests for evals/flow_report.py."""

from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

from evals.flow_report import (
    evaluate_decision_table,
    generate_markdown_report,
    append_flow_results,
)


class TestFlowReport(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="flow-rep-test-"))
        self.results_dir = self.tmp / "results"
        self.results_dir.mkdir()
        self.results_jsonl = self.tmp / "flow_results.jsonl"
        self.report_md = self.tmp / "FLOW_REPORT.md"

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_decision_table_h1_wins(self):
        # loses at most 2 tasks (e.g. only_d0=1), median saving >= 30% (e.g. 0.45)
        decision = evaluate_decision_table(only_d0=1, median_saving=0.45)
        self.assertIn("recommended mode", decision.lower())

    def test_decision_table_parity_marginal_savings(self):
        # loses <= 2 tasks, saving between 10% and 30% (e.g. 0.20)
        decision = evaluate_decision_table(only_d0=1, median_saving=0.20)
        self.assertIn("delegate only delegable categories", decision.lower())

    def test_decision_table_low_savings(self):
        # loses <= 2 tasks, saving under 10% (e.g. 0.05)
        decision = evaluate_decision_table(only_d0=0, median_saving=0.05)
        self.assertIn("do not recommend decomposition", decision.lower())

    def test_decision_table_h1_loses_tasks(self):
        # loses > 2 tasks (e.g. only_d0=3)
        decision = evaluate_decision_table(only_d0=3, median_saving=0.50)
        self.assertIn("read every lost task", decision.lower())

    def test_generate_markdown_report_sections(self):
        eval_data = {
            "pins": {
                "orchestrator_model": "test-orch",
                "worker_model": "test-worker",
                "prompt_version": "1.0",
                "git_commit": "abc1234",
                "date": "2026-11-16",
            },
            "paired_table": {"both_pass": 10, "both_fail": 2, "only_d0": 1, "only_h1": 2},
            "sign_test_p_val": 0.50,
            "conditions": {
                "D0": {"runs": 15, "pass_rate": 0.8, "tokens": 100000, "cost": 0.20},
                "H1": {"runs": 15, "pass_rate": 0.85, "tokens": 40000, "cost": 0.08},
            },
            "token_savings": {"median_saving": 0.45, "aggregate_saving": 0.50},
            "by_category": {
                "single_function": {"h1_pass_rate": 1.0, "d0_pass_rate": 0.8},
            },
            "by_language": {
                "python": {"h1_pass_rate": 0.9, "d0_pass_rate": 0.8},
            },
            "b9_failure_tally": {
                "contract_fault": 1,
                "worker_fault": 0,
                "verifier_fault": 0,
            },
            "decision": "Make flow the recommended mode for large tasks",
        }

        md = generate_markdown_report(eval_data)
        self.assertIn("# Flow Evaluation Report", md)
        self.assertIn("Configuration and Pins", md)
        self.assertIn("Paired Accuracy Table", md)
        self.assertIn("Token and Cost Comparison", md)
        self.assertIn("Category Breakdown", md)
        self.assertIn("Failure Analysis (B9)", md)
        self.assertIn("Decision Table Recommendation", md)

    def test_append_flow_results_jsonl(self):
        record = {
            "run_id": "r1",
            "condition": "H1",
            "task": "t1",
            "passed": True,
            "cloud_tokens": 1200,
            "cost_usd": 0.002,
        }
        append_flow_results(self.results_jsonl, [record])
        self.assertTrue(self.results_jsonl.exists())
        lines = self.results_jsonl.read_text().strip().splitlines()
        self.assertEqual(len(lines), 1)
        parsed = json.loads(lines[0])
        self.assertEqual(parsed["run_id"], "r1")


if __name__ == "__main__":
    unittest.main()

