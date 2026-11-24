"""E2E coverage for casting bare NULL fixture columns to declared source types."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    FixtureDeclaredTypeE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "fixture_types"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "fixture_types.duckdb"\n'
    ),
    "sql/sources/raw_orders.yml": (
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n    table: raw_orders\n    columns:\n"
        "      - name: order_id\n        type: INTEGER\n"
        "      - name: ordered_on\n        type: DATE\n"
    ),
    "sql/models/order_years.sql": (
        "MODEL (description 'Test model order_years.');\n"
        'SELECT o.order_id, YEAR(o.ordered_on) AS order_year FROM __source("raw_orders") AS o\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        FixtureDeclaredTypeE2ETestCase(
            description="single select fixture",
            fixture_sql="SELECT 1 AS order_id, NULL AS ordered_on",
            expected_sql="SELECT 1 AS order_id, CAST(NULL AS BIGINT) AS order_year",
            expected_artifact_fragment='CAST("__sqlbuild_partial_fixture".ordered_on AS DATE)',
        ),
        FixtureDeclaredTypeE2ETestCase(
            description="union all fixture",
            fixture_sql=(
                "SELECT 1 AS order_id, NULL AS ordered_on\n"
                "  UNION ALL SELECT 2 AS order_id, NULL AS ordered_on"
            ),
            expected_sql=(
                "SELECT 1 AS order_id, CAST(NULL AS BIGINT) AS order_year\n"
                "  UNION ALL SELECT 2 AS order_id, CAST(NULL AS BIGINT) AS order_year"
            ),
            expected_artifact_fragment='CAST("__sqlbuild_partial_fixture".ordered_on AS DATE)',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bare_null_source_fixture_when_testing_then_casts_to_declared_type(
    test_case: FixtureDeclaredTypeE2ETestCase, tmp_path: Path
) -> None:
    files: dict[str, str] = {
        **_PROJECT_FILES,
        "tests/unit/test_order_years.sql": (
            "TEST();\nWITH\n"
            f"__source__raw_orders AS (\n  {test_case.fixture_sql}\n),\n"
            f"__expected__order_years AS (\n  {test_case.expected_sql}\n)\nSELECT 1\n"
        ),
    }
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="fixture_types", repo_files=files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )

    artifact: str = (project_dir / "target/run/tests/order_years/test_order_years.sql").read_text(
        encoding="utf-8"
    )
    assert test_case.expected_stdout_fragment in result.stdout, result.stdout + result.stderr
    assert test_case.expected_artifact_fragment in artifact


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
