"""E2E tests for `__empty_fixture()` mocks in SQL scenarios."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    ScenarioEmptyFixtureE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_empty_fixture_scenario_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_OPEN_CUSTOMER_COLUMNS: str = (
    "    columns:\n"
    "      - name: id\n"
    "        type: INTEGER\n"
    "      - name: name\n"
    "        type: VARCHAR\n"
)
_TYPED_CUSTOMER_COLUMNS: str = "    contract: enforced\n" + _OPEN_CUSTOMER_COLUMNS


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioEmptyFixtureE2ETestCase(
            description="typed source declaration shapes the empty fixture",
            customer_columns_yaml=_TYPED_CUSTOMER_COLUMNS,
            expected_exit_code=0,
            expected_fragments=(
                "expect    assertion no_unmatched_orders",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
        ),
        ScenarioEmptyFixtureE2ETestCase(
            description="source without an enforced contract is rejected clearly",
            customer_columns_yaml=_OPEN_CUSTOMER_COLUMNS,
            expected_exit_code=1,
            expected_fragments=(
                "error[S510]",
                "mock source 'raw_customers' uses __empty_fixture() but its column schema is "
                "not authoritative",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_empty_fixture_scenario_when_running_rules_and_scenario_then_types_come_from_source(
    test_case: ScenarioEmptyFixtureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_empty_fixture",
        repo_files=build_empty_fixture_scenario_project_files(
            customer_columns_yaml=test_case.customer_columns_yaml
        ),
    )

    rules: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "rules", "run", "SQLCLEAN021"), project_dir=project_dir
    )
    scenario: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"), project_dir=project_dir
    )

    assert rules.returncode == 0, rules.stdout + rules.stderr
    assert "SQLCLEAN021" not in rules.stdout.replace("Rule selection SQLCLEAN021 passed", "")
    assert scenario.returncode == test_case.expected_exit_code, scenario.stdout + scenario.stderr
    for fragment in test_case.expected_fragments:
        assert fragment in scenario.stdout, scenario.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioEmptyFixtureE2ETestCase(
            description="empty fixture casts every declared source column",
            customer_columns_yaml=_TYPED_CUSTOMER_COLUMNS,
            expected_exit_code=0,
            expected_fragments=(
                'CAST(NULL AS INTEGER) AS "id"',
                'CAST(NULL AS VARCHAR) AS "name"',
                "WHERE FALSE",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_typed_empty_fixture_when_running_scenario_then_fixture_casts_declared_types(
    test_case: ScenarioEmptyFixtureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_empty_fixture",
        repo_files=build_empty_fixture_scenario_project_files(
            customer_columns_yaml=test_case.customer_columns_yaml
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    fixture_sql: str = (
        project_dir
        / "target/run/scenarios/orders_without_customers/fixtures/source__raw_customers.sql"
    ).read_text(encoding="utf-8")
    for fragment in test_case.expected_fragments:
        assert fragment in fixture_sql
    assert "__empty_fixture" not in fixture_sql


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
