"""E2E coverage that scenario check and fixture SQL run as authored, not regenerated."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    ScenarioAuthoredCheckSqlE2ETestCase,
    ScenarioLocalReplayProjectDialectE2ETestCase,
    ScenarioUnresolvableCheckSqlE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_offline_snowflake_project_toml,
    build_scenario_project_files,
    restamp_scenario_snapshot_capture_adapter,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioAuthoredCheckSqlE2ETestCase(
            description="DuckDB integer division in fixture, expected and assertion SQL",
            scenario_sql=(
                "SCENARIO (description 'Test scenario.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 21 // 2 AS amount -- integer division\n"
                "  UNION ALL\n"
                "  SELECT 2 AS id, 5 AS amount\n"
                "),\n"
                "__expected__order_totals AS (\n"
                "  SELECT 30 // 2 AS total_amount\n"
                "),\n"
                "__assert__no_odd_halves AS (\n"
                '  SELECT * FROM __ref("order_totals") WHERE total_amount // 5 <> 3\n'
                ")\n"
                "SELECT 1\n"
            ),
            expected_stdout_fragment="PASS=1  FAIL=0",
        ),
        ScenarioAuthoredCheckSqlE2ETestCase(
            description="DuckDB escape strings between relation markers in assertion SQL",
            scenario_sql=(
                "SCENARIO (description 'Test scenario.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "  UNION ALL\n"
                "  SELECT 2 AS id, 5 AS amount\n"
                "),\n"
                "__assert__no_named_totals AS (\n"
                "  SELECT * FROM __ref(\"order_totals\") WHERE E'O\\'Brien' = 'x'\n"
                "  UNION ALL\n"
                '  SELECT * FROM __ref("order_totals") WHERE total_amount <> 15\n'
                "    AND E'D\\'Arcy' IS NOT NULL\n"
                ")\n"
                "SELECT 1\n"
            ),
            expected_stdout_fragment="PASS=1  FAIL=0",
        ),
        ScenarioAuthoredCheckSqlE2ETestCase(
            description="DuckDB escape string before the first relation marker",
            scenario_sql=(
                "SCENARIO (description 'Test scenario.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "  UNION ALL\n"
                "  SELECT 2 AS id, 5 AS amount\n"
                "),\n"
                "__assert__no_named_totals AS (\n"
                "  SELECT E'O\\'Brien' AS customer_name, total_amount\n"
                '  FROM __ref("order_totals") WHERE total_amount <> 15\n'
                ")\n"
                "SELECT 1\n"
            ),
            expected_stdout_fragment="PASS=1  FAIL=0",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dialect_specific_scenario_sql_when_testing_then_runs_authored_sql(
    test_case: ScenarioAuthoredCheckSqlE2ETestCase, tmp_path: Path
) -> None:
    repo_files: dict[str, str] = build_scenario_project_files()
    del repo_files["tests/scenarios/order_totals_pass.sql"]
    del repo_files["tests/scenarios/order_totals_fail.sql"]
    del repo_files["tests/scenarios/nested/orders_assert_pass.sql"]
    repo_files["tests/scenarios/order_halves.sql"] = test_case.scenario_sql
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="authored_scenario_sql", repo_files=repo_files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioUnresolvableCheckSqlE2ETestCase(
            description="DuckDB backslash before a quote leaves the assertion string unclosed",
            scenario_sql=(
                "SCENARIO (description 'Test scenario.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "),\n"
                "__assert__no_named_totals AS (\n"
                "  SELECT * FROM __ref(\"order_totals\") WHERE 'O\\'Brien' = 'x'\n"
                "  UNION ALL\n"
                '  SELECT * FROM __ref("order_totals")\n'
                ")\n"
                "SELECT 1\n"
            ),
            expected_output_fragments=("unclosed quoted string",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unclosed_scenario_string_when_testing_then_fails_without_running(
    test_case: ScenarioUnresolvableCheckSqlE2ETestCase, tmp_path: Path
) -> None:
    repo_files: dict[str, str] = build_scenario_project_files()
    del repo_files["tests/scenarios/order_totals_pass.sql"]
    del repo_files["tests/scenarios/order_totals_fail.sql"]
    del repo_files["tests/scenarios/nested/orders_assert_pass.sql"]
    repo_files["tests/scenarios/order_halves.sql"] = test_case.scenario_sql
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="unresolvable_scenario_sql", repo_files=repo_files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode != 0, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioLocalReplayProjectDialectE2ETestCase(
            description="Snowflake backslash-escaped strings around a later marker",
            scenario_name="named_orders",
            source_fixture_sql="SELECT 1 AS id, 10 AS amount UNION ALL SELECT 2 AS id, 5 AS amount",
            assertion_sql=(
                "SELECT * FROM __ref(\"order_totals\") WHERE 'O\\'Brien' = 'x'\n"
                "  UNION ALL\n"
                '  SELECT * FROM __ref("order_totals")\n'
                "  WHERE total_amount <> 15 AND 'D\\'Arcy' IS NULL"
            ),
            expected_stdout_fragment="PASS=1  FAIL=0  TOTAL=1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_dialect_scenario_sql_when_replaying_locally_then_resolves_every_marker(
    test_case: ScenarioLocalReplayProjectDialectE2ETestCase, tmp_path: Path
) -> None:
    repo_files: dict[str, str] = build_scenario_project_files()
    del repo_files["tests/scenarios/order_totals_pass.sql"]
    del repo_files["tests/scenarios/order_totals_fail.sql"]
    del repo_files["tests/scenarios/nested/orders_assert_pass.sql"]
    # SQLQuack converts a model only when the models it reads declare their column types.
    repo_files["sql/models/orders.sql"] = (
        "MODEL (\n  description 'Test model orders.',\n  materialized table,\n"
        "  columns (order_id (type NUMBER(38,0)), amount (type NUMBER(38,0))),\n);\n\n"
        'SELECT id AS order_id, amount FROM __source("raw_orders")\n'
    )
    repo_files["sql/models/order_totals.sql"] = (
        "MODEL (\n  description 'Test model order_totals.',\n  materialized table,\n"
        "  columns (total_amount (type NUMBER(38,0))),\n);\n\n"
        'SELECT SUM(amount) AS total_amount FROM __ref("orders")\n'
    )
    repo_files[f"tests/scenarios/{test_case.scenario_name}.sql"] = (
        "SCENARIO (description 'Test scenario orders_assert_pass.');\n\n"
        "WITH\n"
        f"__source__raw_orders AS (\n  {test_case.source_fixture_sql}\n),\n"
        f"__assert__no_named_totals AS (\n  {test_case.assertion_sql}\n)\n"
        "SELECT 1\n"
    )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="project_dialect_local_replay", repo_files=repo_files
    )
    capture_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "capture", test_case.scenario_name),
        project_dir=project_dir,
    )
    assert capture_result.returncode == 0, capture_result.stdout + capture_result.stderr
    (project_dir / "sqlbuild_project.toml").write_text(
        build_offline_snowflake_project_toml(), encoding="utf-8"
    )
    restamp_scenario_snapshot_capture_adapter(
        project_dir=project_dir,
        scenario_name=test_case.scenario_name,
        source_fixture_sql={"raw_orders": test_case.source_fixture_sql},
        capture_adapter="snowflake",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test", test_case.scenario_name, "--run-on", "local"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
