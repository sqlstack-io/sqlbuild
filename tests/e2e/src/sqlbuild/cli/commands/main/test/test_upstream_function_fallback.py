"""Exercise SQL-test chains that contain a function call requiring textual resolution."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import SqlTestE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlTestE2ETestCase(
            description="fully mocked upstream function chain executes after textual fallback",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_upstream_function_when_compiling_and_testing_then_mocked_chain_executes(
    test_case: SqlTestE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_function_chain",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "orders_function_chain"\nadapter = "duckdb"\n'
                '[connection]\ndatabase = "orders.duckdb"\n'
            ),
            "sql/sources/raw.yml": (
                "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    schema: main\n    table: raw_orders\n"
            ),
            "sql/functions/sql/identity_value.sql": (
                "FUNCTION (description 'Test function identity_value.', arguments (value INTEGER), returns INTEGER);\nvalue\n"
            ),
            "sql/models/stg_orders.sql": (
                "MODEL (description 'Test model stg_orders.', materialized table);\n"
                'SELECT __udf("identity_value")(order_id) AS order_id '
                'FROM __source("raw_orders")\n'
            ),
            "sql/models/orders.sql": (
                'MODEL (description "Test model orders.", materialized table);\nSELECT * FROM __ref("stg_orders")\n'
            ),
            "tests/unit/orders_case.sql": (
                'TEST (name "orders_case");\nWITH\n'
                "__source__raw_orders AS (SELECT 1 AS order_id),\n"
                "__expected__orders AS (SELECT 1 AS order_id),\n"
                "__assert__positive AS ("
                'SELECT * FROM __ref("orders") WHERE order_id < 0)\nSELECT 1\n'
            ),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("compile",), project_dir=project_dir
    )
    execute_duckdb(
        db_path=project_dir / "orders.duckdb",
        sql="CREATE TABLE raw_orders AS SELECT 1 AS order_id",
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("build", "--no-tests", "--no-audits"), project_dir=project_dir
    )
    tested: subprocess.CompletedProcess[str] = run_sqb(command=("test",), project_dir=project_dir)

    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert built.returncode == 0, built.stdout + built.stderr
    assert tested.returncode == test_case.expected_exit_code, tested.stdout + tested.stderr
    assert test_case.expected_stdout_fragment in tested.stdout
    assert "has no mock" not in compiled.stdout + compiled.stderr + tested.stdout + tested.stderr
