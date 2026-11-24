"""E2E tests for audit failure behavior through the CLI."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    AuditFailureBuildE2ETestCase,
    MeasurementAuditCliE2ETestCase,
    StandaloneAuditRowE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import seed_audit_files
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        AuditFailureBuildE2ETestCase(
            description="delta audit failure blocks incremental target update",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
            name = "audit_failure_project"
            adapter = "duckdb"

            [connection]
            database = "audit_failures.duckdb"

            [defaults]
            materialized = "table"
                """
                ).strip()
                + "\n",
                "seed_raw_data.sql": dedent(
                    """
                CREATE TABLE IF NOT EXISTS raw_orders (
                  id INTEGER,
                  ordered_at TIMESTAMP,
                  amount_cents INTEGER
                );

                INSERT INTO raw_orders VALUES
                  (1, '2026-01-01 00:30:00', 100),
                  (2, '2026-01-01 01:30:00', 200);

                CREATE TABLE IF NOT EXISTS orders (
                  id INTEGER,
                  ordered_at TIMESTAMP,
                  amount_cents INTEGER
                );

                INSERT INTO orders VALUES (1, '2025-12-31 00:30:00', 50);
                """
                ).strip()
                + "\n",
                "sql/models/orders.sql": dedent(
                    """
                MODEL (description "Test model orders.",
                  materialized incremental,
                  incremental_strategy merge,
                  unique_key [id],
                  audits [
                    expression_is_true (
                      name "amount_is_negative",
                      expression "amount_cents < 0",
                      severity error,
                      run_scope delta_and_final,
                    ),
                  ],
                );

                SELECT id, ordered_at, amount_cents FROM main.raw_orders
                """
                ).strip()
                + "\n",
                "sql/audits/generic/expression_is_true.sql": dedent(
                    """
                AUDIT ();

                SELECT * FROM __ref("@model") WHERE NOT (@expression)
                """
                ).strip()
                + "\n",
            },
            command=("--no-color", "build", "--select", "orders"),
            expected_exit_code=1,
            expected_failure_fragment=(
                "delta audit for 'orders' failed before target update with severity level: error"
            ),
            expected_retained_relation_fragment="delta table kept for inspection: main.orders__delta",
        ),
        AuditFailureBuildE2ETestCase(
            description="final audit failure reports target already updated for incremental model",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
            name = "audit_failure_project"
            adapter = "duckdb"

            [connection]
            database = "audit_failures.duckdb"

            [defaults]
            materialized = "table"
                """
                ).strip()
                + "\n",
                "seed_raw_data.sql": dedent(
                    """
                CREATE TABLE IF NOT EXISTS raw_orders (
                  id INTEGER,
                  ordered_at TIMESTAMP,
                  amount_cents INTEGER
                );

                INSERT INTO raw_orders VALUES
                  (1, '2026-01-01 00:30:00', 100),
                  (2, '2026-01-01 01:30:00', 200);

                CREATE TABLE IF NOT EXISTS orders (
                  id INTEGER,
                  ordered_at TIMESTAMP,
                  amount_cents INTEGER
                );

                INSERT INTO orders VALUES (1, '2025-12-31 00:30:00', 50);
                """
                ).strip()
                + "\n",
                "sql/models/orders.sql": dedent(
                    """
                MODEL (description "Test model orders.",
                  materialized incremental,
                  incremental_strategy merge,
                  unique_key [id],
                  audits [
                    expression_is_true (
                      name "amount_is_negative",
                      expression "amount_cents < 0",
                      severity error,
                      run_scope final,
                    ),
                  ],
                );

                SELECT id, ordered_at, amount_cents FROM main.raw_orders
                """
                ).strip()
                + "\n",
                "sql/audits/generic/expression_is_true.sql": dedent(
                    """
                AUDIT ();

                SELECT * FROM __ref("@model") WHERE NOT (@expression)
                """
                ).strip()
                + "\n",
            },
            command=("--no-color", "build", "--select", "orders"),
            expected_exit_code=1,
            expected_failure_fragment=(
                "final audit for 'orders' failed after target update with severity level: error"
            ),
            expected_retained_relation_fragment="delta table kept for inspection: main.orders__delta",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_audit_failure_projects_when_running_build_then_cli_reports_failure_phase(
    test_case: AuditFailureBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="audit_failure_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "audit_failures.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute((project_dir / "seed_raw_data.sql").read_text(encoding="utf-8"))
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_failure_fragment in result.stdout
    assert test_case.expected_retained_relation_fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        MeasurementAuditCliE2ETestCase(
            description="failing model measurement audit",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "measurement_build"\nadapter = "duckdb"\n'
                    '[connection]\ndatabase = "measurement.duckdb"\n'
                ),
                "sql/models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table, audits [row_rate ("
                    "thresholds (error (below 90)))]) ; SELECT 1 AS order_id"
                ),
                "sql/audits/generic/row_rate.sql": (
                    "AUDIT (evaluation measurement, value rate); "
                    "MEASURE (SELECT 80.0 AS rate FROM @relation LIMIT 1);"
                ),
            },
            command=("--no-color", "build", "--select", "orders"),
            expected_exit_code=1,
            expected_output="row_rate",
        ),
        MeasurementAuditCliE2ETestCase(
            description="failing standalone measurement audit",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "measurement_audit"\nadapter = "duckdb"\n'
                    '[connection]\ndatabase = "measurement.duckdb"\n'
                ),
                "sql/models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table); SELECT 1 AS order_id"
                ),
                "sql/models/customers.sql": (
                    "MODEL (description 'Test model customers.', materialized table); SELECT 1 AS order_id"
                ),
                "sql/audits/singular/rate.sql": (
                    "AUDIT (evaluation measurement, value rate, "
                    "thresholds (error (below 90))); "
                    'MEASURE (SELECT 80.0 AS rate FROM __ref("orders") '
                    'JOIN __ref("customers") USING (order_id) LIMIT 1);'
                ),
            },
            command=("--no-color", "audit"),
            expected_exit_code=1,
            expected_output="rate",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_measurement_audit_when_running_cli_then_outcome_controls_exit_and_is_visible(
    test_case: MeasurementAuditCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="measurement_project",
        repo_files=test_case.repo_files,
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )
    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_output in result.stdout
    assert "FAIL" in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        StandaloneAuditRowE2ETestCase(
            description="passing seed audits name their column and show PASS",
            status_values=("placed", "shipped"),
            expected_exit_code=0,
            expected_rows=(
                "  audit     not_null (order_statuses.status_id) PASS",
                "  audit     accepted_values (order_statuses.status) PASS",
            ),
        ),
        StandaloneAuditRowE2ETestCase(
            description="a failing warn seed audit shows WARN",
            status_values=("placed",),
            expected_exit_code=0,
            expected_rows=(
                "  audit     not_null (order_statuses.status_id) PASS",
                "  audit     accepted_values (order_statuses.status) WARN",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_seed_audits_when_building_then_rows_show_target_and_outcome(
    test_case: StandaloneAuditRowE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="seed_audit_rows",
        repo_files=seed_audit_files(test_case.status_values),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, test_case.description + output
    assert all(row in result.stdout for row in test_case.expected_rows), (
        test_case.description + output
    )
