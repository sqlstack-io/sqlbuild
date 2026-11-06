"""E2E coverage of local replay of captured-dialect lambdas, which DuckDB 2.0 needs as lambda x:."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    CapturedLambdaScenarioTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_snowflake_lambda_scenario_project_files,
    write_snowflake_order_ids_snapshot,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        CapturedLambdaScenarioTestCase(
            description="a Snowflake FILTER with an arrow lambda replays locally",
            scenario_name="order_quantities__lambda",
            expected_stdout_fragment="PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_arrow_lambda_when_replaying_locally_then_the_scenario_passes(
    test_case: CapturedLambdaScenarioTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_lambda",
        repo_files=build_snowflake_lambda_scenario_project_files(),
    )
    write_snowflake_order_ids_snapshot(
        project_dir=project_dir, scenario_name=test_case.scenario_name
    )

    replayed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test", "--local", "--strict", test_case.scenario_name),
        project_dir=project_dir,
    )

    assert replayed.returncode == 0, replayed.stdout + replayed.stderr
    assert test_case.expected_stdout_fragment in replayed.stdout, replayed.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
