"""Integration coverage for SQL test planning during real builds."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import BuildTestPlanningTestCase

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "demo.duckdb"\n'
    ),
    "sql/models/orders.sql": (
        "MODEL (description 'Test model orders.',\n"
        "  materialized table,\n"
        "  contract enforced,\n"
        "  columns (\n"
        "    order_id (type INTEGER, nullable false),\n"
        "    amount (type INTEGER),\n"
        "  ),\n"
        ");\n\n"
        "SELECT 1 AS order_id, 5 AS amount\n"
    ),
    "sql/models/order_totals.sql": (
        'MODEL (description "Test model order_totals.", materialized table);\n\nSELECT order_id, amount * 2 AS doubled FROM __ref("orders")\n'
    ),
    "tests/unit/test_order_totals.sql": (
        "TEST ();\nWITH\n"
        "__ref__orders AS (SELECT 1 AS order_idd, 5 AS amount),\n"
        "__expected__order_totals AS (SELECT 1 AS order_id, 10 AS doubled)\n"
        "SELECT 1\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        BuildTestPlanningTestCase(
            description="build with tests rejects an invalid SQL test fixture",
            build_flags=(),
            expected_exit_code=1,
            expected_fragment="error[B302]",
        ),
        BuildTestPlanningTestCase(
            description="build without tests does not plan SQL tests",
            build_flags=("--no-tests",),
            expected_exit_code=0,
            expected_fragment="Completed successfully",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_test_fixture_when_building_then_only_test_runs_plan_sql_tests(
    test_case: BuildTestPlanningTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    for relative_path, contents in _PROJECT_FILES.items():
        destination: Path = tmp_path / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(contents, encoding="utf-8")

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "build", *test_case.build_flags]
    )
    captured: str = "".join(capsys.readouterr())

    assert exit_code == test_case.expected_exit_code, captured
    assert test_case.expected_fragment in captured


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
