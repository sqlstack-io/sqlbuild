"""E2E coverage of scenarios in a Snowflake project whose macros only Snowflake's parser reads."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    SnowflakeMacroScenarioRunOnTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    prepare_snowflake_macro_scenario_project,
    scenario_result_rows,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_PROJECT_COMPILE_FAILED: str = "Project compile  FAIL"
_HASH_SCENARIO: str = "order_fingerprints__hash"


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeMacroScenarioRunOnTestCase(
            description="sqb compile accepts the Snowflake-only macros",
            command=("--no-color", "compile"),
            expected_returncode=0,
            expected_rows=(),
            expected_absent_rows=(),
            expected_fragments=("Project compiled  3 models",),
        ),
        SnowflakeMacroScenarioRunOnTestCase(
            description="--run-on local replays the macro scenarios and refuses only the HASH one",
            command=("--no-color", "scenario", "test", "--run-on", "local"),
            expected_returncode=1,
            expected_rows=(
                ("order_flags__macros", "PASS", "local"),
                ("order_ids__plain", "PASS", "local"),
                (_HASH_SCENARIO, "ERROR", "local"),
            ),
            expected_absent_rows=(),
            expected_fragments=("error[SQ0008]: HASH gives a different value in DuckDB",),
        ),
        SnowflakeMacroScenarioRunOnTestCase(
            description="auto replays the macro scenarios and sends only the HASH one to the warehouse",
            command=("--no-color", "scenario", "test", "--run-on", "auto"),
            expected_returncode=1,
            expected_rows=(
                ("order_flags__macros", "PASS", "local"),
                ("order_ids__plain", "PASS", "local"),
            ),
            expected_absent_rows=(_HASH_SCENARIO,),
            expected_fragments=("Connecting to snowflake...",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_only_macros_when_running_then_only_unconvertible_scenarios_leave_duckdb(
    test_case: SnowflakeMacroScenarioRunOnTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_snowflake_macro_scenario_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_returncode, output
    assert _PROJECT_COMPILE_FAILED not in output, output
    name: str
    status: str
    where: str
    for name, status, where in test_case.expected_rows:
        assert scenario_result_rows(output=output, name=name) == [(status, where)], output
    for name in test_case.expected_absent_rows:
        assert scenario_result_rows(output=output, name=name) == [], output
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in output, output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
