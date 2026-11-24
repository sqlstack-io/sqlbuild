"""E2E coverage for INTERSECT and EXCEPT in SQL-test expected CTEs."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    SetOperationExpectedE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    run_sqb,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "set_operation_expected"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "set_operation_expected.duckdb"\n\n'
        '[defaults]\nmaterialized = "table"\n'
    ),
    "sql/macros/python/status.py": (
        'def normalize_status(value: str) -> str:\n    return f"LOWER(TRIM({value}))"\n'
    ),
    "sql/sources/raw_orders.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n"
        "    table: raw_orders\n"
        "    columns:\n"
        "      - name: order_id\n        type: INTEGER\n"
        "      - name: status\n        type: VARCHAR\n"
    ),
    "sql/models/orders.sql": (
        "MODEL (description 'Test model orders.');\n\n"
        'SELECT order_id, @py.normalize_status("status") AS status FROM __source("raw_orders")\n'
    ),
}
_SOURCE_MOCK: str = (
    "__source__raw_orders AS (\n"
    "  SELECT 1 AS order_id, ' PAID ' AS status\n"
    "  UNION ALL SELECT 2 AS order_id, 'void' AS status\n"
    "),\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        SetOperationExpectedE2ETestCase(
            description="model and macro tests pass with intersect and except",
            test_files={
                "tests/unit/test_orders_except.sql": (
                    'TEST (name "orders_except");\n\nWITH\n'
                    + _SOURCE_MOCK
                    + "__expected__orders AS (\n"
                    "  SELECT 1 AS order_id UNION ALL SELECT 2 AS order_id\n"
                    "  UNION ALL SELECT 3 AS order_id\n"
                    "  EXCEPT SELECT 3 AS order_id\n"
                    ")\nSELECT 1\n"
                ),
                "tests/unit/test_orders_intersect.sql": (
                    'TEST (name "orders_intersect");\n\nWITH\n'
                    + _SOURCE_MOCK
                    + "__expected__orders AS (\n"
                    "  SELECT order_id FROM (VALUES (1), (2), (3)) AS candidates(order_id)\n"
                    "  INTERSECT ALL\n"
                    "  SELECT order_id FROM (VALUES (1), (2)) AS loaded(order_id)\n"
                    ")\nSELECT 1\n"
                ),
                "tests/unit/test_normalize_status.sql": (
                    'TEST (mode macro, name "normalizes_status");\n\nWITH\n'
                    "input_values AS (SELECT '  PAID  ' AS raw_status),\n"
                    "__macro_actual__ AS (\n"
                    '  SELECT @py.normalize_status("raw_status") AS status FROM input_values\n'
                    "),\n"
                    "__macro_expected__ AS (\n"
                    "  SELECT 'paid' AS status\n"
                    "  INTERSECT DISTINCT SELECT 'paid' AS status\n"
                    "  EXCEPT SELECT 'void' AS status\n"
                    ")\nSELECT 1\n"
                ),
            },
            expected_exit_code=0,
            expected_output_fragments=("PASS=3", "FAIL=0"),
        ),
        SetOperationExpectedE2ETestCase(
            description="model test with a mismatched intersect branch fails clearly",
            test_files={
                "tests/unit/test_orders_mismatch.sql": (
                    'TEST (name "orders_mismatch");\n\nWITH\n'
                    + _SOURCE_MOCK
                    + "__expected__orders AS (\n"
                    "  SELECT 1 AS order_id INTERSECT SELECT 1 AS other_id\n"
                    ")\nSELECT 1\n"
                ),
            },
            expected_exit_code=1,
            expected_output_fragments=(
                "SQL test 'tests/unit/test_orders_mismatch.sql' must use the same "
                "__expected__orders projection names and order in every set-operation "
                "branch; branch 2 does not match branch 1",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_set_operation_expected_ctes_when_testing_then_every_branch_is_checked(
    test_case: SetOperationExpectedE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="set_operation_expected",
        repo_files={**_PROJECT_FILES, **test_case.test_files},
    )
    execute_duckdb(
        db_path=project_dir / "set_operation_expected.duckdb",
        sql="CREATE TABLE raw_orders AS SELECT 1 AS order_id, 'paid' AS status",
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )

    output: str = tested.stdout + tested.stderr
    assert tested.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
