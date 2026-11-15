"""Task schema and loaders for flow evaluations.

Python 3 standard library only.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

VALID_LANGUAGES = {"rust", "python", "go", "typescript"}
VALID_SIZES = {"S", "M", "L"}
VALID_CATEGORIES = {
    "boilerplate",
    "single_function",
    "tests",
    "docs",
    "local_refactor",
    "local_bugfix",
    "multi_file",
    "other",
}


@dataclass
class TaskDefinition:
    name: str
    language: str
    size: str
    category: str
    repo: str
    repo_path: Path
    objective: str
    public_verify: list[str]
    hidden_verify: list[str]
    regression_verify: list[str]
    protected: list[str]
    reference_patch: str
    hidden_dir: Path | None = None

    @classmethod
    def from_dict(cls, data: dict[str, Any], base_dir: Path | None = None) -> TaskDefinition:
        from evals.flow_validity import TaskValidationError

        required = [
            "name",
            "language",
            "size",
            "category",
            "repo",
            "objective",
            "public_verify",
            "hidden_verify",
            "regression_verify",
            "protected",
            "reference_patch",
        ]
        for req in required:
            if req not in data:
                raise TaskValidationError(f"Missing required task field: '{req}'")

        language = str(data["language"]).lower()
        if language not in VALID_LANGUAGES:
            raise TaskValidationError(
                f"Invalid language '{language}'; must be one of {sorted(VALID_LANGUAGES)}"
            )

        size = str(data["size"]).upper()
        if size not in VALID_SIZES:
            raise TaskValidationError(
                f"Invalid size '{size}'; must be one of {sorted(VALID_SIZES)}"
            )

        category = str(data["category"]).lower()
        if category not in VALID_CATEGORIES:
            raise TaskValidationError(
                f"Invalid category '{category}'; must be one of {sorted(VALID_CATEGORIES)}"
            )

        repo_str = str(data["repo"])
        if base_dir:
            repo_path = (base_dir / repo_str).resolve()
            if not repo_path.exists():
                repo_path = Path(repo_str).resolve()
        else:
            repo_path = Path(repo_str).resolve()

        patch_val = str(data["reference_patch"])
        if base_dir and (base_dir / patch_val).is_file():
            reference_patch = (base_dir / patch_val).read_text(encoding="utf-8")
        else:
            reference_patch = patch_val

        hidden_dir = None
        if base_dir and (base_dir / "hidden").is_dir():
            hidden_dir = base_dir / "hidden"

        return cls(
            name=str(data["name"]),
            language=language,
            size=size,
            category=category,
            repo=repo_str,
            repo_path=repo_path,
            objective=str(data["objective"]),
            public_verify=[str(cmd) for cmd in data["public_verify"]],
            hidden_verify=[str(cmd) for cmd in data["hidden_verify"]],
            regression_verify=[str(cmd) for cmd in data["regression_verify"]],
            protected=[str(p) for p in data["protected"]],
            reference_patch=reference_patch,
            hidden_dir=hidden_dir,
        )


@dataclass
class PlannerTask:
    name: str
    objective: str
    repo: str
    repo_path: Path
    public_verify: list[str]
    category: str = "other"


@dataclass
class NanoTask:
    name: str
    category: str
    repo: str
    repo_path: Path
    contract: dict[str, Any]
    hidden_verify: list[str]
    reference_patch: str = ""


def load_e2e_task(task_path: Path) -> TaskDefinition:
    if task_path.is_dir():
        json_file = task_path / "task.json"
        if not json_file.is_file():
            from evals.flow_validity import TaskValidationError
            raise TaskValidationError(f"Missing task.json in {task_path}")
        base_dir = task_path
    else:
        json_file = task_path
        base_dir = task_path.parent

    with json_file.open("r", encoding="utf-8") as f:
        data = json.load(f)
    if "name" not in data:
        data["name"] = base_dir.name
    return TaskDefinition.from_dict(data, base_dir=base_dir)


def load_planner_task(task_path: Path) -> PlannerTask:
    if task_path.is_dir():
        planner_json = task_path / "planner.json"
        if planner_json.is_file():
            with planner_json.open("r", encoding="utf-8") as f:
                data = json.load(f)
            return PlannerTask(
                name=data.get("name", task_path.name),
                objective=data["objective"],
                repo=data["repo"],
                repo_path=(task_path / data["repo"]).resolve() if (task_path / data["repo"]).exists() else Path(data["repo"]).resolve(),
                public_verify=data.get("public_verify", []),
                category=data.get("category", "other"),
            )
        # Fall back to extracting from e2e task.json
        e2e = load_e2e_task(task_path)
        return PlannerTask(
            name=e2e.name,
            objective=e2e.objective,
            repo=e2e.repo,
            repo_path=e2e.repo_path,
            public_verify=e2e.public_verify,
            category=e2e.category,
        )
    with task_path.open("r", encoding="utf-8") as f:
        data = json.load(f)
    return PlannerTask(
        name=data.get("name", task_path.stem),
        objective=data["objective"],
        repo=data["repo"],
        repo_path=Path(data["repo"]).resolve(),
        public_verify=data.get("public_verify", []),
        category=data.get("category", "other"),
    )


def load_nano_task(task_path: Path) -> NanoTask:
    if task_path.is_dir():
        nano_json = task_path / "nano.json"
        if nano_json.is_file():
            with nano_json.open("r", encoding="utf-8") as f:
                data = json.load(f)
            base_dir = task_path
        else:
            with (task_path / "task.json").open("r", encoding="utf-8") as f:
                data = json.load(f)
            base_dir = task_path
    else:
        with task_path.open("r", encoding="utf-8") as f:
            data = json.load(f)
        base_dir = task_path.parent

    repo_str = data.get("repo", "setup")
    repo_path = (base_dir / repo_str).resolve() if (base_dir / repo_str).exists() else Path(repo_str).resolve()
    patch_val = data.get("reference_patch", "")
    if (base_dir / patch_val).is_file():
        reference_patch = (base_dir / patch_val).read_text(encoding="utf-8")
    else:
        reference_patch = patch_val

    return NanoTask(
        name=data.get("name", base_dir.name),
        category=data.get("category", "other"),
        repo=repo_str,
        repo_path=repo_path,
        contract=data.get("contract", {}),
        hidden_verify=data.get("hidden_verify", []),
        reference_patch=reference_patch,
    )


def load_tasks_from_dir(tasks_dir: Path) -> list[TaskDefinition]:
    if not tasks_dir.is_dir():
        return []
    tasks = []
    for item in sorted(tasks_dir.iterdir()):
        if item.is_dir() and (item / "task.json").is_file():
            tasks.append(load_e2e_task(item))
    return tasks

