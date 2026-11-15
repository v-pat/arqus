"""Unit tests for evals/flow_metrics.py."""

from __future__ import annotations

import unittest
import math

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

from evals.flow_metrics import (
    compute_accuracy_metrics,
    compute_cloud_token_metrics,
    compute_cloud_dollars_per_solved,
    compute_token_savings,
    compute_delegation_metrics,
    compute_overhead_metrics,
    compute_contract_quality_metrics,
    compute_stability_metrics,
    compute_paired_table,
    exact_sign_test,
)


class TestFlowMetrics(unittest.TestCase):
    def test_accuracy_metrics_all_pass(self):
        runs = [
            {"task": "t1", "passed": True, "scope_violations": 0, "regression": False},
            {"task": "t1", "passed": True, "scope_violations": 0, "regression": False},
            {"task": "t2", "passed": True, "scope_violations": 0, "regression": False},
        ]
        m = compute_accuracy_metrics(runs)
        self.assertEqual(m["total_runs"], 3)
        self.assertEqual(m["passed_runs"], 3)
        self.assertAlmostEqual(m["run_pass_rate"], 1.0)
        self.assertEqual(m["tasks_evaluated"], 2)
        self.assertEqual(m["tasks_passed_majority"], 2)
        self.assertAlmostEqual(m["task_pass_rate"], 1.0)
        self.assertEqual(m["scope_violations"], 0)
        self.assertEqual(m["regressions"], 0)

    def test_accuracy_metrics_majority_and_regressions(self):
        runs = [
            {"task": "t1", "passed": True, "scope_violations": 0, "regression": False},
            {"task": "t1", "passed": False, "scope_violations": 0, "regression": True},
            {"task": "t1", "passed": True, "scope_violations": 0, "regression": False},
            {"task": "t2", "passed": False, "scope_violations": 1, "regression": False},
            {"task": "t2", "passed": False, "scope_violations": 0, "regression": True},
            {"task": "t2", "passed": True, "scope_violations": 0, "regression": False},
        ]
        m = compute_accuracy_metrics(runs)
        self.assertEqual(m["total_runs"], 6)
        self.assertEqual(m["passed_runs"], 3)
        self.assertAlmostEqual(m["run_pass_rate"], 0.5)
        # t1 passed 2/3 -> majority True. t2 passed 1/3 -> majority False
        self.assertEqual(m["tasks_passed_majority"], 1)
        self.assertAlmostEqual(m["task_pass_rate"], 0.5)
        self.assertEqual(m["scope_violations"], 1)
        self.assertEqual(m["regressions"], 2)

    def test_accuracy_scope_violations(self):
        runs = [
            {"task": "t1", "passed": False, "scope_violations": 2, "regression": False},
        ]
        m = compute_accuracy_metrics(runs)
        self.assertEqual(m["scope_violations"], 2)

    def test_cloud_tokens_raw_and_cost_weighted(self):
        records = [
            {
                "input": 1000,
                "output": 200,
                "cache_read": 500,
                "cache_write": 100,
                "cost_usd": 0.005,
            },
            {
                "input": 2000,
                "output": 300,
                "cache_read": 1000,
                "cache_write": 0,
                "cost_usd": 0.008,
            },
        ]
        tokens = compute_cloud_token_metrics(records)
        # Raw total = 1000+200+500+100 + 2000+300+1000+0 = 5100
        self.assertEqual(tokens["raw_total"], 5100)
        self.assertEqual(tokens["input"], 3000)
        self.assertEqual(tokens["output"], 500)
        self.assertEqual(tokens["cache_read"], 1500)
        self.assertEqual(tokens["cache_write"], 100)
        self.assertAlmostEqual(tokens["cost_usd"], 0.013)

    def test_cloud_dollars_per_solved_task(self):
        cost = 0.50
        solved_tasks = 5
        self.assertAlmostEqual(
            compute_cloud_dollars_per_solved(cost, solved_tasks), 0.10
        )
        self.assertIsNone(compute_cloud_dollars_per_solved(cost, 0))

    def test_token_saving_paired_median_and_aggregate(self):
        # task a: D0=1000, H1=400 -> saving = 1 - 400/1000 = 0.60
        # task b: D0=2000, H1=1000 -> saving = 1 - 1000/2000 = 0.50
        # task c: D0=500,  H1=400 -> saving = 1 - 400/500 = 0.20
        d0_tasks = {"a": 1000, "b": 2000, "c": 500}
        h1_tasks = {"a": 400, "b": 1000, "c": 400}
        savings = compute_token_savings(d0_tasks, h1_tasks)
        # sorted savings: [0.20, 0.50, 0.60] -> median is 0.50
        self.assertAlmostEqual(savings["median_saving"], 0.50)
        # aggregate: 1 - (400+1000+400)/(1000+2000+500) = 1 - 1800/3500 = 1700/3500 = 0.485714...
        self.assertAlmostEqual(savings["aggregate_saving"], 1700 / 3500)

    def test_token_saving_unpaired_or_zero_baseline(self):
        d0_tasks = {"a": 0, "b": 1000}
        h1_tasks = {"a": 100, "b": 500}
        savings = compute_token_savings(d0_tasks, h1_tasks)
        # task a should be skipped because d0 is 0
        self.assertEqual(len(savings["per_task"]), 1)
        self.assertAlmostEqual(savings["median_saving"], 0.50)

    def test_delegation_metrics(self):
        tasks = [
            {"executor": "worker", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "split": False},
            {"executor": "worker", "first_attempt_pass": False, "attempts": 2, "redesigns": 1, "takeover": False, "split": False},
            {"executor": "orchestrator", "first_attempt_pass": False, "attempts": 3, "redesigns": 1, "takeover": True, "split": True},
            {"executor": "worker", "first_attempt_pass": True, "attempts": 1, "redesigns": 0, "takeover": False, "split": False},
        ]
        m = compute_delegation_metrics(tasks)
        self.assertEqual(m["total_tasks"], 4)
        self.assertAlmostEqual(m["worker_finished_share"], 3 / 4)
        # worker attempted tasks = 4 (since all had worker or takeover after worker)
        self.assertAlmostEqual(m["worker_first_attempt_pass_rate"], 2 / 4)
        self.assertAlmostEqual(m["redesign_rate"], 2 / 4)
        self.assertAlmostEqual(m["takeover_rate"], 1 / 4)
        self.assertAlmostEqual(m["split_rate"], 1 / 4)

    def test_overhead_metrics(self):
        # cloud calls with role and tokens, plus failed tasks
        calls = [
            {"role": "planner", "tokens": 1000},
            {"role": "redesign", "tokens": 500},
            {"role": "takeover", "tokens": 500},
            {"role": "worker", "tokens": 4000}, # local, excluded from cloud
        ]
        failed_tasks_tokens = 300
        m = compute_overhead_metrics(calls, failed_tasks_tokens)
        # cloud tokens = 1000 + 500 + 500 = 2000
        self.assertAlmostEqual(m["planner_share"], 1000 / 2000)
        self.assertAlmostEqual(m["redesign_takeover_share"], 1000 / 2000)
        self.assertEqual(m["failed_tasks_tokens"], 300)

    def test_contract_quality_metrics(self):
        plans = [
            {"accepted_without_repair": True, "repair_rounds": 0, "rejected_contracts": 0},
            {"accepted_without_repair": False, "repair_rounds": 2, "rejected_contracts": 1},
        ]
        worker_contracts = [
            {"first_attempt_failed": False},
            {"first_attempt_failed": True},
            {"first_attempt_failed": False},
        ]
        m = compute_contract_quality_metrics(plans, worker_contracts)
        self.assertAlmostEqual(m["plans_accepted_without_repair_rate"], 0.5)
        self.assertAlmostEqual(m["avg_repair_rounds_per_plan"], 1.0)
        self.assertEqual(m["validator_rejected_contracts"], 1)
        self.assertAlmostEqual(m["worker_first_attempt_failure_rate"], 1 / 3)

    def test_stability_metrics_and_unstable_flagging(self):
        # 3 repeats
        task_runs = {
            "t1": [True, True, True],     # stable pass (3/3)
            "t2": [False, False, False], # stable fail (0/3)
            "t3": [True, False, True],   # unstable (2/3)
        }
        m = compute_stability_metrics(task_runs, repeats=3)
        self.assertEqual(m["counts"]["t1"], 3)
        self.assertEqual(m["counts"]["t2"], 0)
        self.assertEqual(m["counts"]["t3"], 2)
        self.assertIn("t3", m["unstable_tasks"])
        self.assertNotIn("t1", m["unstable_tasks"])
        self.assertNotIn("t2", m["unstable_tasks"])

    def test_paired_table_counts(self):
        # D0 and H1 results per task (majority pass)
        d0_pass = {"t1": True, "t2": False, "t3": True, "t4": False}
        h1_pass = {"t1": True, "t2": True, "t3": False, "t4": False}
        table = compute_paired_table(d0_pass, h1_pass)
        self.assertEqual(table["both_pass"], 1) # t1
        self.assertEqual(table["both_fail"], 1) # t4
        self.assertEqual(table["only_d0"], 1)   # t3
        self.assertEqual(table["only_h1"], 1)   # t2

    def test_exact_sign_test_hand_computed_values(self):
        # n10 = 1, n01 = 5, total N = 6
        # P(k <= 1) = (1 + 6)/64 = 7/64. Two-tailed = 14/64 = 7/32 = 0.21875
        p_val = exact_sign_test(only_d0=1, only_h1=5)
        self.assertAlmostEqual(p_val, 0.21875)

        # n10 = 0, n01 = 8, N = 8 -> 2 * (1/256) = 1/128 = 0.0078125
        p_val_2 = exact_sign_test(only_d0=0, only_h1=8)
        self.assertAlmostEqual(p_val_2, 0.0078125)

    def test_exact_sign_test_concordant_and_zero_discordant(self):
        # No discordant tasks -> p-value 1.0
        p_val = exact_sign_test(only_d0=0, only_h1=0)
        self.assertAlmostEqual(p_val, 1.0)
        # Even split -> p-value 1.0
        p_val_even = exact_sign_test(only_d0=4, only_h1=4)
        self.assertAlmostEqual(p_val_even, 1.0)


if __name__ == "__main__":
    unittest.main()

