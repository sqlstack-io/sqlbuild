"""E2E coverage of what a failed SQLBuild command reports through Dagster."""

import json
import logging
import subprocess
from collections.abc import Iterator
from pathlib import Path

import duckdb
import pytest
from dagster import (
    AssetExecutionContext,
    DagsterInstance,
    EventLogEntry,
    ExecuteInProcessResult,
    materialize,
)
from dagster._core.execution.plan.objects import UserFailureData

from sqlbuild.integrations.dagster import SqlBuildCliResource, SqlBuildProject, sqlbuild_assets
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    REPO_ROOT,
    prepare_inline_project,
    run_sqb,
)
from tests.e2e.src.sqlbuild.integrations.dagster._test_types import (
    DagsterFailureOutputE2ETestCase,
)

_FAILING_AUDIT_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders_shop"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "warehouse.duckdb"\n\n[defaults]\nmaterialized = "table"\n'
    ),
    "sql/models/customers.sql": (
        "MODEL (description 'One row per customer.');\nSELECT 1 AS customer_id\n"
    ),
    "sql/models/orders.sql": (
        "MODEL (\n  description 'One row per order.',\n"
        "  columns (order_id (audits [not_null])),\n);\n"
        'SELECT 1 AS order_id, customer_id FROM __ref("customers")\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterFailureOutputE2ETestCase(
            description="a failing audit leads the failure and is logged as an error",
            null_order_rows=2,
            expected_description_start=(
                "Failed: 1 check\n  audit not_null on orders.order_id: 2 failing rows\n\n"
                "SQLBuild CLI command failed with exit code 1: "
            ),
            expected_failed_checks=[
                {
                    "kind": "audit",
                    "name": "not_null",
                    "asset": "orders",
                    "column": "order_id",
                    "status": "error",
                    "failing_rows": 2,
                }
            ],
            expected_error_log="SQLBuild check audit__not_null__order_id on main/orders failed: "
            "2 failing rows",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_failing_audit_when_auditing_through_dagster_then_the_failure_names_it(
    test_case: DagsterFailureOutputE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders_shop", repo_files=_FAILING_AUDIT_PROJECT_FILES
    )
    sqb: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    built: subprocess.CompletedProcess[str] = run_sqb(command=("build",), project_dir=project_dir)
    assert built.returncode == 0, built.stderr
    with duckdb.connect(str(project_dir / "warehouse.duckdb")) as connection:
        for _ in range(test_case.null_order_rows):
            _ = connection.execute("INSERT INTO main.orders VALUES (NULL, 1)")
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir, sqb_command=(str(sqb),)
    )
    sqlbuild_project.prepare()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def orders_job_assets(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["audit"], context=context).stream()

    instance: DagsterInstance = DagsterInstance.ephemeral()
    result: ExecuteInProcessResult = materialize(
        [orders_job_assets],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
        instance=instance,
        raise_on_error=False,
    )
    failure: UserFailureData | None = result.get_step_failure_events()[
        0
    ].step_failure_data.user_failure_data
    assert failure is not None
    description: str = failure.description or ""
    metadata: dict[str, object] = {key: value.value for key, value in failure.metadata.items()}
    logs: list[EventLogEntry] = list(instance.all_logs(result.run_id))
    logged: list[tuple[str | int, str]] = [(entry.level, entry.user_message) for entry in logs]
    recalled: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "recall",
            "query",
            "--json",
            "SELECT origin, integration FROM invocations WHERE command = 'audit'",
        ),
        project_dir=project_dir,
    )

    assert result.success is False
    assert description.startswith(test_case.expected_description_start)
    assert metadata["failed_checks"] == test_case.expected_failed_checks
    assert metadata["failed_check_count"] == len(test_case.expected_failed_checks)
    assert "selection" not in metadata
    assert isinstance(metadata["selection_count"], int)
    assert any(
        level == logging.ERROR and test_case.expected_error_log in message
        for level, message in logged
    )
    assert json.loads(recalled.stdout)["rows"] == [["integration", "dagster"]]
