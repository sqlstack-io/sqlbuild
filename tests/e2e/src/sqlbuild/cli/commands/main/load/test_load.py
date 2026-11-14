from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.load._test_types import (
    ChainedLoaderFailureE2ETestCase,
    ChainedLoaderPruningE2ETestCase,
    ChainedLoaderSelectionE2ETestCase,
    IntermediateLoaderDefaultE2ETestCase,
    IntermediateLoaderStrategyE2ETestCase,
    LoaderWaffleShopE2ETestCase,
    SourceLoaderErrorE2ETestCase,
    SourceLoaderSchemaEvolutionE2ETestCase,
    SourceLoaderStrategiesE2ETestCase,
    SourceOnlyComplexIngressE2ETestCase,
    SourceOnlyIngressDependencyE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.load.helpers import (
    assert_source_only_ingress_dependency_case,
    build_loader_waffle_shop_project_files,
    build_schema_behavior_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    prepare_source_loader_strategies,
    query_duckdb,
    run_sqb,
    table_exists,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoaderStrategiesE2ETestCase(
            description="source loader strategies apply expected rows across two loads",
            command=("--no-color", "load", "--concurrency", "4"),
            expected_countries=((1, "US", "United States"), (2, "CA", "Canada")),
            expected_webhook_event_counts=((101, "signup", 2), (102, "checkout", 2)),
            expected_order_events=((201, 1000), (202, 2500), (203, 3000)),
            expected_customers=((1, "pro"), (2, "trial"), (3, "enterprise")),
            expected_loader_status=((1, "loaded", "self_managed"),),
            expected_stdout_fragments=(
                "raw_countries",
                "raw_webhook_events",
                "raw_order_events",
                "raw_customers",
                "raw_loader_status",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_strategy_project_when_loading_twice_then_all_write_modes_apply(
    tmp_path: Path,
    test_case: SourceLoaderStrategiesE2ETestCase,
) -> None:
    project_dir: Path = prepare_source_loader_strategies(tmp_path=tmp_path)
    db_path: Path = project_dir / "source_loader_strategies.duckdb"

    first_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )
    second_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert first_result.returncode == test_case.expected_return_code, (
        first_result.stdout + first_result.stderr
    )
    assert second_result.returncode == test_case.expected_return_code, (
        second_result.stdout + second_result.stderr
    )
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in second_result.stdout

    countries: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT country_id, country_code, country_name FROM raw_countries ORDER BY country_id"
        ),
    )
    webhook_event_counts: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT event_id, event_name, COUNT(*) FROM raw_webhook_events "
            "GROUP BY event_id, event_name ORDER BY event_id"
        ),
    )
    order_events: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id, amount_cents FROM raw_order_events ORDER BY event_id",
    )
    customers: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT customer_id, plan_name FROM raw_customers ORDER BY customer_id",
    )
    loader_status: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=("SELECT status_id, status_name, loaded_by FROM raw_loader_status ORDER BY status_id"),
    )

    assert tuple(countries) == test_case.expected_countries
    assert tuple(webhook_event_counts) == test_case.expected_webhook_event_counts
    assert tuple(order_events) == test_case.expected_order_events
    assert tuple(customers) == test_case.expected_customers
    assert tuple(loader_status) == test_case.expected_loader_status


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoaderSchemaEvolutionE2ETestCase(
            description="non-contract source adds late columns to existing append target",
            command=("--no-color", "load"),
            expected_rows=((1, None), (2, "late-note")),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_non_contract_loader_when_late_column_appears_then_existing_target_evolves(
    tmp_path: Path,
    test_case: SourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_events\n    description: Test source raw_events.\n"
                "    managed: true\n"
                "    write_strategy: append\n"
                "    cursor_column: load_seq\n"
                "    columns:\n"
                "      - name: event_id\n"
                "        type: INTEGER\n"
                "      - name: load_seq\n"
                "        type: INTEGER\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    if ctx.current_cursor_value is None:\n"
                "        return [{'event_id': 1, 'load_seq': 1}]\n"
                "    return [{'event_id': 2, 'load_seq': 2, 'note': 'late-note'}]\n"
            ),
        ),
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    first_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )
    second_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert first_result.returncode == test_case.expected_return_code, (
        first_result.stdout + first_result.stderr
    )
    assert second_result.returncode == test_case.expected_return_code, (
        second_result.stdout + second_result.stderr
    )
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id, note FROM raw_events ORDER BY event_id",
    )
    assert tuple(rows) == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderWaffleShopE2ETestCase(
            description="loader focused waffle shop grows across repeated builds",
            command=("--no-color", "build", "--select", "+customer_revenue"),
            run_count=3,
            expected_order_count=6,
            expected_customer_revenue_rows=(
                (1, "pro", 650, 1),
                (2, "plus", 6750, 3),
                (3, "enterprise", 3250, 2),
            ),
            expected_intermediate_counts=(
                ("__loader__fetch_order_events", 6),
                ("__loader__fetch_prices", 2),
                ("__loader__fetch_customers", 3),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_waffle_shop_project_when_building_repeatedly_then_dag_grows_models(
    tmp_path: Path,
    test_case: LoaderWaffleShopE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="loader_waffle_shop",
        repo_files=build_loader_waffle_shop_project_files(),
    )
    db_path: Path = project_dir / "loader_waffle_shop.duckdb"

    for _ in range(test_case.run_count):
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )
        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        assert "loader    fetch_order_events" in result.stdout
        assert "loader    fetch_prices" in result.stdout
        assert "loader    fetch_customers" in result.stdout
        assert "source    raw_orders" in result.stdout
        assert "source    raw_customers" in result.stdout

    order_count_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT COUNT(*) FROM fact_waffle_orders",
    )
    customer_revenue_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT customer_id, plan_name, revenue_cents, order_count "
            "FROM customer_revenue ORDER BY customer_id"
        ),
    )
    assert int(order_count_rows[0][0]) == test_case.expected_order_count
    assert tuple(customer_revenue_rows) == test_case.expected_customer_revenue_rows
    for relation_name, expected_count in test_case.expected_intermediate_counts:
        count_rows: list[tuple[object, ...]] = query_duckdb(
            db_path=db_path,
            sql=f"SELECT COUNT(*) FROM {relation_name}",
        )
        assert int(count_rows[0][0]) == expected_count


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoaderErrorE2ETestCase(
            description="plan separates intermediate loaders and terminal sources",
            repo_files=build_loader_waffle_shop_project_files(),
            command=("--no-color", "plan", "--select", "+customer_revenue"),
            expected_error_fragment="",
            expected_return_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_waffle_shop_project_when_planning_then_shows_loader_dag_sections(
    tmp_path: Path,
    test_case: SourceLoaderErrorE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="loader_waffle_shop",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    assert "Loaders to load (3)" in result.stdout
    assert "fetch_order_events" in result.stdout
    assert "fetch_prices" in result.stdout
    assert "fetch_customers" in result.stdout
    assert "Sources to load (2)" in result.stdout
    assert "raw_orders" in result.stdout
    assert "raw_customers" in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoaderSchemaEvolutionE2ETestCase(
            description="shared intermediate runs once for multiple terminal sources",
            command=(
                "--no-color",
                "load",
                "--select",
                "+raw_orders",
                "--select",
                "+raw_order_metrics",
            ),
            expected_rows=((1,), (2,)),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_two_terminal_sources_when_sharing_intermediate_then_intermediate_runs_once(
    tmp_path: Path,
    test_case: SourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "  - name: raw_order_metrics\n    description: Test source raw_order_metrics.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='append', cursor_column='load_seq', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_orders(ctx):\n"
                "    '''Test loader fetch_orders.'''\n    if ctx.current_cursor_value is None:\n"
                "        next_seq = 1\n"
                "    else:\n"
                "        next_seq = ctx.current_cursor_value + 1\n"
                "    return [{'event_id': next_seq, 'load_seq': next_seq}]\n\n"
                "@loader(depends_on=[fetch_orders])\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    orders = ctx.loader(fetch_orders)\n"
                "    cursor = ctx.query(\n"
                "        f'SELECT event_id FROM {orders.destination} ORDER BY event_id'\n"
                "    )\n"
                "    return [{'event_id': row[0]} for row in cursor.fetchall()]\n\n"
                "@loader(depends_on=[fetch_orders])\n"
                "def raw_order_metrics(ctx):\n"
                "    '''Test loader raw_order_metrics.'''\n    orders = ctx.loader(fetch_orders)\n"
                "    cursor = ctx.query(\n"
                "        f'SELECT event_id FROM {orders.destination} ORDER BY event_id'\n"
                "    )\n"
                "    return [{'event_id': row[0]} for row in cursor.fetchall()]\n"
            ),
        ),
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    for _ in range(2):
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )
        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr

    intermediate_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id FROM __loader__fetch_orders ORDER BY event_id",
    )
    raw_orders_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id FROM raw_orders ORDER BY event_id",
    )
    raw_metrics_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id FROM raw_order_metrics ORDER BY event_id",
    )
    assert tuple(intermediate_rows) == test_case.expected_rows
    assert tuple(raw_orders_rows) == test_case.expected_rows
    assert tuple(raw_metrics_rows) == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoaderSchemaEvolutionE2ETestCase(
            description="chained loader writes intermediate and terminal source tables",
            command=("--no-color", "load", "--select", "+raw_events"),
            expected_rows=((1, "loaded", 1), (2, "loaded", 1)),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_loading_source_then_runs_dependencies_first(
    tmp_path: Path,
    test_case: SourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_events\n    description: Test source raw_events.\n"
                "    managed: true\n"
                "    columns:\n"
                "      - name: event_id\n"
                "        type: INTEGER\n"
                "      - name: status\n"
                "        type: VARCHAR\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', cursor_column='load_seq', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    source_cursor = ctx.source('raw_events').max('event_id')\n"
                "    return [\n"
                "        {'event_id': 1, 'load_seq': 1},\n"
                "        {'event_id': 2, 'load_seq': 1},\n"
                "    ] if source_cursor is None else []\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                "SELECT event_id, 'loaded' AS status, {events.current_cursor_value} AS max_seq "
                'FROM {events.destination}")\n'
            ),
        ),
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    assert "loader    fetch_events" in result.stdout
    assert "source    raw_events" in result.stdout
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id, status, max_seq FROM raw_events ORDER BY event_id",
    )
    intermediate_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id, load_seq FROM __loader__fetch_events ORDER BY event_id",
    )
    assert tuple(rows) == test_case.expected_rows
    assert tuple(intermediate_rows) == ((1, 1), (2, 1))


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoaderSchemaEvolutionE2ETestCase(
            description="build auto-loads intermediate loader dependencies",
            command=("--no-color", "build", "--select", "+fact_events"),
            expected_rows=((1, "loaded"), (2, "loaded")),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_building_source_model_then_runs_loader_dag(
    tmp_path: Path,
    test_case: SourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_events\n    description: Test source raw_events.\n"
                "    managed: true\n"
                "    columns:\n"
                "      - name: event_id\n"
                "        type: INTEGER\n"
                "      - name: status\n"
                "        type: VARCHAR\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    return [{'event_id': 1}, {'event_id': 2}]\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                "SELECT event_id, 'loaded' AS status FROM {events.destination}\")\n"
            ),
        )
        | {
            "models/fact_events.sql": (
                "MODEL (description 'Test model fact_events.', materialized table);\n\n"
                'SELECT event_id, status FROM __source("raw_events")\n'
            )
        },
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    assert "loader    fetch_events" in result.stdout
    assert "source    raw_events" in result.stdout
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id, status FROM fact_events ORDER BY event_id",
    )
    intermediate_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id FROM __loader__fetch_events ORDER BY event_id",
    )
    assert tuple(rows) == test_case.expected_rows
    assert tuple(intermediate_rows) == ((1,), (2,))


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoaderSchemaEvolutionE2ETestCase(
            description="run auto-loads intermediate loader dependencies",
            command=(
                "--no-color",
                "build",
                "--no-tests",
                "--no-audits",
                "--select",
                "+fact_events",
            ),
            expected_rows=((1, "loaded"), (2, "loaded")),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_running_source_model_then_runs_loader_dag(
    tmp_path: Path,
    test_case: SourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_events\n    description: Test source raw_events.\n"
                "    managed: true\n"
                "    columns:\n"
                "      - name: event_id\n"
                "        type: INTEGER\n"
                "      - name: status\n"
                "        type: VARCHAR\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    return [{'event_id': 1}, {'event_id': 2}]\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                "SELECT event_id, 'loaded' AS status FROM {events.destination}\")\n"
            ),
        )
        | {
            "models/fact_events.sql": (
                "MODEL (description 'Test model fact_events.', materialized table);\n\n"
                'SELECT event_id, status FROM __source("raw_events")\n'
            )
        },
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    assert "loader    fetch_events" in result.stdout
    assert "source    raw_events" in result.stdout
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id, status FROM fact_events ORDER BY event_id",
    )
    intermediate_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id FROM __loader__fetch_events ORDER BY event_id",
    )
    assert tuple(rows) == test_case.expected_rows
    assert tuple(intermediate_rows) == ((1,), (2,))


