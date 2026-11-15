"""Generates FLOW_REPORT.md and appends to flow_results.jsonl.

Python 3 standard library only.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def evaluate_decision_table(only_d0: int, median_saving: float) -> str:
    """Evaluate decision table rules from document 6 (TESTS.md B8)."""
    if only_d0 > 2:
        return (
            "Read every lost task, fix the top cause (contract quality, "
            "context, verifier gaps), and re-run once on the held-out set. "
            "If it still fails, stop"
        )
    if median_saving >= 0.30:
        return "Make flow the recommended mode for large tasks"
    if median_saving >= 0.10:
        return (
            "Keep flow, but tell the planner to delegate only delegable "
            "categories and keep the rest"
        )
    return (
        "Do not recommend decomposition. Keep only the cheap lane: "
        "local worker for delegable categories, orchestrator for everything else"
    )


def append_flow_results(path: Path, records: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as f:
        for r in records:
            f.write(json.dumps(r) + "\n")


def generate_markdown_report(data: dict[str, Any]) -> str:
    lines = []
    lines.append("# Flow Evaluation Report")
    lines.append("")

    # 1. Pins
    pins = data.get("pins", {})
    lines.append("## Configuration and Pins")
    lines.append("")
    lines.append("| Parameter | Value |")
    lines.append("| --- | --- |")
    for k, v in sorted(pins.items()):
        lines.append(f"| `{k}` | {v} |")
    lines.append("")

    # 2. Paired Table
    paired = data.get("paired_table", {})
    p_val = data.get("sign_test_p_val", 1.0)
    lines.append("## Paired Accuracy Table")
    lines.append("")
    lines.append("| Outcome | Count |")
    lines.append("| --- | --- |")
    lines.append(f"| Both pass | {paired.get('both_pass', 0)} |")
    lines.append(f"| Both fail | {paired.get('both_fail', 0)} |")
    lines.append(f"| Only D0 passes | {paired.get('only_d0', 0)} |")
    lines.append(f"| Only H1 passes | {paired.get('only_h1', 0)} |")
    lines.append("")
    lines.append(f"**Exact sign test p-value on discordant tasks:** `{p_val:.4f}`")
    lines.append("")

    # 3. Conditions
    conditions = data.get("conditions", {})
    lines.append("## Token and Cost Comparison")
    lines.append("")
    lines.append("| Condition | Runs | Pass Rate | Cloud Tokens | Cost ($) |")
    lines.append("| --- | --- | --- | --- | --- |")
    for cond_id, stats in sorted(conditions.items()):
        pass_pct = f"{stats.get('pass_rate', 0.0) * 100:.1f}%"
        cost_val = stats.get('cost', 0.0)
        lines.append(
            f"| `{cond_id}` | {stats.get('runs', 0)} | {pass_pct} | "
            f"{stats.get('tokens', 0):,} | ${cost_val:.4f} |"
        )
    lines.append("")

    savings = data.get("token_savings", {})
    med_sav = savings.get("median_saving", 0.0) * 100
    agg_sav = savings.get("aggregate_saving", 0.0) * 100
    lines.append(f"- **Median cloud token saving (H1 vs D0):** `{med_sav:.1f}%`")
    lines.append(f"- **Aggregate cloud token saving (H1 vs D0):** `{agg_sav:.1f}%`")
    lines.append("")

    # 4. Category breakdown
    by_cat = data.get("by_category", {})
    if by_cat:
        lines.append("## Category Breakdown")
        lines.append("")
        lines.append("| Category | H1 Pass Rate | D0 Pass Rate |")
        lines.append("| --- | --- | --- |")
        for cat, c_stats in sorted(by_cat.items()):
            h1_rate = f"{c_stats.get('h1_pass_rate', 0.0) * 100:.1f}%"
            d0_rate = f"{c_stats.get('d0_pass_rate', 0.0) * 100:.1f}%"
            lines.append(f"| `{cat}` | {h1_rate} | {d0_rate} |")
        lines.append("")

    # 5. Language breakdown
    by_lang = data.get("by_language", {})
    if by_lang:
        lines.append("## Language Breakdown")
        lines.append("")
        lines.append("| Language | H1 Pass Rate | D0 Pass Rate |")
        lines.append("| --- | --- | --- |")
        for lang, l_stats in sorted(by_lang.items()):
            h1_rate = f"{l_stats.get('h1_pass_rate', 0.0) * 100:.1f}%"
            d0_rate = f"{l_stats.get('d0_pass_rate', 0.0) * 100:.1f}%"
            lines.append(f"| `{lang}` | {h1_rate} | {d0_rate} |")
        lines.append("")

    # 6. Failure analysis (B9)
    b9 = data.get("b9_failure_tally", {})
    lines.append("## Failure Analysis (B9)")
    lines.append("")
    lines.append("| Cause | Tally | Recommended Action |")
    lines.append("| --- | --- | --- |")
    lines.append(f"| Contract at fault | {b9.get('contract_fault', 0)} | Tune planner prompt |")
    lines.append(f"| Worker at fault | {b9.get('worker_fault', 0)} | Check worker choice / limits |")
    lines.append(f"| Verifier at fault | {b9.get('verifier_fault', 0)} | Review verify commands |")
    lines.append("")

    # 7. Decision
    decision = data.get("decision", evaluate_decision_table(
        paired.get("only_d0", 0), savings.get("median_saving", 0.0)
    ))
    lines.append("## Decision Table Recommendation")
    lines.append("")
    lines.append(f"> **{decision}**")
    lines.append("")

    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate Flow evaluation report.")
    parser.add_argument("results_dir", type=Path, help="Directory containing run results")
    parser.add_argument(
        "--report",
        type=Path,
        default=Path("evals/FLOW_REPORT.md"),
        help="Path for generated FLOW_REPORT.md",
    )
    parser.add_argument(
        "--results-jsonl",
        type=Path,
        default=Path("evals/flow_results.jsonl"),
        help="Path for flow_results.jsonl",
    )
    args = parser.parse_args()

    summary_file = args.results_dir / "summary.json"
    if not summary_file.is_file():
        print(f"error: summary.json not found in {args.results_dir}", file=sys.stderr)
        return 1

    with summary_file.open("r", encoding="utf-8") as f:
        data = json.load(f)

    report_content = generate_markdown_report(data)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(report_content, encoding="utf-8")
    print(f"Report written to {args.report}")

    raw_results_file = args.results_dir / "results.json"
    if raw_results_file.is_file():
        with raw_results_file.open("r", encoding="utf-8") as f:
            records = json.load(f)
        append_flow_results(args.results_jsonl, records)
        print(f"Results appended to {args.results_jsonl}")

    return 0


if __name__ == "__main__":
    sys.exit(main())

