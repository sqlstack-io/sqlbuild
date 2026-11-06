"""E2E coverage that compile-step SQL scanning follows DuckDB escape-string rules."""

from __future__ import annotations

from pathlib import Path
from subprocess import CompletedProcess

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import SqlTestE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "escape_strings"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "escape_strings.duckdb"\n'
    ),
    "models/stg_orders.sql": "MODEL (description 'Test model stg_orders.', materialized table);\n\nSELECT 1 AS order_id\n",
    "models/order_names.sql": (
        "MODEL (description 'Test model order_names.', materialized table);\n\n"
        "SELECT E'O\\'Brien' AS customer_name, 'C:\\' AS folder, order_id\n"
        'FROM __ref("stg_orders")\n'
    ),
    "tests/unit/test_order_names.sql": (
        "TEST ();\n\n"
        "WITH\n"
        "__ref__stg_orders AS (SELECT 1 AS order_id),\n"
        "__assert__named_orders AS (\n"
        "  SELECT E'O\\'Brien), (' AS note, customer_name\n"
        '  FROM __ref("order_names")\n'
        "  WHERE customer_name <> 'O''Brien'\n"
        ")\n"
        "SELECT 1\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    (
        SqlTestE2ETestCase(
            description="escape strings and a plain trailing backslash before references and markers",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1  FAIL=0",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_duckdb_escape_strings_before_references_when_building_and_testing_then_succeeds(
    test_case: SqlTestE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="escape_strings", repo_files=_PROJECT_FILES
    )

    built: CompletedProcess[str] = run_sqb(command=("--no-color", "build"), project_dir=project_dir)
    tested: CompletedProcess[str] = run_sqb(command=("--no-color", "test"), project_dir=project_dir)

    assert built.returncode == test_case.expected_exit_code, built.stdout + built.stderr
    assert tested.returncode == test_case.expected_exit_code, tested.stdout + tested.stderr
    assert test_case.expected_stdout_fragment in tested.stdout, tested.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
