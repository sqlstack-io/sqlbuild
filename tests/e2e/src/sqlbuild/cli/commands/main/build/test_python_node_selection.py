"""Real-CLI coverage of Python node selection against unselected and selected SQL upstreams."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    PythonNodeSelectionE2ETestCase,
    PythonUpstreamResultE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    PYTHON_NODE_SELECTION_DATABASE,
    python_node_selection_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
    table_exists,
)

_FAILING_ORDERS_SQL: str = (
    "MODEL (description 'Test model.', materialized table);\n"
    "SELECT order_id FROM __source(\"raw_orders\") WHERE error('orders failed') IS NULL\n"
)
_EXISTING_ORDERS: str = "CREATE TABLE main.orders AS SELECT * FROM range(1, 6) AS t(order_id);"
_EXISTING_ROWS: str = (
    "CREATE TABLE main.raw_orders AS SELECT * FROM range(1, 3) AS t(order_id);"
    "CREATE TABLE main.raw_customers AS SELECT * FROM range(1, 4) AS t(customer_id);"
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeSelectionE2ETestCase(
            description="task selected alone reads the existing managed source table",
            select=("task:count_customers",),
            expected_exit_code=0,
            expected_output_patterns=(r"count_customers\s+OK",),
            expected_row_counts={"raw_customers": 3},
            expected_node_counts={"count_customers": 3},
        ),
        PythonNodeSelectionE2ETestCase(
            description="asset selected alone reads the existing managed source table",
            select=("asset:customers_extract",),
            expected_exit_code=0,
            expected_output_patterns=(r"customers_extract\s+OK",),
            expected_row_counts={"raw_customers": 3},
            expected_node_counts={"customers_extract": 3},
        ),
        PythonNodeSelectionE2ETestCase(
            description="task on an unmanaged source runs when selected alone",
            select=("task:count_raw_orders",),
            expected_exit_code=0,
            expected_output_patterns=(r"count_raw_orders\s+OK",),
            expected_row_counts={},
            expected_node_counts={"count_raw_orders": 2},
        ),
        PythonNodeSelectionE2ETestCase(
            description="task on an unmanaged source runs when selected with upstreams",
            select=("+task:count_raw_orders",),
            expected_exit_code=0,
            expected_output_patterns=(r"count_raw_orders\s+OK",),
            expected_row_counts={},
            expected_node_counts={"count_raw_orders": 2},
        ),
        PythonNodeSelectionE2ETestCase(
            description="task selected alone reads the existing unselected model",
            select=("task:count_orders",),
            existing_sql=_EXISTING_ORDERS,
            expected_exit_code=0,
            expected_output_patterns=(r"count_orders\s+OK",),
            expected_row_counts={"orders": 5},
            expected_node_counts={"count_orders": 5},
        ),
        PythonNodeSelectionE2ETestCase(
            description="asset selected alone reads the existing unselected model",
            select=("asset:orders_extract",),
            existing_sql=_EXISTING_ORDERS,
            expected_exit_code=0,
            expected_output_patterns=(r"orders_extract\s+OK",),
            expected_row_counts={"orders": 5},
            expected_node_counts={"orders_extract": 5},
        ),
        PythonNodeSelectionE2ETestCase(
            description="task waits for its selected upstream model",
            select=("+task:count_orders",),
            expected_exit_code=0,
            expected_output_patterns=(r"count_orders\s+OK",),
            expected_row_counts={"orders": 2},
            expected_node_counts={"count_orders": 2},
        ),
        PythonNodeSelectionE2ETestCase(
            description="asset waits for its selected upstream model",
            select=("+asset:orders_extract",),
            expected_exit_code=0,
            expected_output_patterns=(r"orders_extract\s+OK",),
            expected_row_counts={"orders": 2},
            expected_node_counts={"orders_extract": 2},
        ),
        PythonNodeSelectionE2ETestCase(
            description="task is skipped when its selected upstream model fails",
            select=("+task:count_orders",),
            orders_sql=_FAILING_ORDERS_SQL,
            expected_exit_code=1,
            expected_output_patterns=(
                r"count_orders\s+SKIP\s+Upstream SQL resource did not succeed: orders",
            ),
            expected_row_counts={},
            expected_node_counts={},
            expected_missing_tables=("count_orders_result",),
        ),
        PythonNodeSelectionE2ETestCase(
            description="asset is skipped when its selected upstream model fails",
            select=("+asset:orders_extract",),
            orders_sql=_FAILING_ORDERS_SQL,
            expected_exit_code=1,
            expected_output_patterns=(
                r"orders_extract\s+SKIP\s+Upstream SQL resource did not succeed: orders",
            ),
            expected_row_counts={},
            expected_node_counts={},
            expected_missing_tables=("orders_extract_result",),
        ),
        PythonNodeSelectionE2ETestCase(
            description="task is skipped when its selected upstream model is skipped",
            select=("+task:count_order_summary",),
            orders_sql=_FAILING_ORDERS_SQL,
            expected_exit_code=1,
            expected_output_patterns=(
                r"count_order_summary\s+SKIP\s+"
                r"Upstream SQL resource did not succeed: order_summary",
            ),
            expected_row_counts={},
            expected_node_counts={},
            expected_missing_tables=("count_order_summary_result",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_node_selection_when_building_then_it_gates_only_on_scheduled_upstreams(
    test_case: PythonNodeSelectionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="order_nodes",
        repo_files=python_node_selection_project_files(orders_sql=test_case.orders_sql),
    )
    database: Path = project_dir / PYTHON_NODE_SELECTION_DATABASE
    execute_duckdb(db_path=database, sql=_EXISTING_ROWS + test_case.existing_sql)

    build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", *test_case.select), project_dir=project_dir
    )

    output: str = build.stdout + build.stderr
    assert build.returncode == test_case.expected_exit_code, output
    assert all(re.search(pattern, output) for pattern in test_case.expected_output_patterns), output
    assert {
        table: query_duckdb(db_path=database, sql=f"SELECT count(*) FROM main.{table}")[0][0]
        for table in test_case.expected_row_counts
    } == test_case.expected_row_counts
    assert {
        node: query_duckdb(db_path=database, sql=f"SELECT n FROM main.{node}_result")[0][0]
        for node in test_case.expected_node_counts
    } == test_case.expected_node_counts
    assert not any(
        table_exists(db_path=database, table_name=table)
        for table in test_case.expected_missing_tables
    )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonUpstreamResultE2ETestCase(
            description="a task reads its unselected upstream task's latest result",
            select="task:report_order_count",
            upstream_select="task:count_orders",
            expected_error=(
                "Upstream task 'count_orders' has no successful result to read. Build it first "
                "with 'sqb build --select task:count_orders', or build both with "
                "'sqb build --select +task:report_order_count'."
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_task_selected_without_its_python_upstream_when_building_then_it_reads_the_persisted_result(
    test_case: PythonUpstreamResultE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="order_nodes",
        repo_files=python_node_selection_project_files(
            orders_sql=PythonNodeSelectionE2ETestCase.orders_sql
        ),
    )
    database: Path = project_dir / PYTHON_NODE_SELECTION_DATABASE
    execute_duckdb(db_path=database, sql=_EXISTING_ROWS + _EXISTING_ORDERS)
    report: tuple[str, ...] = ("--no-color", "build", "--select", test_case.select)

    never_built: subprocess.CompletedProcess[str] = run_sqb(command=report, project_dir=project_dir)
    upstream: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", test_case.upstream_select),
        project_dir=project_dir,
    )
    rebuilt: subprocess.CompletedProcess[str] = run_sqb(command=report, project_dir=project_dir)

    output: str = never_built.stdout + never_built.stderr
    assert never_built.returncode == 1, test_case.description + output
    assert test_case.expected_error in output, test_case.description
    assert upstream.returncode == 0, upstream.stdout + upstream.stderr
    assert rebuilt.returncode == 0, rebuilt.stdout + rebuilt.stderr
    node: str = test_case.select.split(":", 1)[1]
    assert query_duckdb(db_path=database, sql=f"SELECT status FROM main.{node}_result") == [
        ("success",)
    ]
