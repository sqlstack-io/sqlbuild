"""E2E coverage of local replay of a Snowflake lambda, which SQLQuack's default policy refuses."""

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
            description="a Snowflake FILTER over a constructed array is refused as a JSON divergence",
            scenario_name="order_quantities__lambda",
            expected_stdout_fragment="fn:ARRAY_CONSTRUCT needs known divergence `json_fidelity`",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_arrow_lambda_when_replaying_locally_then_sqlquack_refuses_it(
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

    assert replayed.returncode == 1, replayed.stdout + replayed.stderr
    assert "error[X607]" in replayed.stdout, replayed.stdout
    assert test_case.expected_stdout_fragment in replayed.stdout, replayed.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
