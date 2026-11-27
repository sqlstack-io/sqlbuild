"""E2E tests for core lifecycle CLI command coverage."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    LifecycleCommandsBuildE2ETestCase,
    WarehouseConnectionFailureE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    assert_fragments_in_order,
    prepare_inline_project,
    prepare_waffle_shop,
    run_sqb,
)

_UNREACHABLE_WAREHOUSE_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "unreachable_warehouse"\n'
        'adapter = "duckdb"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        'database = "missing_directory/nested/orders.duckdb"\n\n'
        "[targets.dev]\n"
        'schema = "dev"\n'
    ),
    "sql/seeds/order_statuses.csv": "status_id,status\n1,placed\n",
    "sql/seeds/order_statuses.yml": (
        "seeds:\n"
        "  - name: order_statuses\n"
        "    description: Order statuses.\n"
        "    columns:\n"
        "      - name: status_id\n"
        "        type: INTEGER\n"
        "        audits: [not_null]\n"
        "      - name: status\n"
        "        type: VARCHAR\n"
    ),
    "sql/models/statuses.sql": (
        "MODEL (description 'Statuses.', materialized table, "
        "columns (status_id (audits [not_null])));\n\n"
        'SELECT status_id FROM __seed("order_statuses")\n'
    ),
    "tests/unit/test_statuses.sql": (
        "TEST();\n\n"
        "WITH __seed__order_statuses AS (SELECT 1 AS status_id, 'placed' AS status),\n"
        "__expected__statuses AS (SELECT 1 AS status_id)\n"
        "SELECT 1\n"
    ),
}
_FORBIDDEN_ADAPTER_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _UNREACHABLE_WAREHOUSE_FILES["sqlbuild_project.toml"]
    .replace('adapter = "duckdb"', 'adapter = "duckdb_forbidden"')
    .replace("missing_directory/nested/orders.duckdb", "orders.duckdb"),
    "python/adapters/duckdb_forbidden.py": (
        "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
        "class Forbidden(Exception):\n"
        "    code = 403\n\n\n"
        "class DuckDbForbiddenAdapter(DuckDbAdapter):\n"
        '    adapter_name = "duckdb_forbidden"\n\n'
        "    def connect(self, config):\n"
        '        raise Forbidden("403 access denied for the orders project")\n'
    ),
}
_CONNECTION_FAILURE: tuple[str, ...] = (
    "error[C115]: could not connect to the duckdb warehouse",
    "`sqb debug` tests the connection",
)


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleCommandsBuildE2ETestCase(
            description="waffle shop core lifecycle commands remain consistent",
            expected_exit_code=0,
            expected_fresh_plan_fragments=(
                "Plan ready  17 selected, 2 sources to load",
                "Sources to load (2)",
                "raw_customers",
                "raw_orders",
                "delete_insert (cursor: ordered_at)",
                "Changed functions (3)",
                "customer_orders",
                "table function",
                "First run (13)",
            ),
            expected_test_fragment="PASS=5  FAIL=0  TOTAL=5",
            expected_audit_fragment="PASS=28  WARN=0  FAIL=0  INSUFFICIENT=0  TOTAL=28",
            expected_run_fragment="\u2713 Completed successfully",
            expected_rerun_reasons={
                "stg_orders": "no_change",
                "hourly_order_activity": "normal_incremental",
            },
            expected_full_refresh_fragment=(
                "Plan ready  full refresh, 17 selected, 2 sources to load"
            ),
            expected_plan_ordered_fragments=(
                "Connecting to duckdb",
                "\u2713 Warehouse connected  duckdb  (<time>)",
                "Inspecting warehouse state...",
                "✓ Plan generated  (<time>)",
                "Plan ready  17 selected, 2 sources to load",
                "Sources to load (2)",
                "Changed functions (3)",
                "First run (13)",
                "Seeds (1)",
            ),
            expected_build_ordered_fragments=(
                "Connecting to duckdb",
                "\u2713 Warehouse connected  duckdb  (<time>)",
                "Inspecting warehouse state...",
                "✓ Plan generated  (<time>)",
                "Plan ready  17 selected, 2 sources to load",
                "Sources to load (2)",
                "Execution  sqb build  (concurrency:",
                "source    raw_customers",
                "source    raw_orders",
                "Connecting to duckdb",
                "\u2713 Warehouse connected  duckdb  (<time>)",
                "udf       is_completed_order",
                "\u2713 Completed successfully",
                "PASS=<n>  WARN=<n>  FAIL=<n>  INSUFFICIENT=<n>  SKIP=<n>  TOTAL=<n>  (<time>)",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_running_core_lifecycle_commands_then_outputs_are_consistent(
    test_case: LifecycleCommandsBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)

    fresh_plan: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan"),
        project_dir=project_dir,
    )
    assert fresh_plan.returncode == test_case.expected_exit_code, (
        fresh_plan.stdout + fresh_plan.stderr
    )
    fragment: str
    for fragment in test_case.expected_fresh_plan_fragments:
        assert fragment in fresh_plan.stdout
    assert_fragments_in_order(fresh_plan.stdout, test_case.expected_plan_ordered_fragments)

    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )
    assert build_result.returncode == test_case.expected_exit_code, (
        build_result.stdout + build_result.stderr
    )
    assert "PASS=" in build_result.stdout
    assert_fragments_in_order(build_result.stdout, test_case.expected_build_ordered_fragments)

    test_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"),
        project_dir=project_dir,
    )
    assert test_result.returncode == test_case.expected_exit_code, (
        test_result.stdout + test_result.stderr
    )
    assert test_case.expected_test_fragment in test_result.stdout

    audit_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "audit"),
        project_dir=project_dir,
    )
    assert audit_result.returncode == test_case.expected_exit_code, (
        audit_result.stdout + audit_result.stderr
    )
    assert test_case.expected_audit_fragment in audit_result.stdout

    run_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits"),
        project_dir=project_dir,
    )
    assert run_result.returncode == test_case.expected_exit_code, (
        run_result.stdout + run_result.stderr
    )
    assert test_case.expected_run_fragment in run_result.stdout

    rerun_plan_json: subprocess.CompletedProcess[str] = run_sqb(
        command=("plan", "--json"),
        project_dir=project_dir,
    )
    assert rerun_plan_json.returncode == test_case.expected_exit_code, (
        rerun_plan_json.stdout + rerun_plan_json.stderr
    )
    payload: dict[str, object] = json.loads(rerun_plan_json.stdout)
    reasons_by_name: dict[str, str] = {
        str(entry["name"]): str(entry["reason"]) for entry in payload["models"]
    }
    assert reasons_by_name["stg_orders"] == test_case.expected_rerun_reasons["stg_orders"]
    assert (
        reasons_by_name["hourly_order_activity"]
        == test_case.expected_rerun_reasons["hourly_order_activity"]
    )

    rerun_build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )
    assert rerun_build.returncode == test_case.expected_exit_code, (
        rerun_build.stdout + rerun_build.stderr
    )
    assert test_case.expected_run_fragment in rerun_build.stdout

    full_refresh_build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--full-refresh"),
        project_dir=project_dir,
    )
    assert full_refresh_build.returncode == test_case.expected_exit_code, (
        full_refresh_build.stdout + full_refresh_build.stderr
    )
    assert test_case.expected_full_refresh_fragment in full_refresh_build.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        WarehouseConnectionFailureE2ETestCase(
            description=f"{' '.join(command)} reports an unreachable warehouse as a coded error",
            command=command,
            expected_exit_code=1,
            expected_stderr_fragments=_CONNECTION_FAILURE,
        )
        for command in (
            ("build",),
            ("seed",),
            ("audit",),
            ("plan",),
            ("test", "--run-on", "warehouse"),
            ("query", "SELECT 1"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unreachable_warehouse_when_running_command_then_reports_coded_error(
    test_case: WarehouseConnectionFailureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="unreachable_warehouse",
        repo_files=_UNREACHABLE_WAREHOUSE_FILES,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, test_case.description + output
    assert "Traceback" not in output, test_case.description + output
    assert all(fragment in result.stderr for fragment in test_case.expected_stderr_fragments), (
        test_case.description + output
    )


@pytest.mark.parametrize(
    "test_case",
    [
        WarehouseConnectionFailureE2ETestCase(
            description="a client error carrying its own code is still a coded connection error",
            command=("build",),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[C115]: could not connect to the duckdb_forbidden warehouse: "
                "403 access denied for the orders project",
            ),
            extra_files=_FORBIDDEN_ADAPTER_FILES,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_client_error_with_own_code_when_connecting_then_reports_coded_error(
    test_case: WarehouseConnectionFailureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="unreachable_warehouse",
        repo_files={**_UNREACHABLE_WAREHOUSE_FILES, **test_case.extra_files},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, test_case.description + output
    assert "Traceback" not in output, test_case.description + output
    assert all(fragment in result.stderr for fragment in test_case.expected_stderr_fragments), (
        test_case.description + output
    )
