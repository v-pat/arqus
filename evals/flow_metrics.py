"""Evaluation metrics and statistics for arqus flow.

Enforces rules from document 6 (TESTS.md B6).
Python 3 standard library only.
"""

from __future__ import annotations

import math
from typing import Any


def compute_accuracy_metrics(runs: list[dict[str, Any]]) -> dict[str, Any]:
    total_runs = len(runs)
    if total_runs == 0:
        return {
            "total_runs": 0,
            "passed_runs": 0,
            "run_pass_rate": 0.0,
            "tasks_evaluated": 0,
            "tasks_passed_majority": 0,
            "task_pass_rate": 0.0,
            "scope_violations": 0,
            "regressions": 0,
        }

    passed_runs = sum(1 for r in runs if r.get("passed", False))
    scope_violations = sum(r.get("scope_violations", 0) for r in runs)
    regressions = sum(1 for r in runs if r.get("regression", False))

    by_task: dict[str, list[bool]] = {}
    for r in runs:
        task = r.get("task", "")
        by_task.setdefault(task, []).append(bool(r.get("passed", False)))

    tasks_passed_majority = 0
    for task_passes in by_task.values():
        if sum(task_passes) > len(task_passes) / 2:
            tasks_passed_majority += 1

    tasks_evaluated = len(by_task)
    return {
        "total_runs": total_runs,
        "passed_runs": passed_runs,
        "run_pass_rate": passed_runs / total_runs,
        "tasks_evaluated": tasks_evaluated,
        "tasks_passed_majority": tasks_passed_majority,
        "task_pass_rate": tasks_passed_majority / tasks_evaluated if tasks_evaluated else 0.0,
        "scope_violations": scope_violations,
        "regressions": regressions,
    }


def compute_cloud_token_metrics(
    records: list[dict[str, Any]], price_config: dict[str, float] | None = None
) -> dict[str, Any]:
    input_tokens = sum(r.get("input", 0) for r in records)
    output_tokens = sum(r.get("output", 0) for r in records)
    cache_read = sum(r.get("cache_read", 0) for r in records)
    cache_write = sum(r.get("cache_write", 0) for r in records)
    raw_total = input_tokens + output_tokens + cache_read + cache_write

    total_cost = 0.0
    for r in records:
        if r.get("cost_usd") is not None:
            total_cost += float(r["cost_usd"])
        elif price_config:
            in_cost = (r.get("input", 0) * price_config.get("input", 0.0)) / 1_000_000
            out_cost = (r.get("output", 0) * price_config.get("output", 0.0)) / 1_000_000
            cached_cost = (
                r.get("cache_read", 0)
                * price_config.get("cached_input", price_config.get("input", 0.0))
            ) / 1_000_000
            total_cost += in_cost + out_cost + cached_cost

    return {
        "raw_total": raw_total,
        "input": input_tokens,
        "output": output_tokens,
        "cache_read": cache_read,
        "cache_write": cache_write,
        "cost_usd": total_cost,
    }


def compute_cloud_dollars_per_solved(
    total_cost: float, solved_tasks: int
) -> float | None:
    if solved_tasks <= 0:
        return None
    return total_cost / solved_tasks


def compute_token_savings(
    d0_tasks: dict[str, int], h1_tasks: dict[str, int]
) -> dict[str, Any]:
    per_task: dict[str, float] = {}
    sum_d0 = 0
    sum_h1 = 0

    for task, d0_tok in d0_tasks.items():
        if task in h1_tasks and d0_tok > 0:
            h1_tok = h1_tasks[task]
            saving = 1.0 - (h1_tok / d0_tok)
            per_task[task] = saving
            sum_d0 += d0_tok
            sum_h1 += h1_tok

    if not per_task:
        return {
            "per_task": {},
            "median_saving": 0.0,
            "aggregate_saving": 0.0,
        }

    sorted_savings = sorted(per_task.values())
    mid = len(sorted_savings) // 2
    if len(sorted_savings) % 2 == 1:
        median_saving = sorted_savings[mid]
    else:
        median_saving = (sorted_savings[mid - 1] + sorted_savings[mid]) / 2.0

    aggregate_saving = 1.0 - (sum_h1 / sum_d0) if sum_d0 > 0 else 0.0

    return {
        "per_task": per_task,
        "median_saving": median_saving,
        "aggregate_saving": aggregate_saving,
    }


