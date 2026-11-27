"""E2E coverage of a scenario whose SQLQuack guard raises when it replays in DuckDB."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    GuardedScenarioRunOnTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    SNOWFLAKE_TWO_ORDER_IDS_FIXTURE_SQL,
    build_snowflake_guarded_scenario_project_files,
    write_snowflake_order_ids_snapshot,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SCENARIO: str = "order_id_lists__guarded"
_LOCAL_LINE: str = "table     order_id_lists__guarded/order_id_lists START"


@pytest.mark.parametrize(
    "test_case",
    [
        GuardedScenarioRunOnTestCase(
            description="--run-on local makes it an error naming the guard",
            run_on="local",
            expected_text=(
                "   = note: SQLQuack's guard array_agg_order raised in DuckDB: ARRAY_AGG of"
                " several values in no defined order\n"
                "   = help: run these scenarios on the warehouse with --run-on auto\n"
            ),
            expected_local_lines=0,
        ),
        GuardedScenarioRunOnTestCase(
            description="--run-on local lists it under Can't run locally",
            run_on="local",
            expected_text=("Can't run locally (1 scenario):\n\nerror: SQLQuack guard"),
            expected_local_lines=0,
        ),
        GuardedScenarioRunOnTestCase(
            description="auto runs it again on the warehouse and hides the local attempt",
            run_on="auto",
            expected_text="Connecting to snowflake...\n",
            expected_local_lines=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_whose_guard_raises_locally_when_testing_then_it_needs_the_warehouse(
    test_case: GuardedScenarioRunOnTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_guarded_scenario",
        repo_files=build_snowflake_guarded_scenario_project_files(),
    )
    write_snowflake_order_ids_snapshot(
        project_dir=project_dir,
        scenario_name=_SCENARIO,
        fixture_sql=SNOWFLAKE_TWO_ORDER_IDS_FIXTURE_SQL,
        order_ids=(1, 2),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test", "--run-on", test_case.run_on, _SCENARIO),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert test_case.expected_text in output, output
    assert output.count(_LOCAL_LINE) == test_case.expected_local_lines, output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
