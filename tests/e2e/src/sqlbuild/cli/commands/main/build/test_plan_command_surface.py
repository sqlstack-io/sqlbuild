"""E2E tests for focused plan command surface behavior."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    PlanCommandBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_waffle_shop, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        PlanCommandBuildE2ETestCase(
            description="plan select no-color scopes to marts",
            command=("--no-color", "plan", "--select", "path:sql/models/marts"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Plan ready  10 selected",
                "Functions (2)",
                "is_completed_order",
                "sql udf",
                "is_completed_order_py",
                "python udf",
                "Models (8)",
            ),
            expected_stderr_fragments=(),
        ),
        PlanCommandBuildE2ETestCase(
            description="plan exclude removes marts branch from selected scope",
            command=(
                "--no-color",
                "plan",
                "--select",
                "/sql/models/marts",
                "--exclude",
                "hourly_order_activity",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Plan ready  9 selected",
                "Functions (2)",
                "Models (7)",
            ),
            expected_stderr_fragments=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_plan_command_variants_when_running_cli_then_scope_behavior_matches_expectation(
    test_case: PlanCommandBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    initial_build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )
    assert initial_build_result.returncode == 0, (
        initial_build_result.stdout + initial_build_result.stderr
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stdout + result.stderr
