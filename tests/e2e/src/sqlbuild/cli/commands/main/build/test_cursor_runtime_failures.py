"""E2E tests for direct cursor-related failure paths."""

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
            description="bad mapped cursor input column fails during compile validation",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "cursor_runtime_project"\n'
                    'adapter = "duckdb"\n\n'
                    "[connection]\n"
                    'database = "cursor_runtime.duckdb"\n'
                ),
                "seed_raw_data.sql": (
                    "CREATE TABLE main.raw_orders (id INTEGER, ordered_at TIMESTAMP);\n"
                    "INSERT INTO main.raw_orders VALUES (1, '2026-01-01 00:30:00');\n"
                    "CREATE TABLE main.customer_status_snapshot "
                    "(customer_id INTEGER, last_ordered_at TIMESTAMP);\n"
                ),
                "sql/models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized view);\n\n"
                    "SELECT id AS order_id, ordered_at FROM main.raw_orders\n"
                ),
                "sql/models/fact_orders.sql": (
                    "MODEL (description 'Test model fact_orders.',\n"
                    "  materialized table,\n"
                    "  contract enforced,\n"
                    "  columns (\n"
                    "    order_id (type INTEGER),\n"
                    "    ordered_at (type TIMESTAMP),\n"
                    "  ),\n"
                    ");\n\n"
                    'SELECT order_id, ordered_at FROM __ref("stg_orders")\n'
                ),
                "sql/models/customer_status_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_status_snapshot.",
                  materialized incremental,
                  incremental_strategy merge,
                  unique_key [customer_id],
                  cursor last_ordered_at,
                  cursor_type timestamp,
                  cursor_grain second,
                  cursor_inputs (
                    fact_orders missing_column,
                  ),
                );

                SELECT 1 AS customer_id, MAX(ordered_at) AS last_ordered_at
                FROM __ref("fact_orders")
                """
                ).strip()
                + "\n",
            },
            command=("--no-color", "build", "--select", "+customer_status_snapshot"),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[B300]: [B300] sql/models/customer_status_snapshot.sql:9:17: cursor_inputs "
                "fact_orders references unknown column 'missing_column'",
            ),
            verification_sql="SELECT COUNT(*) FROM main.customer_status_snapshot",
            expected_verification_rows=((0,),),
            pre_commands=(("--no-color", "build", "--select", "+fact_orders"),),
        ),
        CliFailureBuildE2ETestCase(
            description="broken runtime-owned upstream relation fails at compile time",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "cursor_runtime_project"\n'
                    'adapter = "duckdb"\n\n'
                    "[connection]\n"
                    'database = "cursor_runtime.duckdb"\n'
                ),
                "seed_raw_data.sql": (
                    "CREATE TABLE main.raw_orders (id INTEGER, ordered_at TIMESTAMP);\n"
                ),
                "sql/models/customer_status_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_status_snapshot.",
                  materialized incremental,
                  incremental_strategy merge,
                  unique_key [customer_id],
                  cursor last_ordered_at,
                  cursor_type timestamp,
                  cursor_grain second,
                  cursor_inputs (
                    missing_orders ordered_at,
                  ),
                );

                SELECT 1 AS customer_id, MAX(ordered_at) AS last_ordered_at
                FROM __ref("missing_orders")
                """
                ).strip()
                + "\n",
            },
            command=("--no-color", "build", "--select", "customer_status_snapshot"),
            expected_exit_code=1,
            expected_stderr_fragments=("references unknown model 'missing_orders'",),
            verification_sql=(
                "SELECT COUNT(*) FROM information_schema.tables "
                "WHERE table_name = 'customer_status_snapshot'"
            ),
            expected_verification_rows=((0,),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_runtime_failure_projects_when_running_build_then_cli_fails_clearly(
    test_case: CliFailureBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="cursor_runtime_project",
        repo_files=test_case.repo_files,
    )
    seed_file: Path = project_dir / "seed_raw_data.sql"
    import duckdb

    db_path: Path = project_dir / "cursor_runtime.duckdb"
    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(seed_file.read_text(encoding="utf-8"))
    connection.close()

    pre_command: tuple[str, ...]
    for pre_command in test_case.pre_commands:
        pre_result: subprocess.CompletedProcess[str] = run_sqb(
            command=pre_command,
            project_dir=project_dir,
        )
        assert pre_result.returncode == 0, pre_result.stdout + pre_result.stderr

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )
    assert result.returncode == test_case.expected_exit_code
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stdout + result.stderr
    assert test_case.verification_sql is not None
    verification_connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    verification_rows: list[tuple[object, ...]] = verification_connection.execute(
        test_case.verification_sql
    ).fetchall()
    verification_connection.close()
    assert tuple(verification_rows) == test_case.expected_verification_rows
