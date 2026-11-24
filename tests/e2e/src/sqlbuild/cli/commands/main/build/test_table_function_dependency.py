"""E2E coverage for managed table functions consumed by models."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    TableFunctionDependencyBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        TableFunctionDependencyBuildE2ETestCase(
            description="selected model creates and consumes managed table function",
            function_argument=7,
            expected_rows=((7, 70),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_model_table_function_dependency_when_building_then_creates_and_consumes_function(
    test_case: TableFunctionDependencyBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="table_function_dependency",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "table_function_dependency"
                adapter = "duckdb"

                [connection]
                database = "warehouse.duckdb"
                """
            ).strip()
            + "\n",
            "sql/functions/sql/customer_orders.sql": dedent(
                """
                FUNCTION (description "Test function customer_orders.",
                  arguments (customer_id INTEGER),
                  returns table (
                    customer_id INTEGER,
                    order_id INTEGER
                  )
                );

                SELECT customer_id AS customer_id, customer_id * 10 AS order_id
                """
            ).strip()
            + "\n",
            "sql/models/customer_order_summary.sql": dedent(
                f"""
                MODEL (description "Test model customer_order_summary.",
                  materialized table
                );

                SELECT customer_id, order_id
                FROM __table_fn("customer_orders")(
                  /* customer key, not another argument */ {test_case.function_argument}
                )
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "customer_order_summary"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert query_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql="SELECT customer_id, order_id FROM main.customer_order_summary",
    ) == list(test_case.expected_rows)