@pytest.mark.parametrize(
    "test_case",
    [
        SourceOnlyComplexIngressE2ETestCase(
            description="source-only load fails when loader reads unselected task payload",
            command=("--no-color", "load", "--select", "raw_events"),
            expected_return_code=1,
            expected_error_fragment="No persisted result found for Python node 'prepare_events'",
            dependency_dir="tasks",
            decorator_import="from sqlbuild.tasks import task\n",
            decorator_name="task",
        ),
        SourceOnlyComplexIngressE2ETestCase(
            description="source-only load fails when loader reads unselected asset payload",
            command=("--no-color", "load", "--select", "raw_events"),
            expected_return_code=1,
            expected_error_fragment="No persisted result found for Python node 'prepare_events'",
            dependency_dir="assets",
            decorator_import="from sqlbuild.assets import asset\n",
            decorator_name="asset",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_reads_unselected_task_payload_when_loading_then_runtime_fails(
    tmp_path: Path,
    test_case: SourceOnlyComplexIngressE2ETestCase,
) -> None:
    assert test_case.expected_error_fragment is not None
    dependency_file: str = f"python/{test_case.dependency_dir}/prepare.py"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files={
            **build_schema_behavior_project_files(
                source_yaml=(
                    "sources:\n"
                    "  - name: raw_events\n    description: Test source raw_events.\n"
                    "    managed: true\n"
                    "    write_strategy: table\n"
                    "    columns:\n"
                    "      - name: event_id\n"
                    "        type: INTEGER\n"
                ),
                loader_py=(
                    "from sqlbuild.loaders import loader\n"
                    f"from python.{test_case.dependency_dir}.prepare import prepare_events\n\n"
                    "@loader(depends_on=[prepare_events])\n"
                    "def raw_events(ctx):\n"
                    "    '''Test loader raw_events.'''\n    payload = ctx.result_of(node_function=prepare_events).payload\n"
                    "    return [{'event_id': payload['event_id']}]\n"
                ),
            ),
            dependency_file: (
                f"{test_case.decorator_import}\n"
                f"@{test_case.decorator_name}\n"
                "def prepare_events(ctx):\n"
                "    return ctx.result(payload={'event_id': 1}, materialized=True)\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    expected_error_fragment: str | None = test_case.expected_error_fragment
    assert expected_error_fragment is not None
    assert expected_error_fragment in result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        SourceOnlyIngressDependencyE2ETestCase(
            description="source-only load fails when skipped intermediate target is missing",
            setup_command=None,
            command=("--no-color", "load", "--select", "raw_events"),
            expected_return_code=1,
            expected_error_fragment=(
                "requires intermediate loader 'fetch_events', but its target relation "
                "'__loader__fetch_events' does not exist"
            ),
        ),
        SourceOnlyIngressDependencyE2ETestCase(
            description="source-only load reuses existing intermediate target without rerunning it",
            setup_command=("--no-color", "load", "--select", "fetch_events"),
            command=("--no-color", "load", "--select", "raw_events"),
            expected_return_code=0,
            expected_stdout_fragments=("Sources (1)", "source    raw_events"),
            expected_intermediate_rows=((1, 1), (2, 1)),
            expected_terminal_rows=((1,), (2,)),
        ),
        SourceOnlyIngressDependencyE2ETestCase(
            description="expanded source load runs intermediate loader dependencies",
            setup_command=None,
            command=("--no-color", "load", "--select", "+raw_events"),
            expected_return_code=0,
            expected_stdout_fragments=(
                "Loaders (1)",
                "loader    fetch_events",
                "source    raw_events",
            ),
            expected_intermediate_rows=((1, 1), (2, 1)),
            expected_terminal_rows=((1,), (2,)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_only_selection_when_upstream_ingress_exists_then_enforces_refresh_policy(
    tmp_path: Path,
    test_case: SourceOnlyIngressDependencyE2ETestCase,
) -> None:
    assert test_case.expected_return_code in {0, 1}
    assert_source_only_ingress_dependency_case(
        tmp_path=tmp_path,
        test_case=test_case,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SourceOnlyComplexIngressE2ETestCase(
            description="source-only load validates multiple skipped intermediate targets",
            command=("--no-color", "load", "--select", "raw_events"),
            expected_return_code=1,
            setup_commands=(("--no-color", "load", "--select", "fetch_events"),),
            expected_error_fragment=(
                "requires intermediate loader 'fetch_prices', but its target relation "
                "'__loader__fetch_prices' does not exist"
            ),
        ),
        SourceOnlyComplexIngressE2ETestCase(
            description="source-only load validates transitive skipped intermediate targets",
            command=("--no-color", "load", "--select", "raw_rollup"),
            expected_return_code=1,
            expected_error_fragment=(
                "requires intermediate loader 'fetch_pages', but its target relation "
                "'__loader__fetch_pages' does not exist"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_only_selection_when_skipping_complex_intermediates_then_validates_all_targets(
    tmp_path: Path,
    test_case: SourceOnlyComplexIngressE2ETestCase,
) -> None:
    assert test_case.expected_return_code == 1
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_events\n    description: Test source raw_events.\n"
                "    managed: true\n"
                "  - name: raw_rollup\n    description: Test source raw_rollup.\n"
                "    managed: true\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    return [{'event_id': 1}]\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'price_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_prices(ctx):\n"
                "    '''Test loader fetch_prices.'''\n    return [{'price_id': 1}]\n\n"
                "@loader(depends_on=[fetch_events, fetch_prices])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                "    prices = ctx.loader(fetch_prices)\n"
                "    ctx.execute_sql(f'CREATE OR REPLACE TABLE {ctx.destination} AS "
                "SELECT event_id FROM {events.destination} UNION ALL "
                "SELECT price_id FROM {prices.destination}')\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'page_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_pages(ctx):\n"
                "    '''Test loader fetch_pages.'''\n    return [{'page_id': 1}]\n\n"
                "@loader(depends_on=[fetch_pages], write_strategy='table', "
                "columns=[{'name': 'page_id', 'type': 'INTEGER'}])\n"
                "def normalize_pages(ctx):\n"
                "    '''Test loader normalize_pages.'''\n    pages = ctx.loader(fetch_pages)\n"
                "    ctx.execute_sql(f'CREATE OR REPLACE TABLE {ctx.destination} AS "
                "SELECT page_id FROM {pages.destination}')\n\n"
                "@loader(depends_on=[normalize_pages])\n"
                "def raw_rollup(ctx):\n"
                "    '''Test loader raw_rollup.'''\n    pages = ctx.loader(normalize_pages)\n"
                "    ctx.execute_sql(f'CREATE OR REPLACE TABLE {ctx.destination} AS "
                "SELECT page_id FROM {pages.destination}')\n"
            ),
        ),
    )
    setup_command: tuple[str, ...]
    for setup_command in test_case.setup_commands:
        setup_result: subprocess.CompletedProcess[str] = run_sqb(
            command=setup_command,
            project_dir=project_dir,
        )
        assert setup_result.returncode == 0, setup_result.stdout + setup_result.stderr

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    expected_error_fragment: str | None = test_case.expected_error_fragment
    assert expected_error_fragment is not None
    assert expected_error_fragment in result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ChainedLoaderSelectionE2ETestCase(
            description="direct intermediate selection loads only the intermediate target",
            command=("--no-color", "load", "--select", "fetch_events"),
            expected_raw_events_exists=False,
            expected_intermediate_rows=((1, 1), (2, 1)),
            expected_stdout_fragments=(
                "Loaders (1)",
                "  fetch_events",
                "loader    fetch_events",
                "Sources (0)",
            ),
        ),
        ChainedLoaderSelectionE2ETestCase(
            description="leading plus source selection includes upstream intermediate",
            command=("--no-color", "load", "--select", "+raw_events"),
            expected_raw_events_exists=True,
            expected_intermediate_rows=((1, 1), (2, 1)),
            expected_stdout_fragments=(
                "Loaders (1)",
                "  fetch_events",
                "loader    fetch_events",
                "Sources (1)",
                "source    raw_events",
            ),
        ),
        ChainedLoaderSelectionE2ETestCase(
            description="trailing plus includes downstream terminal source",
            command=("--no-color", "load", "--select", "fetch_events+"),
            expected_raw_events_exists=True,
            expected_intermediate_rows=((1, 1), (2, 1)),
            expected_stdout_fragments=(
                "Loaders (1)",
                "  fetch_events",
                "loader    fetch_events",
                "Sources (1)",
                "source    raw_events",
            ),
        ),
        ChainedLoaderSelectionE2ETestCase(
            description="both sided plus includes upstream and downstream load graph",
            command=("--no-color", "load", "--select", "+fetch_events+"),
            expected_raw_events_exists=True,
            expected_intermediate_rows=((1, 1), (2, 1)),
            expected_stdout_fragments=(
                "Loaders (1)",
                "  fetch_events",
                "loader    fetch_events",
                "Sources (1)",
                "source    raw_events",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_selecting_loader_then_expands_expected_nodes(
    tmp_path: Path,
    test_case: ChainedLoaderSelectionE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n  - name: raw_events\n    description: Test source raw_events.\n    managed: true\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    return [{'event_id': 1, 'load_seq': 1}, {'event_id': 2, 'load_seq': 1}]\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                'SELECT event_id FROM {events.destination}")\n'
            ),
        ),
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    intermediate_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id, load_seq FROM __loader__fetch_events ORDER BY event_id",
    )
    assert tuple(intermediate_rows) == test_case.expected_intermediate_rows
    assert table_exists(db_path=db_path, table_name="raw_events") is (
        test_case.expected_raw_events_exists
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ChainedLoaderPruningE2ETestCase(
            description="excluding required intermediate prunes terminal source",
            command=("--no-color", "load", "--select", "+raw_events", "--exclude", "fetch_events"),
            expected_raw_events_exists=False,
            expected_intermediate_exists=False,
        ),
        ChainedLoaderPruningE2ETestCase(
            description="excluding downstream from intermediate prunes terminal source",
            command=(
                "--no-color",
                "load",
                "--select",
                "fetch_events+",
                "--exclude",
                "fetch_events+",
            ),
            expected_raw_events_exists=False,
            expected_intermediate_exists=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_excluding_dependency_then_prunes_dependents(
    tmp_path: Path,
    test_case: ChainedLoaderPruningE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n  - name: raw_events\n    description: Test source raw_events.\n    managed: true\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    return [{'event_id': 1}]\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                'SELECT event_id FROM {events.destination}")\n'
            ),
        ),
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    assert (db_path.exists() and table_exists(db_path=db_path, table_name="raw_events")) is (
        test_case.expected_raw_events_exists
    )
    assert (
        db_path.exists() and table_exists(db_path=db_path, table_name="__loader__fetch_events")
    ) is (test_case.expected_intermediate_exists)


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoaderSchemaEvolutionE2ETestCase(
            description="custom intermediate target is used by downstream loader refs",
            command=("--no-color", "load", "--select", "+raw_events"),
            expected_rows=((1, "custom_fetch_events"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_intermediate_has_custom_target_then_refs_resolve(
    tmp_path: Path,
    test_case: SourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n  - name: raw_events\n    description: Test source raw_events.\n    managed: true\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(destination='custom_fetch_events', write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    return [{'event_id': 1}]\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                "SELECT event_id, '{events.table_name}' AS source_table "
                'FROM {events.destination}")\n'
            ),
        ),
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id, source_table FROM raw_events ORDER BY event_id",
    )
    custom_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id FROM custom_fetch_events ORDER BY event_id",
    )
    assert tuple(rows) == test_case.expected_rows
    assert tuple(custom_rows) == ((1,),)


@pytest.mark.parametrize(
    "test_case",
    [
        IntermediateLoaderStrategyE2ETestCase(
            description="append intermediate accumulates rows across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='append', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "    {'name': 'amount', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    return [{'event_id': 1, 'amount': 100}, {'event_id': 2, 'amount': 200}]\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                "    cursor = ctx.query(\n"
                "        f'SELECT event_id, amount FROM {events.destination} '\n"
                "        'ORDER BY event_id, amount'\n"
                "    )\n"
                "    rows = cursor.fetchall()\n"
                "    return [{'event_id': row[0], 'amount': row[1]} for row in rows]\n"
            ),
            expected_intermediate_rows=((1, 100), (1, 100), (2, 200), (2, 200)),
            expected_terminal_rows=((1, 100), (1, 100), (2, 200), (2, 200)),
        ),
        IntermediateLoaderStrategyE2ETestCase(
            description="merge intermediate remains keyed across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(\n"
                "    write_strategy='merge',\n"
                "    unique_key='event_id',\n"
                "    cursor_column='load_seq',\n"
                "    columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "    {'name': 'amount', 'type': 'INTEGER'},\n"
                "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    if ctx.current_cursor_value is None:\n"
                "        return [\n"
                "            {'event_id': 1, 'amount': 100, 'load_seq': 1},\n"
                "            {'event_id': 2, 'amount': 200, 'load_seq': 1},\n"
                "        ]\n"
                "    return [\n"
                "        {'event_id': 1, 'amount': 150, 'load_seq': 2},\n"
                "        {'event_id': 3, 'amount': 300, 'load_seq': 2},\n"
                "    ]\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                "    cursor = ctx.query(\n"
                "        f'SELECT event_id, amount FROM {events.destination} '\n"
                "        'ORDER BY event_id, amount'\n"
                "    )\n"
                "    rows = cursor.fetchall()\n"
                "    return [{'event_id': row[0], 'amount': row[1]} for row in rows]\n"
            ),
            expected_intermediate_rows=((1, 150), (2, 200), (3, 300)),
            expected_terminal_rows=((1, 150), (2, 200), (3, 300)),
        ),
        IntermediateLoaderStrategyE2ETestCase(
            description="delete insert intermediate is idempotent across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='delete_insert', cursor_column='load_seq', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "    {'name': 'amount', 'type': 'INTEGER'},\n"
                "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    if ctx.current_cursor_value is not None:\n"
                "        return [\n"
                "            {'event_id': 2, 'amount': 250, 'load_seq': 1},\n"
                "            {'event_id': 3, 'amount': 300, 'load_seq': 1},\n"
                "        ]\n"
                "    return [\n"
                "        {'event_id': 1, 'amount': 100, 'load_seq': 1},\n"
                "        {'event_id': 2, 'amount': 200, 'load_seq': 1},\n"
                "    ]\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                "    cursor = ctx.query(\n"
                "        f'SELECT event_id, amount FROM {events.destination} '\n"
                "        'ORDER BY event_id, amount'\n"
                "    )\n"
                "    rows = cursor.fetchall()\n"
                "    return [{'event_id': row[0], 'amount': row[1]} for row in rows]\n"
            ),
            expected_intermediate_rows=((2, 250), (3, 300)),
            expected_terminal_rows=((2, 250), (3, 300)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_intermediate_uses_strategy_then_applies_strategy(
    tmp_path: Path,
    test_case: IntermediateLoaderStrategyE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_events\n    description: Test source raw_events.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: event_id\n"
                "        type: INTEGER\n"
                "      - name: amount\n"
                "        type: INTEGER\n"
            ),
            loader_py=test_case.loader_py,
        ),
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    first_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )
    second_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert first_result.returncode == test_case.expected_return_code, (
        first_result.stdout + first_result.stderr
    )
    assert second_result.returncode == test_case.expected_return_code, (
        second_result.stdout + second_result.stderr
    )
    intermediate_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=("SELECT event_id, amount FROM __loader__fetch_events ORDER BY event_id, amount"),
    )
    terminal_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id, amount FROM raw_events ORDER BY event_id, amount",
    )
    assert tuple(intermediate_rows) == test_case.expected_intermediate_rows
    assert tuple(terminal_rows) == test_case.expected_terminal_rows


@pytest.mark.parametrize(
    "test_case",
    [
        IntermediateLoaderDefaultE2ETestCase(
            description="sqb load writes the docs' intermediate loader as a table",
            command=("--no-color", "load", "--select", "+raw_account_metrics"),
            expected_intermediate_rows=((1, "Pawnee Parks"), (2, "Eagleton")),
            expected_terminal_rows=((1, "active"), (2, "active")),
        ),
        IntermediateLoaderDefaultE2ETestCase(
            description="sqb build writes the docs' intermediate loader as a table",
            command=("--no-color", "build", "--select", "+source:raw_account_metrics"),
            expected_intermediate_rows=((1, "Pawnee Parks"), (2, "Eagleton")),
            expected_terminal_rows=((1, "active"), (2, "active")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_intermediate_loader_without_write_strategy_when_loading_twice_then_rows_replace_a_table(
    tmp_path: Path,
    test_case: IntermediateLoaderDefaultE2ETestCase,
) -> None:
    edge_cases: Path = Path("benchmarks/duckshop/edge_cases/loaders")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(edge_cases / "intermediate_default_accounts.yml").read_text(),
            loader_py=(edge_cases / "intermediate_default_accounts.py").read_text(),
        ),
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    results: list[subprocess.CompletedProcess[str]] = [
        run_sqb(command=test_case.command, project_dir=project_dir) for _ in range(2)
    ]

    for result in results:
        assert result.returncode == 0, test_case.description + result.stdout + result.stderr
    assert (
        tuple(
            query_duckdb(
                db_path=db_path,
                sql="SELECT account_id, account_name FROM __loader__raw_accounts ORDER BY 1",
            )
        )
        == test_case.expected_intermediate_rows
    )
    assert (
        tuple(
            query_duckdb(
                db_path=db_path,
                sql="SELECT account_id, metric FROM raw_account_metrics ORDER BY 1",
            )
        )
        == test_case.expected_terminal_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ChainedLoaderFailureE2ETestCase(
            description=(
                "failed intermediate skips dependent source but independent branch continues"
            ),
            command=("--no-color", "load"),
            expected_ok_rows=((1,),),
            expected_error_fragment="boom",
            expected_skip_fragment="FAIL=1  SKIP=1",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_dependency_failure_when_loading_then_only_dependents_skip(
    tmp_path: Path,
    test_case: ChainedLoaderFailureE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_bad\n    description: Test source raw_bad.\n"
                "    managed: true\n"
                "  - name: raw_ok\n    description: Test source raw_ok.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_bad(ctx):\n"
                "    '''Test loader fetch_bad.'''\n    raise RuntimeError('boom')\n\n"
                "@loader(depends_on=[fetch_bad])\n"
                "def raw_bad(ctx):\n"
                "    '''Test loader raw_bad.'''\n    events = ctx.loader(fetch_bad)\n"
                '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                'SELECT event_id FROM {events.destination}")\n\n'
                "@loader\n"
                "def raw_ok(ctx):\n"
                "    '''Test loader raw_ok.'''\n    return [{'event_id': 1}]\n"
            ),
        ),
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    assert test_case.expected_error_fragment in result.stdout
    assert test_case.expected_skip_fragment in result.stdout
    ok_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id FROM raw_ok ORDER BY event_id",
    )
    assert tuple(ok_rows) == test_case.expected_ok_rows
    assert table_exists(db_path=db_path, table_name="raw_bad") is False


@pytest.mark.parametrize(
    "test_case",
    [
        ChainedLoaderFailureE2ETestCase(
            description="build skips models dependent on failed loader branch only",
            command=("--no-color", "build", "--select", "+bad_model", "--select", "+ok_model"),
            expected_ok_rows=((1,),),
            expected_error_fragment="boom",
            expected_skip_fragment="SKIP",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_dependency_failure_when_building_then_only_dependents_skip(
    tmp_path: Path,
    test_case: ChainedLoaderFailureE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_bad\n    description: Test source raw_bad.\n"
                "    managed: true\n"
                "  - name: raw_ok\n    description: Test source raw_ok.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_bad(ctx):\n"
                "    '''Test loader fetch_bad.'''\n    raise RuntimeError('boom')\n\n"
                "@loader(depends_on=[fetch_bad])\n"
                "def raw_bad(ctx):\n"
                "    '''Test loader raw_bad.'''\n    events = ctx.loader(fetch_bad)\n"
                '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                'SELECT event_id FROM {events.destination}")\n\n'
                "@loader\n"
                "def raw_ok(ctx):\n"
                "    '''Test loader raw_ok.'''\n    return [{'event_id': 1}]\n"
            ),
        )
        | {
            "models/bad_model.sql": (
                'MODEL (description "Test model bad_model.", materialized table);\n\nSELECT event_id FROM __source("raw_bad")\n'
            ),
            "models/ok_model.sql": (
                'MODEL (description "Test model ok_model.", materialized table);\n\nSELECT event_id FROM __source("raw_ok")\n'
            ),
        },
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    assert test_case.expected_error_fragment in result.stdout
    assert test_case.expected_skip_fragment in result.stdout
    assert "ok_model" in result.stdout
    assert "bad_model" in result.stdout
    ok_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id FROM ok_model ORDER BY event_id",
    )
    assert tuple(ok_rows) == test_case.expected_ok_rows
    assert table_exists(db_path=db_path, table_name="bad_model") is False


@pytest.mark.parametrize(
    "test_case",
    [
        ChainedLoaderFailureE2ETestCase(
            description="json output includes skipped dependent loader result",
            command=("--no-color", "load", "--json"),
            expected_ok_rows=((1,),),
            expected_error_fragment="boom",
            expected_skip_fragment="skipped",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_dependency_failure_when_loading_json_then_reports_skipped_asset(
    tmp_path: Path,
    test_case: ChainedLoaderFailureE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_bad\n    description: Test source raw_bad.\n"
                "    managed: true\n"
                "  - name: raw_ok\n    description: Test source raw_ok.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
            ),
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_bad(ctx):\n"
                "    '''Test loader fetch_bad.'''\n    raise RuntimeError('boom')\n\n"
                "@loader(depends_on=[fetch_bad])\n"
                "def raw_bad(ctx):\n"
                "    '''Test loader raw_bad.'''\n    events = ctx.loader(fetch_bad)\n"
                '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                'SELECT event_id FROM {events.destination}")\n\n'
                "@loader\n"
                "def raw_ok(ctx):\n"
                "    '''Test loader raw_ok.'''\n    return [{'event_id': 1}]\n"
            ),
        ),
    )
    db_path: Path = project_dir / "source_loader_schema_behavior.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    assets: list[dict[str, object]] = list(payload["assets"])
    statuses_by_name: dict[str, object] = {str(asset["name"]): asset["status"] for asset in assets}
    summary: dict[str, object] = dict(payload["summary"])
    assert statuses_by_name["fetch_bad"] == "failed"
    assert statuses_by_name["raw_bad"] == test_case.expected_skip_fragment
    assert statuses_by_name["raw_ok"] == "success"
    assert summary["skipped_count"] == 1
    ok_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT event_id FROM raw_ok ORDER BY event_id",
    )
    assert tuple(ok_rows) == test_case.expected_ok_rows


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoaderErrorE2ETestCase(
            description="contract rejects extra returned columns",
            command=("--no-color", "load"),
            expected_error_fragment="contract has extra columns: extra_note",
            repo_files=build_schema_behavior_project_files(
                source_yaml=(
                    "sources:\n"
                    "  - name: raw_contract_events\n    description: Test source raw_contract_events.\n"
                    "    managed: true\n"
                    "    write_strategy: table\n"
                    "    contract: enforced\n"
                    "    columns:\n"
                    "      - name: event_id\n"
                    "        type: INTEGER\n"
                ),
                loader_py=(
                    "from sqlbuild.loaders import loader\n\n"
                    "@loader\n"
                    "def raw_contract_events(ctx):\n"
                    "    '''Test loader raw_contract_events.'''\n    return [{'event_id': 1, 'extra_note': 'not declared'}]\n"
                ),
            ),
        ),
        SourceLoaderErrorE2ETestCase(
            description="contract rejects missing declared columns",
            command=("--no-color", "load"),
            expected_error_fragment="contract missing columns: event_name",
            repo_files=build_schema_behavior_project_files(
                source_yaml=(
                    "sources:\n"
                    "  - name: raw_contract_events\n    description: Test source raw_contract_events.\n"
                    "    managed: true\n"
                    "    write_strategy: table\n"
                    "    contract: enforced\n"
                    "    columns:\n"
                    "      - name: event_id\n"
                    "        type: INTEGER\n"
                    "      - name: event_name\n"
                    "        type: VARCHAR\n"
                ),
                loader_py=(
                    "from sqlbuild.loaders import loader\n\n"
                    "@loader\n"
                    "def raw_contract_events(ctx):\n"
                    "    '''Test loader raw_contract_events.'''\n    return [{'event_id': 1}]\n"
                ),
            ),
        ),
        SourceLoaderErrorE2ETestCase(
            description="existing target rejects returned type changes",
            command=("--no-color", "load"),
            expected_error_fragment="column 'amount' changed type",
            repo_files=build_schema_behavior_project_files(
                source_yaml=(
                    "sources:\n"
                    "  - name: raw_events\n    description: Test source raw_events.\n"
                    "    managed: true\n"
                    "    write_strategy: append\n"
                    "    cursor_column: load_seq\n"
                    "    columns:\n"
                    "      - name: event_id\n"
                    "        type: INTEGER\n"
                    "      - name: load_seq\n"
                    "        type: INTEGER\n"
                ),
                loader_py=(
                    "from sqlbuild.loaders import loader\n\n"
                    "@loader\n"
                    "def raw_events(ctx):\n"
                    "    '''Test loader raw_events.'''\n    if ctx.current_cursor_value is None:\n"
                    "        return [{'event_id': 1, 'load_seq': 1, 'amount': 100}]\n"
                    "    return [{'event_id': 2, 'load_seq': 2, 'amount': 'one hundred'}]\n"
                ),
            ),
        ),
        SourceLoaderErrorE2ETestCase(
            description="intermediate contract rejects extra returned columns",
            command=("--no-color", "load", "--select", "+raw_events"),
            expected_error_fragment="contract has extra columns: extra_note",
            repo_files=build_schema_behavior_project_files(
                source_yaml=(
                    "sources:\n  - name: raw_events\n    description: Test source raw_events.\n    managed: true\n"
                ),
                loader_py=(
                    "from sqlbuild.loaders import loader\n\n"
                    "@loader(write_strategy='table', contract='enforced', columns=[\n"
                    "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                    "])\n"
                    "def fetch_events(ctx):\n"
                    "    '''Test loader fetch_events.'''\n    return [{'event_id': 1, 'extra_note': 'not declared'}]\n\n"
                    "@loader(depends_on=[fetch_events])\n"
                    "def raw_events(ctx):\n"
                    "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                    '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                    'SELECT event_id FROM {events.destination}")\n'
                ),
            ),
        ),
        SourceLoaderErrorE2ETestCase(
            description="self managed intermediate without target fails",
            command=("--no-color", "load", "--select", "+raw_events"),
            expected_error_fragment="returned no rows and has no destination declared",
            repo_files=build_schema_behavior_project_files(
                source_yaml=(
                    "sources:\n  - name: raw_events\n    description: Test source raw_events.\n    managed: true\n"
                ),
                loader_py=(
                    "from sqlbuild.loaders import loader\n\n"
                    "@loader\n"
                    "def fetch_events(ctx):\n"
                    "    '''Test loader fetch_events.'''\n    ctx.execute_sql('SELECT 1')\n\n"
                    "@loader(depends_on=[fetch_events])\n"
                    "def raw_events(ctx):\n"
                    "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                    '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS '
                    'SELECT * FROM {events.destination}")\n'
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_schema_error_when_loading_then_reports_expected_failure(
    tmp_path: Path,
    test_case: SourceLoaderErrorE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=test_case.repo_files,
    )

    first_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert first_result.returncode in {0, test_case.expected_return_code}, (
        first_result.stdout + first_result.stderr
    )
    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    assert test_case.expected_error_fragment in result.stdout
