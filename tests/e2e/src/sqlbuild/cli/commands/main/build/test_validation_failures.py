"""E2E tests for CLI validation failure behavior."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    CliFailureBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        CliFailureBuildE2ETestCase(
            description="invalid typed pre hook fails at compile time",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
            name = "hook_validation_project"
            adapter = "duckdb"

            [connection]
            database = "validation.duckdb"
                """
                ).strip()
                + "\n",
                "seed_raw_data.sql": "",
                "models/orders.sql": dedent(
                    """
                MODEL (description "Test model orders.",
                  materialized table,
                  pre_hooks [inline_sql('THIS IS NOT VALID SQL')]
                );

                SELECT 1 AS id
                """
                ).strip()
                + "\n",
            },
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[P001]: SQL syntax error in model 'orders' pre_hooks[0] inline_sql(\"...\")",
                "the duckdb parser cannot read Syntax error at or near 'THIS'",
                "  = help: if this SQL is valid for your warehouse, skip SQL analysis for this "
                "model, add this to the MODEL header:\n"
                "            MODEL (\n"
                "              sql_analysis false,",
            ),
        ),
        CliFailureBuildE2ETestCase(
            description="unknown cursor_inputs key fails at compile time",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
            name = "cursor_inputs_validation_project"
            adapter = "duckdb"

            [connection]
            database = "validation.duckdb"

            [defaults]
            materialized = "table"
                """
                ).strip()
                + "\n",
                "seed_raw_data.sql": dedent(
                    """
                CREATE TABLE IF NOT EXISTS raw_orders (
                  id INTEGER,
                  ordered_at TIMESTAMP
                );

                INSERT INTO raw_orders VALUES
                  (1, '2026-01-01 00:30:00');
                """
                ).strip()
                + "\n",
                "sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_orders
                    description: Test source raw_orders.
                    schema: main
                    table: raw_orders
                """
                ).strip()
                + "\n",
                "models/staging/stg_orders.sql": dedent(
                    """
                MODEL (description "Test model stg_orders.", materialized view);

                SELECT
                  id AS order_id,
                  ordered_at
                FROM __source("raw_orders")
                """
                ).strip()
                + "\n",
                "models/marts/fact_orders.sql": dedent(
                    """
                MODEL (description "Test model fact_orders.", materialized table);

                SELECT order_id, ordered_at FROM __ref("stg_orders")
                """
                ).strip()
                + "\n",
                "models/marts/customer_status_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_status_snapshot.",
                  materialized incremental,
                  incremental_strategy merge,
                  unique_key [order_id],
                  cursor ordered_at,
                  cursor_type timestamp,
                  cursor_grain second,
                  cursor_inputs (
                    missing_relation ordered_at,
                  ),
                );

                SELECT order_id, ordered_at FROM __ref("fact_orders")
                """
                ).strip()
                + "\n",
            },
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "cursor_inputs references unknown input 'missing_relation'",
                "expected one of: fact_orders",
            ),
        ),
        CliFailureBuildE2ETestCase(
            description="invalid source expression fails with sql validation enabled",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
            name = "source_expression_validation_project"
            adapter = "duckdb"

            [connection]
            database = "validation.duckdb"
                """
                ).strip()
                + "\n",
                "seed_raw_data.sql": "",
                "sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_orders
                    description: Test source raw_orders.
                    expression: SELECT FROM
                """
                ).strip()
                + "\n",
                "models/orders.sql": dedent(
                    """
                MODEL (description "Test model orders.", materialized table);

                SELECT * FROM __source("raw_orders")
                """
                ).strip()
                + "\n",
            },
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "SQL syntax error in source expression 'raw_orders'",
                "set this in sqlbuild_project.toml:\n            [settings]\n"
                "            sql_analysis = false",
                "  = help: to skip SQL analysis for one run, use `--no-sql-analysis`",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_cli_project_when_running_build_then_it_fails_clearly(
    test_case: CliFailureBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="validation_project",
        repo_files=test_case.repo_files,
    )
    seed_file: Path = project_dir / "seed_raw_data.sql"
    import duckdb

    db_path: Path = project_dir / "validation.duckdb"
    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(seed_file.read_text(encoding="utf-8"))
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    fragment: str
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stdout + result.stderr
