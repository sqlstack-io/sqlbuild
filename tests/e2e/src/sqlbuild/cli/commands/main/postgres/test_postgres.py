from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.load.helpers import (
    build_loader_waffle_shop_project_files,
    build_schema_behavior_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.main.postgres._test_types import (
    PostgresBuildE2ETestCase,
    PostgresDbtProfileE2ETestCase,
    PostgresDbtSeedChangeE2ETestCase,
    PostgresIntermediateDagStrategyE2ETestCase,
    PostgresLoaderWaffleShopE2ETestCase,
    PostgresNodeResultE2ETestCase,
    PostgresPartialSourceTypeEnforcementE2ETestCase,
    PostgresSnapshotApplyE2ETestCase,
    PostgresSnapshotE2ETestCase,
    PostgresSourceDeferralE2ETestCase,
    PostgresSourceLoaderDagE2ETestCase,
    PostgresSourceLoaderStrategiesE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.postgres.helpers import (
    append_postgres_dbt_seed_change_order,
    assert_current_postgres_snapshot_rows_from_case,
    assert_postgres_snapshot_apply_rows,
    assert_postgres_snapshot_matrix_rows,
    build_postgres_project_toml,
    build_postgres_source_deferral_project_toml,
    build_unique_schema_name,
    cleanup_postgres_schema,
    ensure_postgres_schema_ready,
    execute_postgres_sql,
    fetch_postgres_rows,
    postgres_dbt_env,
    prepare_postgres_dbt_seed_change_workspace,
    prepare_postgres_source_loader_strategies,
    prepare_postgres_waffle_shop,
    relation_name,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    add_dbt_profile_downstream_model,
    build_current_check_customers_model_sql,
    build_current_customers_model_sql,
    build_current_delete_customers_model_sql,
    build_historical_check_daily_model_sql,
    build_historical_timestamp_extracts_model_sql,
    build_real_warehouse_existing_snapshot_project_files,
    build_real_warehouse_snapshot_project_files,
    prepare_dbt_profile_workspace,
    prepare_inline_project,
    run_sqb,
    stringify_warehouse_rows,
    write_dbt_profile_orders_model,
)
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files


@pytest.mark.dbt
@pytest.mark.parametrize(
    "test_case",
    [
        PostgresDbtProfileE2ETestCase(
            description="dbt init generated project builds through postgres dbt profile",
            expected_toml_fragments=(
                'adapter = "postgres"',
                'source = "dbt_profile"',
                'profile = "analytics"',
                'target = "dev"',
            ),
            unexpected_toml_fragments=("DBT_POSTGRES_PASSWORD",),
            expected_initial_rows=((1,),),
            expected_changed_rows=((2,),),
            expected_rerun_fragments=("dbt execution", "SQLBuild execution"),
            expected_plain_selector_block_fragments=(
                "Skipping dbt: no dbt work selected.",
                'relation "',
                'dbt_orders" does not exist',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_postgres_dbt_profile_when_running_dbt_init_then_plain_build_uses_profile(
    tmp_path: Path,
    test_case: PostgresDbtProfileE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_dbt_profile")
    workspace: Path = prepare_dbt_profile_workspace(
        tmp_path=tmp_path,
        profiles_yml=(
            "analytics:\n"
            "  target: dev\n"
            "  outputs:\n"
            "    dev:\n"
            "      type: postgres\n"
            f"      host: {postgres_e2e_config['host']}\n"
            f"      port: {postgres_e2e_config['port']}\n"
            f"      dbname: {postgres_e2e_config['dbname']}\n"
            f"      user: {postgres_e2e_config['user']}\n"
            "      pass: \"{{ env_var('DBT_POSTGRES_PASSWORD') }}\"\n"
            f"      schema: {schema_name}\n"
        ),
    )
    sqlbuild_project_dir: Path = workspace / "sqlbuild_project"
    env: dict[str, str] = postgres_dbt_env(password=str(postgres_e2e_config["password"]))
    try:
        init_result: subprocess.CompletedProcess[str] = run_sqb(
            command=(
                "--no-color",
                "dbt",
                "init",
                "--project-dir",
                "dbt_project",
                "--profiles-dir",
                "profiles",
                "--skip-dbt-debug",
            ),
            project_dir=workspace,
            env=env,
        )

        assert init_result.returncode == test_case.expected_return_code, (
            init_result.stdout + init_result.stderr
        )
        generated_toml: str = (sqlbuild_project_dir / "sqlbuild_project.toml").read_text(
            encoding="utf-8"
        )
        for fragment in test_case.expected_toml_fragments:
            assert fragment in generated_toml
        for fragment in test_case.unexpected_toml_fragments:
            assert fragment not in generated_toml

        add_dbt_profile_downstream_model(sqlbuild_project_dir=sqlbuild_project_dir)

        plain_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "dbt", "build", "--select", "downstream_orders"),
            project_dir=sqlbuild_project_dir,
            env=env,
        )

        assert plain_result.returncode != test_case.expected_return_code, (
            plain_result.stdout + plain_result.stderr
        )
        for fragment in test_case.expected_plain_selector_block_fragments:
            assert fragment in plain_result.stdout
        assert (
            fetch_postgres_rows(
                config=postgres_e2e_config,
                sql=(
                    "SELECT table_name FROM information_schema.tables "
                    f"WHERE table_schema = '{schema_name}' "
                    "AND table_name IN ('dbt_orders', 'downstream_orders') "
                    "ORDER BY table_name"
                ),
            )
            == ()
        )

        first_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "dbt", "build", "--select", "+downstream_orders"),
            project_dir=sqlbuild_project_dir,
            env=env,
        )

        assert first_result.returncode == test_case.expected_return_code, (
            first_result.stdout + first_result.stderr
        )
        assert (
            fetch_postgres_rows(
                config=postgres_e2e_config,
                sql=f"SELECT order_id FROM {schema_name}.dbt_orders ORDER BY order_id",
            )
            == test_case.expected_initial_rows
        )
        assert (
            fetch_postgres_rows(
                config=postgres_e2e_config,
                sql=f"SELECT order_id FROM {schema_name}.downstream_orders ORDER BY order_id",
            )
            == test_case.expected_initial_rows
        )

        rerun_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "dbt", "build", "--select", "+downstream_orders"),
            project_dir=sqlbuild_project_dir,
            env=env,
        )

        assert rerun_result.returncode == test_case.expected_return_code, (
            rerun_result.stdout + rerun_result.stderr
        )
        for fragment in test_case.expected_rerun_fragments:
            assert fragment in rerun_result.stdout

        write_dbt_profile_orders_model(workspace=workspace, order_id=2)
        changed_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "dbt", "build", "--select", "+downstream_orders"),
            project_dir=sqlbuild_project_dir,
            env=env,
        )

        assert changed_result.returncode == test_case.expected_return_code, (
            changed_result.stdout + changed_result.stderr
        )
        assert (
            fetch_postgres_rows(
                config=postgres_e2e_config,
                sql=f"SELECT order_id FROM {schema_name}.dbt_orders ORDER BY order_id",
            )
            == test_case.expected_changed_rows
        )
        assert (
            fetch_postgres_rows(
                config=postgres_e2e_config,
                sql=f"SELECT order_id FROM {schema_name}.downstream_orders ORDER BY order_id",
            )
            == test_case.expected_changed_rows
        )
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresNodeResultE2ETestCase(
            description="direct node results persist and read on postgres",
            expected_rows=(
                ("asset", "produce_result", "success"),
                ("check", "check_produce_result", "success"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_result_when_running_check_on_postgres_then_persists_node_results(
    tmp_path: Path,
    test_case: PostgresNodeResultE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_node_results")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="postgres_node_results",
        repo_files={
            "sqlbuild_project.toml": build_postgres_project_toml(
                project_name="postgres_node_results",
                schema_name=schema_name,
                config=postgres_e2e_config,
            ),
            "python/nodes/tasks/results/asset.py": (
                "from sqlbuild.assets import asset\n\n"
                "@asset\n"
                "def produce_result(ctx):\n"
                "    '''Test task produce_result.'''\n    return ctx.result(payload={'value': 42}, metadata={'source': 'postgres'})\n"
            ),
            "python/nodes/checks/results/checks.py": (
                "from sqlbuild.checks import check\n"
                "from python.nodes.tasks.results.asset import produce_result\n\n"
                "@check(depends_on=produce_result)\n"
                "def check_produce_result(ctx):\n"
                "    '''Test check check_produce_result.'''\n    return ctx.result_of(node_function=produce_result).payload['value'] == 42\n"
            ),
        },
    )
    ensure_postgres_schema_ready(schema_name=schema_name, config=postgres_e2e_config)

    try:
        build_result: subprocess.CompletedProcess[str] = run_sqb(
            command=(
                "--no-color",
                "build",
                "--select",
                "produce_result",
                "--exclude",
                "check:check_produce_result",
            ),
            project_dir=project_dir,
        )
        check_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "check", "--select", "check:check_produce_result"),
            project_dir=project_dir,
        )

        assert build_result.returncode == test_case.expected_return_code, (
            build_result.stdout + build_result.stderr
        )
        assert check_result.returncode == test_case.expected_return_code, (
            check_result.stdout + check_result.stderr
        )
        rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            sql=(
                "SELECT node_type, node_name, status "
                f"FROM {relation_name(schema_name=schema_name, name='_sqlbuild_node_results')} "
                "WHERE node_name IN ('produce_result', 'check_produce_result') "
                "ORDER BY node_type, node_name"
            ),
            config=postgres_e2e_config,
        )
        assert rows == test_case.expected_rows
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresBuildE2ETestCase(
            description="waffle shop full build succeeds on postgres",
            command=("--no-color", "build", "--concurrency", "4"),
            expected_table_name="fact_orders",
            expected_row_count=10,
            expected_stdout_fragments=("Execution", "OK"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_running_full_build_on_postgres_then_expected_table_exists(
    tmp_path: Path,
    test_case: PostgresBuildE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    project_dir: Path
    schema_name: str
    project_dir, schema_name = prepare_postgres_waffle_shop(
        tmp_path=tmp_path, config=postgres_e2e_config
    )
    ensure_postgres_schema_ready(schema_name=schema_name, config=postgres_e2e_config)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in result.stdout
        rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            sql=(
                f"SELECT COUNT(*) FROM "
                f"{relation_name(schema_name=schema_name, name=test_case.expected_table_name)}"
            ),
            config=postgres_e2e_config,
        )
        row_count: object = rows[0][0]
        assert isinstance(row_count, int)
        assert row_count == test_case.expected_row_count
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresSourceDeferralE2ETestCase(
            description="postgres loader writes dev while model reads prod deferred source",
            expected_model_rows=(("99", "prod-source"),),
            expected_loader_rows=(("7", "loaded-dev"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_deferral_env_when_building_on_postgres_then_reads_prod_and_writes_dev(
    tmp_path: Path,
    test_case: PostgresSourceDeferralE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    dev_schema_name: str = build_unique_schema_name(prefix="sqlbuild_defer_dev")
    prod_schema_name: str = build_unique_schema_name(prefix="sqlbuild_defer_prod")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="postgres_source_deferral",
        repo_files={
            "sqlbuild_project.toml": build_postgres_source_deferral_project_toml(
                project_name="postgres_source_deferral",
                dev_schema_name=dev_schema_name,
                prod_schema_name=prod_schema_name,
                config=postgres_e2e_config,
            ),
            "sql/sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: BIGINT\n"
                "      - name: status\n"
                "        type: TEXT\n"
            ),
            "python/nodes/loaders/raw_orders/loader.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='replace')\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    return [{'order_id': 7, 'status': 'loaded-dev'}]\n"
            ),
            "sql/models/stg_orders.sql": (
                'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT order_id, status FROM __source("raw_orders")'
            ),
        },
    )
    ensure_postgres_schema_ready(schema_name=dev_schema_name, config=postgres_e2e_config)
    ensure_postgres_schema_ready(schema_name=prod_schema_name, config=postgres_e2e_config)

    try:
        execute_postgres_sql(
            config=postgres_e2e_config,
            sql=(
                f"CREATE TABLE {relation_name(schema_name=prod_schema_name, name='raw_orders')} "
                "(order_id INTEGER, status VARCHAR)"
            ),
        )
        execute_postgres_sql(
            config=postgres_e2e_config,
            sql=(
                f"INSERT INTO {relation_name(schema_name=prod_schema_name, name='raw_orders')} "
                "VALUES (99, 'prod-source')"
            ),
        )

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--select", "stg_orders"),
            project_dir=project_dir,
        )

        assert result.returncode == 0, result.stdout + result.stderr
        model_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT order_id, status FROM "
                f"{relation_name(schema_name=dev_schema_name, name='stg_orders')} "
                "ORDER BY order_id"
            ),
        )
        loader_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT order_id, status FROM "
                f"{relation_name(schema_name=dev_schema_name, name='raw_orders')} "
                "ORDER BY order_id"
            ),
        )
        assert stringify_warehouse_rows(model_rows) == test_case.expected_model_rows
        assert stringify_warehouse_rows(loader_rows) == test_case.expected_loader_rows
    finally:
        cleanup_postgres_schema(schema_name=dev_schema_name, config=postgres_e2e_config)
        cleanup_postgres_schema(schema_name=prod_schema_name, config=postgres_e2e_config)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresPartialSourceTypeEnforcementE2ETestCase(
            description="postgres partial source type enforcement preserves extra columns",
            expected_rows=(("7", "loaded-dev", "99"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_partial_source_type_enforcement_when_building_on_postgres_then_casts_columns(
    tmp_path: Path,
    test_case: PostgresPartialSourceTypeEnforcementE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_src_cast")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="postgres_partial_source_type_enforcement",
        repo_files={
            "sqlbuild_project.toml": build_postgres_project_toml(
                project_name="postgres_partial_source_type_enforcement",
                schema_name=schema_name,
                config=postgres_e2e_config,
            ),
            "sql/sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                f"    schema: {schema_name}\n"
                "    type_enforcement: true\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: BIGINT\n"
            ),
            "sql/models/stg_orders.sql": (
                "MODEL (description 'Test model stg_orders.', materialized table);\n\n"
                'SELECT order_id, status, amount_cents FROM __source("raw_orders")'
            ),
        },
    )
    ensure_postgres_schema_ready(schema_name=schema_name, config=postgres_e2e_config)

    try:
        execute_postgres_sql(
            config=postgres_e2e_config,
            sql=(
                f"CREATE TABLE {relation_name(schema_name=schema_name, name='raw_orders')} "
                "(order_id TEXT, status VARCHAR, amount_cents INTEGER)"
            ),
        )
        execute_postgres_sql(
            config=postgres_e2e_config,
            sql=(
                f"INSERT INTO {relation_name(schema_name=schema_name, name='raw_orders')} "
                "VALUES ('7', 'loaded-dev', 99)"
            ),
        )

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT order_id, status, amount_cents FROM "
                f"{relation_name(schema_name=schema_name, name='stg_orders')} "
                "ORDER BY order_id"
            ),
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresLoaderWaffleShopE2ETestCase(
            description="loader focused waffle shop grows across repeated postgres builds",
            command=("--no-color", "build", "--select", "+customer_revenue"),
            expected_rows=(
                ("1", "pro", "650", "1"),
                ("2", "plus", "3750", "2"),
                ("3", "enterprise", "1300", "1"),
            ),
            expected_event_count=4,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_waffle_shop_when_building_on_postgres_then_dag_grows_models(
    tmp_path: Path,
    test_case: PostgresLoaderWaffleShopE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_load_waffle")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="loader_waffle_shop",
        repo_files=build_loader_waffle_shop_project_files(
            project_toml=build_postgres_project_toml(
                project_name="loader_waffle_shop",
                schema_name=schema_name,
                config=postgres_e2e_config,
            )
        ),
    )
    ensure_postgres_schema_ready(schema_name=schema_name, config=postgres_e2e_config)

    try:
        for _ in range(2):
            result: subprocess.CompletedProcess[str] = run_sqb(
                command=test_case.command,
                project_dir=project_dir,
            )
            assert result.returncode == test_case.expected_return_code, (
                result.stdout + result.stderr
            )
            assert "loader    fetch_order_events" in result.stdout
            assert "source    raw_orders" in result.stdout

        rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT customer_id, plan_name, revenue_cents, order_count FROM "
                f"{relation_name(schema_name=schema_name, name='customer_revenue')} "
                "ORDER BY customer_id"
            ),
        )
        event_count_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT COUNT(*) FROM "
                f"{relation_name(schema_name=schema_name, name='__loader__fetch_order_events')}"
            ),
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
        assert int(str(event_count_rows[0][0])) == test_case.expected_event_count
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresSourceLoaderStrategiesE2ETestCase(
            description="source loader strategies apply expected rows on postgres",
            command=("--no-color", "load", "--concurrency", "4"),
            expected_countries=(("1", "US", "United States"), ("2", "CA", "Canada")),
            expected_webhook_event_counts=(("101", "signup", "2"), ("102", "checkout", "2")),
            expected_order_events=(("201", "1000"), ("202", "2500"), ("203", "3000")),
            expected_customers=(("1", "pro"), ("2", "trial"), ("3", "enterprise")),
            expected_loader_status=(("1", "loaded", "self_managed"),),
            expected_stdout_fragments=("raw_countries", "raw_webhook_events", "raw_customers"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_strategy_project_when_loading_twice_on_postgres_then_write_modes_apply(
    tmp_path: Path,
    test_case: PostgresSourceLoaderStrategiesE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    project_dir: Path
    schema_name: str
    project_dir, schema_name = prepare_postgres_source_loader_strategies(
        tmp_path=tmp_path,
        config=postgres_e2e_config,
    )
    ensure_postgres_schema_ready(schema_name=schema_name, config=postgres_e2e_config)

    try:
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

        countries: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT country_id, country_code, country_name FROM "
                f"{relation_name(schema_name=schema_name, name='raw_countries')} "
                "ORDER BY country_id"
            ),
        )
        webhook_event_counts: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT event_id, event_name, COUNT(*) FROM "
                f"{relation_name(schema_name=schema_name, name='raw_webhook_events')} "
                "GROUP BY event_id, event_name ORDER BY event_id"
            ),
        )
        order_events: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT event_id, amount_cents FROM "
                f"{relation_name(schema_name=schema_name, name='raw_order_events')} "
                "ORDER BY event_id"
            ),
        )
        customers: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT customer_id, plan_name FROM "
                f"{relation_name(schema_name=schema_name, name='raw_customers')} "
                "ORDER BY customer_id"
            ),
        )
        loader_status: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT status_id, status_name, loaded_by FROM "
                f"{relation_name(schema_name=schema_name, name='raw_loader_status')} "
                "ORDER BY status_id"
            ),
        )

        assert stringify_warehouse_rows(countries) == test_case.expected_countries
        assert stringify_warehouse_rows(webhook_event_counts) == (
            test_case.expected_webhook_event_counts
        )
        assert stringify_warehouse_rows(order_events) == test_case.expected_order_events
        assert stringify_warehouse_rows(customers) == test_case.expected_customers
        assert stringify_warehouse_rows(loader_status) == test_case.expected_loader_status
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresSourceLoaderDagE2ETestCase(
            description="chained source loader runs on postgres",
            command=("--no-color", "load", "--select", "+raw_events"),
            expected_rows=(("1", "loaded"), ("2", "loaded")),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_loading_on_postgres_then_runs_loader_dag(
    tmp_path: Path,
    test_case: PostgresSourceLoaderDagE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_load_dag")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_dag_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_events\n    description: Test source raw_events.\n"
                "    managed: true\n"
                "    columns:\n"
                "      - name: event_id\n"
                "        type: BIGINT\n"
                "      - name: status\n"
                "        type: TEXT\n"
            ),
            node_files={
                **node_module_files(
                    "python/nodes/loaders/source_rows.py",
                    "from sqlbuild.loaders import loader\n\n"
                    "@loader(write_strategy='replace', columns=[\n"
                    "    {'name': 'event_id', 'type': 'BIGINT'},\n"
                    "])\n"
                    "def fetch_events(ctx):\n"
                    "    '''Test loader fetch_events.'''\n    return [{'event_id': 1}, {'event_id': 2}]\n\n"
                    "@loader(write_strategy='replace', depends_on=[fetch_events])\n"
                    "def raw_events(ctx):\n"
                    "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                    "    cursor = ctx.query(\n"
                    "        f'SELECT event_id FROM {events.destination} ORDER BY event_id'\n"
                    "    )\n"
                    "    rows = cursor.fetchall()\n"
                    "    return [{'event_id': row[0], 'status': 'loaded'} for row in rows]\n",
                )
            },
        ),
    )
    (project_dir / "sqlbuild_project.toml").write_text(
        build_postgres_project_toml(
            project_name="source_loader_dag_behavior",
            schema_name=schema_name,
            config=postgres_e2e_config,
        ),
        encoding="utf-8",
    )
    ensure_postgres_schema_ready(schema_name=schema_name, config=postgres_e2e_config)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT event_id, status FROM "
                f"{relation_name(schema_name=schema_name, name='raw_events')} "
                "ORDER BY event_id"
            ),
        )
        intermediate_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT COUNT(*) FROM "
                f"{relation_name(schema_name=schema_name, name='__loader__fetch_events')}"
            ),
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
        assert int(str(intermediate_rows[0][0])) == 2
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresIntermediateDagStrategyE2ETestCase(
            description="postgres append intermediate accumulates rows across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='append', cursor_column='load_seq', columns=[\n"
                "    {'name': 'event_id', 'type': 'BIGINT'},\n"
                "    {'name': 'amount', 'type': 'BIGINT'},\n"
                "    {'name': 'load_seq', 'type': 'BIGINT'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    if ctx.cursor_value is None:\n"
                "        next_seq = 1\n"
                "    else:\n"
                "        next_seq = ctx.cursor_value + 1\n"
                "    return [\n"
                "        {'event_id': next_seq, 'amount': next_seq * 100, 'load_seq': next_seq}\n"
                "    ]\n\n"
                "@loader(write_strategy='replace', depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                "    cursor = ctx.query(\n"
                "        f'SELECT event_id, amount FROM {events.destination} '\n"
                "        'ORDER BY event_id, amount'\n"
                "    )\n"
                "    rows = cursor.fetchall()\n"
                "    return [{'event_id': row[0], 'amount': row[1]} for row in rows]\n"
            ),
            expected_intermediate_rows=(("1", "100"), ("2", "200")),
            expected_terminal_rows=(("1", "100"), ("2", "200")),
        ),
        PostgresIntermediateDagStrategyE2ETestCase(
            description="postgres merge intermediate updates and adds rows across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(\n"
                "    write_strategy='merge',\n"
                "    unique_key='event_id',\n"
                "    cursor_column='load_seq',\n"
                "    columns=[\n"
                "        {'name': 'event_id', 'type': 'BIGINT'},\n"
                "        {'name': 'amount', 'type': 'BIGINT'},\n"
                "        {'name': 'load_seq', 'type': 'BIGINT'},\n"
                "    ],\n"
                ")\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    if ctx.cursor_value is None:\n"
                "        return [\n"
                "            {'event_id': 1, 'amount': 100, 'load_seq': 1},\n"
                "            {'event_id': 2, 'amount': 200, 'load_seq': 1},\n"
                "        ]\n"
                "    return [\n"
                "        {'event_id': 1, 'amount': 150, 'load_seq': 2},\n"
                "        {'event_id': 3, 'amount': 300, 'load_seq': 2},\n"
                "    ]\n\n"
                "@loader(write_strategy='replace', depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                "    cursor = ctx.query(\n"
                "        f'SELECT event_id, amount FROM {events.destination} '\n"
                "        'ORDER BY event_id, amount'\n"
                "    )\n"
                "    rows = cursor.fetchall()\n"
                "    return [{'event_id': row[0], 'amount': row[1]} for row in rows]\n"
            ),
            expected_intermediate_rows=(("1", "150"), ("2", "200"), ("3", "300")),
            expected_terminal_rows=(("1", "150"), ("2", "200"), ("3", "300")),
        ),
        PostgresIntermediateDagStrategyE2ETestCase(
            description="postgres delete insert intermediate replaces cursor window across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='delete_insert', cursor_column='load_seq', columns=[\n"
                "    {'name': 'event_id', 'type': 'BIGINT'},\n"
                "    {'name': 'amount', 'type': 'BIGINT'},\n"
                "    {'name': 'load_seq', 'type': 'BIGINT'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    if ctx.cursor_value is None:\n"
                "        return [\n"
                "            {'event_id': 1, 'amount': 100, 'load_seq': 1},\n"
                "            {'event_id': 2, 'amount': 200, 'load_seq': 1},\n"
                "        ]\n"
                "    return [\n"
                "        {'event_id': 2, 'amount': 250, 'load_seq': 1},\n"
                "        {'event_id': 3, 'amount': 300, 'load_seq': 1},\n"
                "    ]\n\n"
                "@loader(write_strategy='replace', depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                "    cursor = ctx.query(\n"
                "        f'SELECT event_id, amount FROM {events.destination} '\n"
                "        'ORDER BY event_id, amount'\n"
                "    )\n"
                "    rows = cursor.fetchall()\n"
                "    return [{'event_id': row[0], 'amount': row[1]} for row in rows]\n"
            ),
            expected_intermediate_rows=(("2", "250"), ("3", "300")),
            expected_terminal_rows=(("2", "250"), ("3", "300")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_intermediate_strategy_project_when_loading_twice_on_postgres_then_strategy_applies(
    tmp_path: Path,
    test_case: PostgresIntermediateDagStrategyE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_load_dag_strategy")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_dag_strategy_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_events\n    description: Test source raw_events.\n"
                "    managed: true\n"
                "    columns:\n"
                "      - name: event_id\n"
                "        type: BIGINT\n"
                "      - name: amount\n"
                "        type: BIGINT\n"
            ),
            node_files={
                **node_module_files("python/nodes/loaders/source_rows.py", test_case.loader_py)
            },
        ),
    )
    (project_dir / "sqlbuild_project.toml").write_text(
        build_postgres_project_toml(
            project_name="source_loader_dag_strategy_behavior",
            schema_name=schema_name,
            config=postgres_e2e_config,
        ),
        encoding="utf-8",
    )
    ensure_postgres_schema_ready(schema_name=schema_name, config=postgres_e2e_config)

    try:
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
        intermediate_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT event_id, amount FROM "
                f"{relation_name(schema_name=schema_name, name='__loader__fetch_events')} "
                "ORDER BY event_id, amount"
            ),
        )
        terminal_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT event_id, amount FROM "
                f"{relation_name(schema_name=schema_name, name='raw_events')} "
                "ORDER BY event_id, amount"
            ),
        )
        assert stringify_warehouse_rows(intermediate_rows) == test_case.expected_intermediate_rows
        assert stringify_warehouse_rows(terminal_rows) == test_case.expected_terminal_rows
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresSnapshotE2ETestCase(
            description="executes snapshot scd2 matrix on postgres",
            expected_current_rows_after_initial_build=(("1", "10", "basic", "2026-01-01", None),),
            expected_current_rows_after_recovery=(
                ("1", "10", "basic", "2026-01-01", "2026-01-02"),
                ("1", "10", "pro", "2026-01-02", None),
            ),
            expected_historical_timestamp_rows=(
                ("1", "basic", "2026-01-01", "2026-01-03"),
                ("1", "pro", "2026-01-03", None),
                ("2", "trial", "2026-01-02", None),
            ),
            expected_historical_check_rows=(
                ("1", "active", "2026-01-01", "2026-01-03"),
                ("1", "paused", "2026-01-03", None),
                ("2", "active", "2026-01-01", "2026-01-02"),
                ("2", "active", "2026-01-03", None),
            ),
            expected_failure_fragments=(
                "current_customer_snapshot",
                "delta audit for 'current_customer_snapshot' failed before target update",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_project_when_building_on_postgres_then_scd2_history_is_valid(
    tmp_path: Path,
    test_case: PostgresSnapshotE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_snapshot")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="postgres_snapshot_project",
        repo_files=build_real_warehouse_snapshot_project_files(
            project_toml=build_postgres_project_toml(
                project_name="postgres_snapshot_project",
                schema_name=schema_name,
                config=postgres_e2e_config,
            ),
        ),
    )
    ensure_postgres_schema_ready(schema_name=schema_name, config=postgres_e2e_config)

    try:
        initial_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"),
            project_dir=project_dir,
        )
        assert initial_result.returncode == 0, initial_result.stdout + initial_result.stderr
        assert_postgres_snapshot_matrix_rows(
            schema_name=schema_name,
            config=postgres_e2e_config,
            expected_current_rows=test_case.expected_current_rows_after_initial_build,
            expected_historical_timestamp_rows=test_case.expected_historical_timestamp_rows,
            expected_historical_check_rows=test_case.expected_historical_check_rows,
        )

        (project_dir / "sql" / "models" / "current_customers.sql").write_text(
            build_current_customers_model_sql(plan="blocked", updated_at="2026-01-02 00:00:00"),
            encoding="utf-8",
        )
        failure_result: subprocess.CompletedProcess[str] = run_sqb(
            command=(
                "--no-color",
                "build",
                "--concurrency",
                "4",
                "--select",
                "+current_customer_snapshot",
            ),
            project_dir=project_dir,
        )
        assert failure_result.returncode == 1, failure_result.stdout + failure_result.stderr
        for fragment in test_case.expected_failure_fragments:
            assert fragment in failure_result.stdout + failure_result.stderr
        assert_current_postgres_snapshot_rows_from_case(
            schema_name=schema_name,
            config=postgres_e2e_config,
            expected_rows=test_case.expected_current_rows_after_initial_build,
        )

        (project_dir / "sql" / "models" / "current_customers.sql").write_text(
            build_current_customers_model_sql(plan="pro", updated_at="2026-01-02 00:00:00"),
            encoding="utf-8",
        )
        recovery_result: subprocess.CompletedProcess[str] = run_sqb(
            command=(
                "--no-color",
                "build",
                "--concurrency",
                "4",
                "--select",
                "+current_customer_snapshot",
            ),
            project_dir=project_dir,
        )
        assert recovery_result.returncode == 0, recovery_result.stdout + recovery_result.stderr
        assert_current_postgres_snapshot_rows_from_case(
            schema_name=schema_name,
            config=postgres_e2e_config,
            expected_rows=test_case.expected_current_rows_after_recovery,
        )
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresSnapshotApplyE2ETestCase(
            description="applies existing-target snapshot changes on postgres",
            expected_current_check_rows=(
                ("1", "active", "False"),
                ("1", "paused", "True"),
                ("2", "active", "True"),
            ),
            expected_current_delete_rows=(
                ("1", "basic", "False"),
                ("1", "pro", "True"),
                ("2", "trial", "False"),
            ),
            expected_historical_timestamp_rows=(
                ("1", "basic", "2026-01-01", "2026-01-03"),
                ("1", "pro", "2026-01-03", None),
                ("2", "trial", "2026-01-01", "2026-01-04"),
            ),
            expected_historical_check_rows=(
                ("1", "active", "2026-01-01", "2026-01-03"),
                ("1", "paused", "2026-01-03", None),
                ("2", "active", "2026-01-01", "2026-01-02"),
                ("2", "active", "2026-01-03", None),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_existing_snapshot_targets_when_building_on_postgres_then_apply_sql_succeeds(
    tmp_path: Path,
    test_case: PostgresSnapshotApplyE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_snapshot_apply")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="postgres_snapshot_apply_project",
        repo_files=build_real_warehouse_existing_snapshot_project_files(
            project_toml=build_postgres_project_toml(
                project_name="postgres_snapshot_apply_project",
                schema_name=schema_name,
                config=postgres_e2e_config,
            ),
        ),
    )
    ensure_postgres_schema_ready(schema_name=schema_name, config=postgres_e2e_config)

    try:
        initial_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"),
            project_dir=project_dir,
        )
        assert initial_result.returncode == 0, initial_result.stdout + initial_result.stderr

        (project_dir / "sql" / "models" / "current_check_customers.sql").write_text(
            build_current_check_customers_model_sql(changed=True), encoding="utf-8"
        )
        (project_dir / "sql" / "models" / "current_delete_customers.sql").write_text(
            build_current_delete_customers_model_sql(changed=True), encoding="utf-8"
        )
        (project_dir / "sql" / "models" / "historical_timestamp_extracts.sql").write_text(
            build_historical_timestamp_extracts_model_sql(changed=True), encoding="utf-8"
        )
        (project_dir / "sql" / "models" / "historical_check_daily.sql").write_text(
            build_historical_check_daily_model_sql(changed=True), encoding="utf-8"
        )

        apply_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"),
            project_dir=project_dir,
        )
        assert apply_result.returncode == 0, apply_result.stdout + apply_result.stderr
        assert_postgres_snapshot_apply_rows(
            schema_name=schema_name,
            config=postgres_e2e_config,
            expected_current_check_rows=test_case.expected_current_check_rows,
            expected_current_delete_rows=test_case.expected_current_delete_rows,
            expected_historical_timestamp_rows=test_case.expected_historical_timestamp_rows,
            expected_historical_check_rows=test_case.expected_historical_check_rows,
        )
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


@pytest.mark.dbt
@pytest.mark.parametrize(
    "test_case",
    [
        PostgresDbtSeedChangeE2ETestCase(
            description="postgres dbt seed change and ordinary rerun execute the selected graph",
            expected_initial_total=75,
            expected_changed_total=115,
            expected_changed_fragments=("dbt execution", "fct_orders"),
            expected_rerun_fragments=("dbt execution", "fct_orders"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_postgres_dbt_seed_change_when_building_then_ordinary_rerun_executes_graph(
    tmp_path: Path,
    test_case: PostgresDbtSeedChangeE2ETestCase,
    postgres_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_dbt_seed_change")
    project_dir: Path = prepare_postgres_dbt_seed_change_workspace(
        tmp_path=tmp_path,
        schema_name=schema_name,
        config=postgres_e2e_config,
    )
    env: dict[str, str] = postgres_dbt_env(password=str(postgres_e2e_config["password"]))
    try:
        baseline: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "dbt", "build", "--select", "+fct_orders"),
            project_dir=project_dir,
            env=env,
        )
        assert baseline.returncode == 0, baseline.stdout + baseline.stderr
        assert fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=f"SELECT total_amount FROM {schema_name}.fct_orders",
        ) == ((test_case.expected_initial_total,),)

        append_postgres_dbt_seed_change_order(project_dir=project_dir, order_id=4, amount=40)

        changed: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "dbt", "build", "--select", "+fct_orders"),
            project_dir=project_dir,
            env=env,
        )
        assert changed.returncode == 0, changed.stdout + changed.stderr
        fragment: str
        for fragment in test_case.expected_changed_fragments:
            assert fragment in changed.stdout
        assert fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=f"SELECT total_amount FROM {schema_name}.fct_orders",
        ) == ((test_case.expected_changed_total,),)

        rerun: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "dbt", "build", "--select", "+fct_orders"),
            project_dir=project_dir,
            env=env,
        )
        assert rerun.returncode == 0, rerun.stdout + rerun.stderr
        for fragment in test_case.expected_rerun_fragments:
            assert fragment in rerun.stdout
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)
