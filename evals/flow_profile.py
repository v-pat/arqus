"""Builds profile_stats.json from stats.jsonl.

Only includes categories with n >= 5.
Python 3 standard library only.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any


def default_arqus_home() -> Path:
    env_home = os.environ.get("ARQUS_HOME")
    if env_home:
        return Path(env_home).resolve()
    return Path.home() / ".arqus"


def build_profile_stats(
    stats_path: Path, model_filter: str | None = None
) -> dict[str, Any]:
    if not stats_path.is_file():
        return {"categories": {}}

    cat_counts: dict[str, int] = {}
    cat_passes: dict[str, int] = {}

    with stats_path.open("r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                continue

            if model_filter and record.get("model") != model_filter:
                continue

            category = record.get("category")
            if not category:
                continue

            cat_counts[category] = cat_counts.get(category, 0) + 1
            if record.get("first_attempt_pass", False):
                cat_passes[category] = cat_passes.get(category, 0) + 1

    eligible_categories: dict[str, dict[str, Any]] = {}
    for cat, n in cat_counts.items():
        if n >= 5:
            passes = cat_passes.get(cat, 0)
            eligible_categories[cat] = {
                "n": n,
                "first_attempt_pass_rate": round(passes / n, 4),
            }

    return {"categories": eligible_categories}


def generate_profile_file(
    stats_path: Path, out_path: Path, model_filter: str | None = None
) -> None:
    data = build_profile_stats(stats_path, model_filter)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    home = default_arqus_home()
    default_stats = home / "flow" / "stats.jsonl"
    default_out = home / "flow" / "profile_stats.json"

    parser = argparse.ArgumentParser(
        description="Build profile_stats.json from flow stats.jsonl"
    )
    parser.add_argument(
        "--stats",
        type=Path,
        default=default_stats,
        help=f"Path to stats.jsonl (default: {default_stats})",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=default_out,
        help=f"Output path for profile_stats.json (default: {default_out})",
    )
    parser.add_argument(
        "--model",
        type=str,
        default=None,
        help="Filter stats by worker model name",
    )
    args = parser.parse_args()

    if not args.stats.is_file():
        print(f"error: stats file not found at {args.stats}", file=sys.stderr)
        return 1

    generate_profile_file(args.stats, args.out, args.model)
    print(f"Generated profile stats at {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