def compute_delegation_metrics(tasks: list[dict[str, Any]]) -> dict[str, Any]:
    total_tasks = len(tasks)
    if total_tasks == 0:
        return {
            "total_tasks": 0,
            "worker_finished_share": 0.0,
            "worker_first_attempt_pass_rate": 0.0,
            "redesign_rate": 0.0,
            "takeover_rate": 0.0,
            "split_rate": 0.0,
        }

    worker_finished = sum(1 for t in tasks if t.get("executor") == "worker")
    first_attempt_passes = sum(1 for t in tasks if t.get("first_attempt_pass", False))
    redesigns = sum(1 for t in tasks if t.get("redesigns", 0) > 0)
    takeovers = sum(1 for t in tasks if t.get("takeover", False))
    splits = sum(1 for t in tasks if t.get("split", False))

    return {
        "total_tasks": total_tasks,
        "worker_finished_share": worker_finished / total_tasks,
        "worker_first_attempt_pass_rate": first_attempt_passes / total_tasks,
        "redesign_rate": redesigns / total_tasks,
        "takeover_rate": takeovers / total_tasks,
        "split_rate": splits / total_tasks,
    }


def compute_overhead_metrics(
    calls: list[dict[str, Any]], failed_tasks_tokens: int = 0
) -> dict[str, Any]:
    cloud_calls = [
        c for c in calls if c.get("role") in ("planner", "redesign", "takeover")
    ]
    cloud_tokens = sum(c.get("tokens", 0) for c in cloud_calls)
    if cloud_tokens == 0:
        return {
            "cloud_tokens": 0,
            "planner_share": 0.0,
            "redesign_takeover_share": 0.0,
            "failed_tasks_tokens": failed_tasks_tokens,
        }

    planner_tokens = sum(
        c.get("tokens", 0) for c in cloud_calls if c.get("role") == "planner"
    )
    redesign_takeover_tokens = sum(
        c.get("tokens", 0) for c in cloud_calls if c.get("role") in ("redesign", "takeover")
    )

    return {
        "cloud_tokens": cloud_tokens,
        "planner_share": planner_tokens / cloud_tokens,
        "redesign_takeover_share": redesign_takeover_tokens / cloud_tokens,
        "failed_tasks_tokens": failed_tasks_tokens,
    }


def compute_contract_quality_metrics(
    plans: list[dict[str, Any]], worker_contracts: list[dict[str, Any]]
) -> dict[str, Any]:
    total_plans = len(plans)
    if total_plans == 0:
        return {
            "plans_accepted_without_repair_rate": 0.0,
            "avg_repair_rounds_per_plan": 0.0,
            "validator_rejected_contracts": 0,
            "worker_first_attempt_failure_rate": 0.0,
        }

    accepted_without_repair = sum(
        1 for p in plans if p.get("accepted_without_repair", False)
    )
    total_repair_rounds = sum(p.get("repair_rounds", 0) for p in plans)
    rejected_contracts = sum(p.get("rejected_contracts", 0) for p in plans)

    w_failures = sum(1 for c in worker_contracts if c.get("first_attempt_failed", False))
    w_rate = w_failures / len(worker_contracts) if worker_contracts else 0.0

    return {
        "plans_accepted_without_repair_rate": accepted_without_repair / total_plans,
        "avg_repair_rounds_per_plan": total_repair_rounds / total_plans,
        "validator_rejected_contracts": rejected_contracts,
        "worker_first_attempt_failure_rate": w_rate,
    }


def compute_stability_metrics(
    task_runs: dict[str, list[bool]], repeats: int
) -> dict[str, Any]:
    counts: dict[str, int] = {}
    unstable: list[str] = []

    for task, runs in task_runs.items():
        pass_count = sum(runs)
        counts[task] = pass_count
        if 0 < pass_count < repeats:
            unstable.append(task)

    return {
        "counts": counts,
        "unstable_tasks": sorted(unstable),
    }


def compute_paired_table(
    d0_pass: dict[str, bool], h1_pass: dict[str, bool]
) -> dict[str, int]:
    both_pass = 0
    both_fail = 0
    only_d0 = 0
    only_h1 = 0

    all_tasks = set(d0_pass.keys()) | set(h1_pass.keys())
    for t in all_tasks:
        d0 = d0_pass.get(t, False)
        h1 = h1_pass.get(t, False)
        if d0 and h1:
            both_pass += 1
        elif not d0 and not h1:
            both_fail += 1
        elif d0 and not h1:
            only_d0 += 1
        else:
            only_h1 += 1

    return {
        "both_pass": both_pass,
        "both_fail": both_fail,
        "only_d0": only_d0,
        "only_h1": only_h1,
    }


def exact_sign_test(only_d0: int, only_h1: int) -> float:
    """Exact two-tailed sign test on discordant tasks."""
    n = only_d0 + only_h1
    if n == 0 or only_d0 == only_h1:
        return 1.0

    k = min(only_d0, only_h1)
    # One-tailed cumulative binomial probability: sum_{i=0}^k (n choose i) * 0.5^n
    cum_prob = sum(math.comb(n, i) for i in range(k + 1)) * (0.5**n)
    p_val = cum_prob * 2.0
    return min(1.0, p_val)

