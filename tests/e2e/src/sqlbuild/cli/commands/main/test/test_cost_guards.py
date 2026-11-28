"""E2E coverage for unit-test cost guards: real-model limit, single-read comparison, time limits."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    ModelTimeoutE2ETestCase,
    ModelTimeoutJsonE2ETestCase,
    RealModelLimitE2ETestCase,
    ScenarioTimeoutE2ETestCase,
    SingleReadComparisonE2ETestCase,
    TransactionTimeoutE2ETestCase,
    UnitTestTimeoutE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import (
    COST_GUARD_PROJECT_TOML,
    COST_GUARD_RAW_ORDERS_SOURCE,
    SLOW_SCALAR_SQL,
    build_order_chain_files,
    slow_model_files,
    slow_promotion_files,
    stored_test_outputs,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
    table_exists,
)

_ORDER_FILES: dict[str, str] = {
    "sqlbuild_project.toml": COST_GUARD_PROJECT_TOML,
    "sql/models/stg_orders.sql": (
        "MODEL (description 'Staged orders.', materialized table);\n\n"
        "SELECT * FROM (VALUES (1, 100, 5)) AS t(order_id, customer_id, amount)\n"
    ),
    "sql/models/order_totals.sql": (
        "MODEL (description 'Order totals.', materialized table);\n\n"
        'SELECT order_id, customer_id, amount * 2 AS total FROM __ref("stg_orders")\n'
    ),
}
_SLOW_FILES: dict[str, str] = {
    "sqlbuild_project.toml": COST_GUARD_PROJECT_TOML,
    "sql/sources/raw_orders.yml": COST_GUARD_RAW_ORDERS_SOURCE,
    "sql/models/stg_orders.sql": (
        "MODEL (description 'Staged orders.', materialized table);\n\n"
        f'SELECT order_id, {SLOW_SCALAR_SQL} AS scanned FROM __source("raw_orders")\n'
    ),
    "sql/models/order_scans.sql": (
        "MODEL (description 'Order scans.', materialized table);\n\n"
        'SELECT order_id, scanned FROM __ref("stg_orders")\n'
    ),
    "tests/unit/order_ids_are_kept.sql": (
        "TEST();\n\nWITH\n__ref__stg_orders AS (\n  SELECT 1 AS order_id, 0 AS scanned\n),\n"
        "__expected__order_scans AS (\n  SELECT 1 AS order_id\n)\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        RealModelLimitE2ETestCase(
            description="default limit rejects an eleven-model chain at compile time",
            local_files={},
            expected_exit_code=1,
            expected_output_fragments=(
                "SQL unit test 'order_chain_totals' executes 11 real models (limit 10).",
                "Model under test: order_step_11",
                "order_step_1   order_step_11 -> order_step_10 -> order_step_9",
                "1. Mock the direct inputs of the model under test to cut the chain:",
                "__ref__order_step_10 AS (SELECT ...)",
                "2. If this test checks behaviour across several models, consider a scenario",
                "compiles and optimises the whole chain at once",
                "3. Raise the limit in sqlbuild_project.toml (or sqlbuild_local.toml):",
                "unit_test_max_real_models = 11",
            ),
        ),
        RealModelLimitE2ETestCase(
            description="local limit allows the chain and the row shows its real model count",
            local_files={"sqlbuild_local.toml": "[settings]\nunit_test_max_real_models = 11\n"},
            expected_exit_code=0,
            expected_output_fragments=("  11 real models", "PASS=1  FAIL=0  TOTAL=1"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_long_unmocked_chain_when_testing_then_real_model_limit_applies(
    tmp_path: Path, test_case: RealModelLimitE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={**build_order_chain_files(chain_length=11), **test_case.local_files},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )
    output: str = result.stdout + result.stderr

    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        SingleReadComparisonE2ETestCase(
            description="passing test",
            test_sql=(
                "TEST();\n\nWITH\n__ref__stg_orders AS (\n"
                "  SELECT 1 AS order_id, 100 AS customer_id, 5 AS amount\n),\n"
                "__expected__order_totals AS (\n"
                "  SELECT 1 AS order_id, 100 AS customer_id, 10 AS total\n)\n"
            ),
            expected_exit_code=0,
            expected_output_fragments=("PASS=1  FAIL=0  TOTAL=1", "  1 real model"),
        ),
        SingleReadComparisonE2ETestCase(
            description="column subset test",
            test_sql=(
                "TEST();\n\nWITH\n__ref__stg_orders AS (\n"
                "  SELECT 1 AS order_id, 100 AS customer_id, 5 AS amount\n),\n"
                "__expected__order_totals AS (\n  SELECT 1 AS order_id, 10 AS total\n)\n"
            ),
            expected_exit_code=0,
            expected_output_fragments=("PASS=1  FAIL=0  TOTAL=1",),
        ),
        SingleReadComparisonE2ETestCase(
            description="failing test keeps counts and samples",
            test_sql=(
                "TEST();\n\nWITH\n__ref__stg_orders AS (\n"
                "  SELECT 1 AS order_id, 100 AS customer_id, 5 AS amount\n),\n"
                "__expected__order_totals AS (\n  SELECT 1 AS order_id, 11 AS total\n)\n"
            ),
            expected_exit_code=1,
            expected_output_fragments=(
                "PASS=0  FAIL=1  TOTAL=1",
                "expected rows of order_totals differ: 1 missing, 1 extra (1 expected, 1 actual)",
                "total: expected 11, actual 10",
                "order_totals_doubles  (test, warehouse)",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unit_test_when_comparing_stored_output_then_results_and_samples_are_unchanged(
    tmp_path: Path, test_case: SingleReadComparisonE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={**_ORDER_FILES, "tests/unit/order_totals_doubles.sql": test_case.test_sql},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "--debug", "test", "--run-on", "warehouse"), project_dir=project_dir
    )
    output: str = result.stdout + result.stderr

    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    assert "phase=assert  kind=CREATE" in output, output
    assert output.index("kind=CREATE") < output.index("kind=WITH") < output.index("kind=DROP")
    assert stored_test_outputs(database_path=project_dir / "warehouse.duckdb") == []


@pytest.mark.parametrize(
    "test_case",
    [
        UnitTestTimeoutE2ETestCase(
            description="inlined slow model times out and suggests a scenario",
            test_sql=(
                'TEST(timeout "1s");\n\nWITH\n'
                "__source__raw_orders AS (\n  SELECT 1 AS order_id, 5 AS amount\n),\n"
                "__expected__order_scans AS (\n  SELECT 1 AS order_id\n)\n"
            ),
            command=("--no-color", "test"),
            expected_output_fragments=(
                "TIMEOUT",
                "SQL unit test 'order_scans_slow' timed out after",
                "(limit 1s).",
                "Limit set by:  TEST() timeout in tests/unit/order_scans_slow.sql",
                "Cancellation:  confirmed; the running statement stopped",
                "This test inlines 1 real model beyond the model under test",
                "__ref__stg_orders AS (SELECT ...)",
                "2. If this test checks behaviour across several models, consider a scenario",
                "3. To allow more time, set TEST(timeout",
                "PASS=1  FAIL=0  TIMEOUT=1  TOTAL=2",
            ),
            expected_absent_fragments=(),
        ),
        UnitTestTimeoutE2ETestCase(
            description="command line limit applies to the model under test alone",
            test_sql=(
                "TEST();\n\nWITH\n"
                "__source__raw_orders AS (\n  SELECT 1 AS order_id, 5 AS amount\n),\n"
                "__expected__stg_orders AS (\n  SELECT 1 AS order_id\n)\n"
            ),
            command=("--no-color", "test", "--test-timeout", "1s"),
            expected_output_fragments=(
                "SQL unit test 'order_scans_slow' timed out after",
                "Limit set by:  --test-timeout",
                'To allow more time, set TEST(timeout "',
                "PASS=1  FAIL=0  TIMEOUT=1  TOTAL=2",
            ),
            expected_absent_fragments=("consider a scenario",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_slow_unit_test_when_limit_expires_then_test_times_out_and_run_continues(
    tmp_path: Path, test_case: UnitTestTimeoutE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={**_SLOW_FILES, "tests/unit/order_scans_slow.sql": test_case.test_sql},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )
    output: str = result.stdout + result.stderr

    assert result.returncode == 1, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    for fragment in test_case.expected_absent_fragments:
        assert fragment not in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        ModelTimeoutE2ETestCase(
            description="header max_duration stops the slow model",
            max_duration="1s",
            expected_output_fragments=(
                "TIMEOUT",
                "[C417] Model 'order_scans' timed out after",
                "Limit set by:  max_duration in MODEL() header",
                "Cancellation:  confirmed; the running statement stopped",
                "Skipped (1): order_scan_summary",
                "FAIL=0  TIMEOUT=1  INSUFFICIENT=0  SKIP=1  TOTAL=3",
            ),
            expected_tables=(
                ("order_scans", False),
                ("order_scan_summary", False),
                ("customers", True),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_max_duration_when_building_then_slow_model_times_out_and_dependents_skip(
    tmp_path: Path, test_case: ModelTimeoutE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files=slow_model_files(max_duration=test_case.max_duration),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--concurrency", "2"), project_dir=project_dir
    )
    output: str = result.stdout + result.stderr

    assert result.returncode == 1, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    for table_name, expected_exists in test_case.expected_tables:
        assert (
            table_exists(db_path=project_dir / "warehouse.duckdb", table_name=table_name)
            == expected_exists
        ), table_name


@pytest.mark.parametrize(
    "test_case",
    [
        ModelTimeoutJsonE2ETestCase(
            description="build JSON counts the timeout within failures",
            expected_status="failed",
            expected_summary_counts={
                "success_count": 1,
                "failure_count": 1,
                "timeout_count": 1,
                "skipped_count": 1,
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_max_duration_when_building_as_json_then_summary_counts_timeout(
    tmp_path: Path, test_case: ModelTimeoutJsonE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="shop", repo_files=slow_model_files(max_duration="1s")
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--json"), project_dir=project_dir
    )
    payload: dict[str, Any] = json.loads(result.stdout)

    assert result.returncode == 1, result.stderr
    assert payload["status"] == test_case.expected_status
    for key, expected_count in test_case.expected_summary_counts.items():
        assert payload["summary"][key] == expected_count, payload["summary"]


@pytest.mark.parametrize(
    "test_case",
    [
        ModelTimeoutJsonE2ETestCase(
            description="test JSON counts the timeout within failures",
            expected_status="failed",
            expected_summary_counts={
                "pass_count": 1,
                "fail_count": 1,
                "timeout_count": 1,
                "total_count": 2,
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_slow_unit_test_when_testing_as_json_then_summary_counts_timeout(
    tmp_path: Path, test_case: ModelTimeoutJsonE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            **_SLOW_FILES,
            "tests/unit/order_scans_slow.sql": (
                'TEST(timeout "1s");\n\nWITH\n'
                "__source__raw_orders AS (\n  SELECT 1 AS order_id, 5 AS amount\n),\n"
                "__expected__order_scans AS (\n  SELECT 1 AS order_id\n)\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--json"), project_dir=project_dir
    )
    payload: dict[str, Any] = json.loads(result.stdout)

    assert result.returncode == 1, result.stderr
    assert payload["status"] == test_case.expected_status
    for key, expected_count in test_case.expected_summary_counts.items():
        assert payload["summary"][key] == expected_count, payload["summary"]


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioTimeoutE2ETestCase(
            description="SCENARIO() timeout stops the slow step",
            header_timeout="1s",
            command=("--no-color", "scenario", "test"),
            expected_output_fragments=(
                "[C417] Scenario 'order_scans_flow' timed out after",
                "Limit set by:  SCENARIO() timeout in tests/scenarios/order_scans_flow.sql",
                "Cancellation:  confirmed; the running statement stopped",
                "PASS=0  FAIL=1  TOTAL=1",
            ),
        ),
        ScenarioTimeoutE2ETestCase(
            description="command line limit overrides the header",
            header_timeout="10m",
            command=("--no-color", "scenario", "test", "--scenario-timeout", "1s"),
            expected_output_fragments=(
                "Scenario 'order_scans_flow' timed out after",
                "Limit set by:  --scenario-timeout",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_slow_scenario_when_limit_expires_then_scenario_times_out(
    tmp_path: Path, test_case: ScenarioTimeoutE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            **_SLOW_FILES,
            "tests/scenarios/order_scans_flow.sql": (
                'SCENARIO (\n  description "Order scans keep order ids",\n'
                f'  timeout "{test_case.header_timeout}"\n);\n\nWITH\n'
                "__source__raw_orders AS (\n  SELECT 1 AS order_id, 5 AS amount\n),\n"
                "__expected__order_scans AS (\n  SELECT 1 AS order_id\n)\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )
    output: str = result.stdout + result.stderr

    assert result.returncode == 1, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        TransactionTimeoutE2ETestCase(
            description="expiry inside the promotion swap keeps the published table",
            max_duration="2s",
            expected_output_fragments=(
                "[C417] Model 'order_totals' timed out after",
                "Limit set by:  max_duration in MODEL() header",
                "z_customers          OK",
                "PASS=1  WARN=0  FAIL=0  TIMEOUT=1",
            ),
            expected_published_rows=((1, 10),),
            expected_later_model_rows=((200,),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_limit_expiring_in_promotion_when_building_then_published_table_is_kept(
    tmp_path: Path, test_case: TransactionTimeoutE2ETestCase
) -> None:
    database: Path = tmp_path / "shop" / "warehouse.duckdb"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files=slow_promotion_files(
            database=database,
            max_duration=test_case.max_duration,
            order_total=10,
            customer_id=100,
        ),
    )
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--concurrency", "1"), project_dir=project_dir
    )
    assert first.returncode == 0, first.stdout + first.stderr
    changed_files: dict[str, str] = slow_promotion_files(
        database=database, max_duration=test_case.max_duration, order_total=20, customer_id=200
    )
    relative_path: str
    for relative_path in ("sql/models/order_totals.sql", "sql/models/z_customers.sql"):
        (project_dir / relative_path).write_text(changed_files[relative_path])

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--concurrency", "1"), project_dir=project_dir
    )
    output: str = result.stdout + result.stderr

    assert result.returncode == 1, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    assert (
        tuple(query_duckdb(db_path=database, sql="SELECT order_id, total FROM main.order_totals"))
        == test_case.expected_published_rows
    )
    assert (
        tuple(query_duckdb(db_path=database, sql="SELECT customer_id FROM main.z_customers"))
        == test_case.expected_later_model_rows
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
