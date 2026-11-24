from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.sqlite._test_types import (
    SqliteScenarioE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.sqlite.helpers import (
    SCENARIO_OUTCOMES,
    SCENARIO_PROJECT_FILES,
    SQLITE_FILE,
    scenario_outcomes,
    scenario_table_shapes,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteScenarioE2ETestCase(
            description="fixtures, expected rows and assertions run in the SQLite file",
            command=("scenario", "test"),
            expected_outcomes=SCENARIO_OUTCOMES,
            expected_tables=("_sqlbuild_relations", "_sqlbuild_schemas"),
        ),
        SqliteScenarioE2ETestCase(
            description="concurrent scenarios in one namespace share the SQLite writer",
            command=("scenario", "test", "--concurrency", "4", "--scenario-namespace", "ci_one"),
            expected_outcomes=SCENARIO_OUTCOMES,
            expected_tables=("_sqlbuild_relations", "_sqlbuild_schemas"),
        ),
        SqliteScenarioE2ETestCase(
            description="SQLite scenarios can't run locally: SQLQuack converts Snowflake SQL only",
            command=("scenario", "test", "--run-on", "local", "--sync-snapshots"),
            expected_outcomes=dict.fromkeys(SCENARIO_OUTCOMES, ("failed", "X607")),
            expected_tables=("_sqlbuild_relations", "_sqlbuild_schemas"),
        ),
        SqliteScenarioE2ETestCase(
            description="retained scenarios keep their SQLite fixture and model tables",
            command=("scenario", "test", "customer_totals_pass", "--retain"),
            expected_outcomes={"customer_totals_pass": ("success", None)},
            expected_tables=(
                "_sqlbuild_relations",
                "_sqlbuild_schemas",
                "analytics____sqb_<run>__model__customer_totals",
                "analytics____sqb_<run>__model__daily_orders",
                "analytics____sqb_<run>__model__orders",
                "analytics____sqb_<run>__source__raw_orders",
            ),
            expected_return_code=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_orders_scenarios_when_testing_then_sqlite_reports_each_outcome(
    tmp_path: Path, test_case: SqliteScenarioE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="scenario_shop", repo_files=SCENARIO_PROJECT_FILES
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command, "--json"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    assert scenario_outcomes(payload) == test_case.expected_outcomes
    assert scenario_table_shapes(db_path=project_dir / SQLITE_FILE) == test_case.expected_tables
