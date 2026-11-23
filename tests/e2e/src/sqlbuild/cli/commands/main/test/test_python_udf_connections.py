"""E2E coverage that every `sqb test` connection can call the project's DuckDB Python UDFs."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    PythonUdfConnectionsE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_UDF: str = (
    "from sqlbuild.functions import udf\n\n\n"
    '@udf(arguments={"order_status": "STRING"}, returns="BOOLEAN", runtime_version="3.11")\n'
    "def main(order_status: str | None) -> bool:\n"
    '    """Test function main."""\n'
    '    return order_status == "completed"\n'
)
_TEST: str = (
    "TEST ();\nWITH\n"
    "__ref__order_statuses AS (SELECT 'completed' AS order_status),\n"
    "__expected__orders AS (SELECT 'completed' AS order_status, TRUE AS is_completed)\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonUdfConnectionsE2ETestCase(
            description="in-memory databases get the UDF on every connection",
            database=":memory:",
            concurrency=4,
            expected_stdout_fragment="PASS=8  FAIL=0  TOTAL=8",
        ),
        PythonUdfConnectionsE2ETestCase(
            description="connections to one database file share the UDF",
            database="orders.duckdb",
            concurrency=4,
            expected_stdout_fragment="PASS=8  FAIL=0  TOTAL=8",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_udf_when_testing_on_several_connections_then_every_test_can_call_it(
    test_case: PythonUdfConnectionsE2ETestCase, tmp_path: Path
) -> None:
    tests: dict[str, str] = {f"tests/unit/test_orders_{index}.sql": _TEST for index in range(1, 9)}
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_python_udf",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "orders_python_udf"\nadapter = "duckdb"\n\n'
                f'[connection]\ndatabase = "{test_case.database}"\n\n'
                "[scopes]\nenforce_placement = false\n"
            ),
            "sql/functions/python/is_completed_order.py": _UDF,
            "sql/models/order_statuses.sql": (
                "MODEL (description 'Test model order_statuses.');\n"
                "SELECT 'completed' AS order_status\n"
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.');\n"
                'SELECT s.order_status, __udf("is_completed_order")(s.order_status) AS is_completed\n'
                'FROM __ref("order_statuses") AS s\n'
            ),
            **tests,
        },
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--concurrency", str(test_case.concurrency)),
        project_dir=project_dir,
    )

    assert tested.returncode == 0, tested.stdout + tested.stderr
    assert test_case.expected_stdout_fragment in tested.stdout, test_case.description
