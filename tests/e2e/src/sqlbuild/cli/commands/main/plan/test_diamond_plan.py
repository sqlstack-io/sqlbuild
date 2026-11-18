"""E2E coverage for plan output over layered shared dependencies."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.lineage.helpers import (
    DIAMOND_LAYERS,
    diamond_model_names,
)
from tests.e2e.src.sqlbuild.cli.commands.main.plan._test_types import (
    DiamondPlanE2ETestCase,
    DiamondPlanJsonE2ETestCase,
    PlanFromE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.plan.helpers import (
    prepare_changed_incremental_diamond_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_PLAN_ENTRY: re.Pattern[str] = re.compile(r"^[├└]── (orders_\w+)\s", re.MULTILINE)


@pytest.mark.parametrize(
    "test_case",
    [
        DiamondPlanE2ETestCase(
            description="plan tree lists each diamond model once without inherited replay",
            expected_fragments=(
                "Query changed (1)",
                f"Models ({len(diamond_model_names()) - 1})",
                f"└── orders_{DIAMOND_LAYERS} ",
            ),
            unexpected_fragments=("Upstream changed", "cause"),
            expected_max_lines_per_model=4,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_changed_diamond_root_when_planning_then_tree_lists_each_model_once(
    test_case: DiamondPlanE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_changed_incremental_diamond_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout
    for fragment in test_case.unexpected_fragments:
        assert fragment not in result.stdout, result.stdout
    plan_output: str = result.stdout[result.stdout.index("Plan ready") :]
    assert sorted(_PLAN_ENTRY.findall(plan_output)) == sorted(diamond_model_names())
    assert len(plan_output.splitlines()) <= test_case.expected_max_lines_per_model * len(
        diamond_model_names()
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DiamondPlanJsonE2ETestCase(
            description="plan json lists each diamond model once with its own reason",
            expected_reasons={
                "orders_0": "query_changed",
                **{name: "normal_incremental" for name in diamond_model_names()[1:]},
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_changed_diamond_root_when_planning_json_then_lists_each_model_once(
    test_case: DiamondPlanJsonE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_changed_incremental_diamond_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--json"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    models: list[dict[str, object]] = json.loads(result.stdout)["models"]
    reasons: dict[str, object] = {str(model["name"]): model["reason"] for model in models}
    assert len(models) == len(reasons)
    assert reasons == test_case.expected_reasons


@pytest.mark.parametrize(
    "test_case",
    [
        PlanFromE2ETestCase(
            description="planning from synced state on DuckDB plans live and says why",
            plan_args=("--plan-from", "synced_state"),
            expected_stderr="Planning live: the duckdb adapter cannot check a synced copy\n",
            unexpected_stderr="Planned from synced state",
        ),
        PlanFromE2ETestCase(
            description="planning live says nothing about synced state",
            plan_args=(),
            expected_stderr="",
            unexpected_stderr="Planning live",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_duckdb_target_when_planning_from_synced_state_then_plans_live_and_says_why(
    test_case: PlanFromE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_changed_incremental_diamond_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", *test_case.plan_args), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_stderr in result.stderr, result.stderr
    assert test_case.unexpected_stderr not in result.stderr, result.stderr
    assert "Query changed (1)" in result.stdout, result.stdout
