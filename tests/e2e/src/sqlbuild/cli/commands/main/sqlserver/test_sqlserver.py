from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.load.helpers import (
    build_loader_waffle_shop_project_files,
    build_schema_behavior_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    assert_optional_local_replay_rows,
    build_real_warehouse_local_replay_project_files,
    maybe_corrupt_scenario_snapshot_dialect,
)
from tests.e2e.src.sqlbuild.cli.commands.main.sqlserver._test_types import (
    SqlServerBuildE2ETestCase,
    SqlServerCloneE2ETestCase,
    SqlServerDbtProfileE2ETestCase,
    SqlServerDiffE2ETestCase,
    SqlServerIntermediateDagStrategyE2ETestCase,
    SqlServerLoaderWaffleShopE2ETestCase,
    SqlServerNodeResultE2ETestCase,
    SqlServerScenarioLocalReplayE2ETestCase,
    SqlServerSnapshotApplyE2ETestCase,
    SqlServerSnapshotE2ETestCase,
    SqlServerSourceDeferralE2ETestCase,
    SqlServerSourceLoaderDagE2ETestCase,
    SqlServerSourceLoaderE2ETestCase,
    SqlServerSourceLoaderStrategiesE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.sqlserver.helpers import (
    adapt_sqlserver_project_files,
    adapt_sqlserver_sql,
    assert_current_sqlserver_snapshot_rows_from_case,
    assert_sqlserver_snapshot_apply_rows,
    assert_sqlserver_snapshot_matrix_rows,
    build_sqlserver_config,
    build_sqlserver_project_toml,
    build_sqlserver_source_deferral_project_toml,
    build_unique_schema_name,
    cleanup_sqlserver_schema,
    ensure_sqlserver_schema_ready,
    execute_sqlserver_sql,
    fetch_sqlserver_rows,
    prepare_sqlserver_source_loader_strategies,
    prepare_sqlserver_waffle_shop,
    relation_name,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    assert_dbt_profile_lifecycle,
    build_current_check_customers_model_sql,
    build_current_customers_model_sql,
    build_current_delete_customers_model_sql,
    build_historical_check_daily_model_sql,
    build_historical_timestamp_extracts_model_sql,
    build_real_warehouse_existing_snapshot_project_files,
    build_real_warehouse_snapshot_project_files,
    prepare_inline_project,
    run_sqb,
    stringify_warehouse_rows,
)


@pytest.mark.parametrize(
    "test_case",
    (
        SqlServerCloneE2ETestCase(
            description="managed source is copied before dependent view recreation",
            expected_stdout_fragments=(
                "raw_customers",
                "copied",
                "stg_customers",
                "recreated_view",
            ),
            expected_rows=((1, "Ada"), (2, "Grace")),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_managed_source_when_cloning_sqlserver_then_source_precedes_dependent_view(
    test_case: SqlServerCloneE2ETestCase,
    tmp_path: Path,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    schema_base: str = build_unique_schema_name(prefix="sqb_clone_source")
    dev_schema_name: str = f"{schema_base}_dev"
    prod_schema_name: str = f"{schema_base}_prod"
    dev_loader_schema_name: str = f"{schema_base}_raw_dev"
    prod_loader_schema_name: str = f"{schema_base}_raw_prod"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sqlserver_managed_source_clone",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "sqlserver_managed_source_clone"\n'
                'adapter = "sqlserver"\n'
                'default_target = "dev"\n\n'
                "[connection]\n"
                f'host = "{config["host"]}"\n'
                f"port = {config['port']}\n"
                f'database = "{config["database"]}"\n'
                f'user = "{config["user"]}"\n'
                f'password = "{config["password"]}"\n\n'
                "[targets.dev]\n"
                f'schema = "{dev_schema_name}"\n\n'
                f'loader_schema = "{dev_loader_schema_name}"\n\n'
                "[targets.dev.clone]\n"
                "allow_as_clone_origin = true\n\n"
                "[targets.prod]\n"
                f'schema = "{prod_schema_name}"\n\n'
                f'loader_schema = "{prod_loader_schema_name}"\n\n'
                "[targets.prod.clone]\n"
                "allow_as_clone_destination = true\n"
            ),
            "python/loaders/raw.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader\n"
                "def raw_customers(ctx):\n"
                "    '''Test loader raw_customers.'''\n    return []\n"
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_customers\n    description: Test source raw_customers.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: customer_id\n"
                "        type: INTEGER\n"
                "      - name: first_name\n"
                "        type: VARCHAR\n"
            ),
            "models/stg_customers.sql": (
                "MODEL (description 'Test model stg_customers.', materialized view);\n\n"
                'SELECT customer_id, first_name FROM __source("raw_customers")\n'
            ),
        },
    )
    try:
        ensure_sqlserver_schema_ready(schema_name=dev_schema_name, config=config)
        ensure_sqlserver_schema_ready(schema_name=prod_schema_name, config=config)
        ensure_sqlserver_schema_ready(schema_name=dev_loader_schema_name, config=config)
        ensure_sqlserver_schema_ready(schema_name=prod_loader_schema_name, config=config)
        execute_sqlserver_sql(
            sql=(
                "SELECT customer_id, first_name "
                f"INTO {relation_name(schema_name=dev_loader_schema_name, name='raw_customers')} "
                "FROM (VALUES (1, 'Ada'), (2, 'Grace')) AS source(customer_id, first_name)"
            ),
            config=config,
        )
        execute_sqlserver_sql(
            sql=(
                f"CREATE VIEW {relation_name(schema_name=dev_schema_name, name='stg_customers')} "
                "AS SELECT customer_id, first_name "
                f"FROM {relation_name(schema_name=dev_loader_schema_name, name='raw_customers')}"
            ),
            config=config,
        )

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "clone", "--from", "dev", "--to", "prod"),
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        expected_fragment: str
        for expected_fragment in test_case.expected_stdout_fragments:
            assert expected_fragment in result.stdout
        assert (
            fetch_sqlserver_rows(
                sql=(
                    "SELECT customer_id, first_name FROM "
                    f"{relation_name(schema_name=prod_schema_name, name='stg_customers')} "
                    "ORDER BY customer_id"
                ),
                config=config,
            )
            == test_case.expected_rows
        )
    finally:
        cleanup_sqlserver_schema(schema_name=prod_loader_schema_name, config=config)
        cleanup_sqlserver_schema(schema_name=dev_loader_schema_name, config=config)
        cleanup_sqlserver_schema(schema_name=prod_schema_name, config=config)
        cleanup_sqlserver_schema(schema_name=dev_schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    (
        SqlServerDiffE2ETestCase(
            description="schema-only diff ignores row drift",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--schema-only",
                "--select",
                "fact_orders",
            ),
            mutation_sql=("UPDATE {dev} SET amount_cents = 105 WHERE order_id = 1",),
            expected_stdout_fragments=("fact_orders", "No schema differences."),
            expected_return_code=0,
        ),
        SqlServerDiffE2ETestCase(
            description="full diff reports row mismatch",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "fact_orders",
            ),
            mutation_sql=("UPDATE {dev} SET amount_cents = 105 WHERE order_id = 1",),
            expected_stdout_fragments=(
                "amount_cents",
                "mismatches=1",
                "order_id=1 | 100 -> 105",
            ),
            expected_return_code=1,
        ),
        SqlServerDiffE2ETestCase(
            description="same-target full diff reports identical rows",
            command=(
                "--no-color",
                "diff",
                "dev:dev",
                "--full",
                "--select",
                "fact_orders",
            ),
            mutation_sql=(),
            expected_stdout_fragments=(
                "fact_orders",
                "No schema differences.",
                "No changed columns.",
            ),
            expected_return_code=0,
        ),
        SqlServerDiffE2ETestCase(
            description="bounded diff filters rows below integer cursor bound",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--bounded",
                "2",
                "--select",
                "fact_orders",
            ),
            mutation_sql=(
                "UPDATE {dev} SET amount_cents = 105 WHERE order_id = 1",
                "UPDATE {dev} SET amount_cents = 205 WHERE order_id = 2",
            ),
            expected_stdout_fragments=(
                "amount_cents",
                "mismatches=1",
                "order_id=2 | 200 -> 205",
            ),
            expected_absent_stdout_fragments=("order_id=1 | 100 -> 105",),
            expected_return_code=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sqlserver_targets_when_running_diff_then_all_modes_execute_end_to_end(
    test_case: SqlServerDiffE2ETestCase,
    tmp_path: Path,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    prod_schema: str = build_unique_schema_name(prefix="sqb_diff_prod")
    dev_schema: str = build_unique_schema_name(prefix="sqb_diff_dev")
    ensure_sqlserver_schema_ready(schema_name=prod_schema, config=config)
    ensure_sqlserver_schema_ready(schema_name=dev_schema, config=config)
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sqlserver_diff_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "sqlserver_diff_project"\n'
                'adapter = "sqlserver"\n'
                'default_target = "dev"\n\n'
                "[connection]\n"
                f'host = "{config["host"]}"\n'
                f"port = {config['port']}\n"
                f'database = "{config["database"]}"\n'
                f'user = "{config["user"]}"\n'
                f'password = "{config["password"]}"\n\n'
                "[targets.prod]\n"
                f'schema = "{prod_schema}"\n\n'
                "[targets.dev]\n"
                f'schema = "{dev_schema}"\n'
            ),
            "models/fact_orders.sql": (
                "MODEL (description 'Test model fact_orders.',\n"
                "  materialized table,\n"
                "  unique_key [order_id],\n"
                "  cursor order_id,\n"
                "  cursor_type integer,\n"
                ");\n\n"
                "SELECT 1 AS order_id, 100 AS amount_cents\n"
            ),
        },
    )
    prod_relation: str = relation_name(schema_name=prod_schema, name="fact_orders")
    dev_relation: str = relation_name(schema_name=dev_schema, name="fact_orders")

    try:
        for relation in (prod_relation, dev_relation):
            execute_sqlserver_sql(
                config=config,
                sql=f"CREATE TABLE {relation} (order_id INT, amount_cents INT); "
                f"INSERT INTO {relation} VALUES (1, 100), (2, 200), (3, 300)",
            )
        for mutation_sql in test_case.mutation_sql:
            execute_sqlserver_sql(sql=mutation_sql.format(dev=dev_relation), config=config)

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in result.stdout, result.stdout + result.stderr
        for fragment in test_case.expected_absent_stdout_fragments:
            assert fragment not in result.stdout, result.stdout + result.stderr
    finally:
        cleanup_sqlserver_schema(schema_name=dev_schema, config=config)
        cleanup_sqlserver_schema(schema_name=prod_schema, config=config)


@pytest.mark.dbt
@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerDbtProfileE2ETestCase(
            description="dbt init generated project builds through SQL Server dbt profile",
            schema_prefix="sqlbuild_dbt_profile",
            expected_toml_fragments=(
                'adapter = "sqlserver"',
                'source = "dbt_profile"',
                'profile = "analytics"',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlserver_dbt_profile_when_running_dbt_init_then_builds_profile_lifecycle(
    tmp_path: Path,
    test_case: SqlServerDbtProfileE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix=test_case.schema_prefix)
    config: dict[str, object] = build_sqlserver_config()
    escaped_schema_name: str = schema_name.replace("'", "''")
    try:
        ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)
        assert_dbt_profile_lifecycle(
            tmp_path=tmp_path,
            profiles_yml=(
                "analytics:\n"
                "  target: dev\n"
                "  outputs:\n"
                "    dev:\n"
                "      type: sqlserver\n"
                "      driver: ODBC Driver 18 for SQL Server\n"
                f"      server: {config['host']}\n"
                f"      port: {config['port']}\n"
                f"      database: {config['database']}\n"
                f"      user: {config['user']}\n"
                f"      password: {config['password']}\n"
                f"      schema: {schema_name}\n"
                "      trust_cert: true\n"
            ),
            env=None,
            fetch_rows=lambda sql: fetch_sqlserver_rows(config=config, sql=sql),
            no_profile_tables_exist=lambda: (
                fetch_sqlserver_rows(
                    config=config,
                    sql=(
                        "SELECT t.name FROM sys.tables t "
                        "JOIN sys.schemas s ON t.schema_id = s.schema_id "
                        f"WHERE s.name = '{escaped_schema_name}' "
                        "AND t.name IN ('dbt_orders', 'downstream_orders') ORDER BY t.name"
                    ),
                )
                == ()
            ),
            dbt_orders_sql=(
                f"SELECT order_id FROM {relation_name(schema_name=schema_name, name='dbt_orders')} "
                "ORDER BY order_id"
            ),
            downstream_orders_sql=(
                "SELECT order_id FROM "
                f"{relation_name(schema_name=schema_name, name='downstream_orders')} "
                "ORDER BY order_id"
            ),
            expected_toml_fragments=test_case.expected_toml_fragments,
            unexpected_toml_fragments=(str(config["password"]),),
        )
    finally:
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerNodeResultE2ETestCase(
            description="direct node results persist and read on SQL Server",
            expected_rows=(
                ("check", "check_produce_result", "success"),
                ("asset", "produce_result", "success"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_result_when_running_check_on_sqlserver_then_persists_node_results(
    tmp_path: Path,
    test_case: SqlServerNodeResultE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    schema_name: str = build_unique_schema_name(prefix="sqb_node_results")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sqlserver_node_results",
        repo_files={
            "sqlbuild_project.toml": build_sqlserver_project_toml(
                project_name="sqlserver_node_results",
                schema_name=schema_name,
                config=config,
            ),
            "python/tasks/results.py": (
                "from sqlbuild.assets import asset\n\n"
                "@asset\n"
                "def produce_result(ctx):\n"
                "    '''Test task produce_result.'''\n    return ctx.result(payload={'value': 42}, metadata={'source': 'sqlserver'})\n"
            ),
            "python/checks/results.py": (
                "from sqlbuild.checks import check\n"
                "from python.tasks.results import produce_result\n\n"
                "@check(depends_on=produce_result)\n"
                "def check_produce_result(ctx):\n"
                "    '''Test check check_produce_result.'''\n    return ctx.result_of(node_function=produce_result).payload['value'] == 42\n"
            ),
        },
    )
    ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)

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
        rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            sql=(
                "SELECT node_type, node_name, status "
                f"FROM {relation_name(schema_name=schema_name, name='_sqlbuild_node_results')} "
                "WHERE node_name IN ('produce_result', 'check_produce_result') "
                "ORDER BY node_type, node_name"
            ),
            config=config,
        )
        assert rows == test_case.expected_rows
    finally:
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerBuildE2ETestCase(
            description="waffle shop full build succeeds on SQL Server",
            command=("--no-color", "build", "--concurrency", "4"),
            expected_table_name="fact_orders",
            expected_row_count=10,
            expected_stdout_fragments=("Execution", "OK"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_running_full_build_on_sqlserver_then_expected_table_exists(
    tmp_path: Path,
    test_case: SqlServerBuildE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    project_dir: Path
    schema_name: str
    project_dir, schema_name = prepare_sqlserver_waffle_shop(tmp_path=tmp_path, config=config)
    ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)

    try:
        first_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )
        cursor_type_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            sql=(
                "SELECT data_type FROM information_schema.columns "
                f"WHERE table_schema = '{schema_name}' AND table_name = 'raw_orders' "
                "AND column_name = 'ordered_at'"
            ),
            config=config,
        )
        second_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert cursor_type_rows == (("datetime",),)
        for result in (first_result, second_result):
            assert result.returncode == test_case.expected_return_code, (
                result.stdout + result.stderr
            )
            for fragment in test_case.expected_stdout_fragments:
                assert fragment in result.stdout
        rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            sql=(
                "SELECT COUNT(*) FROM "
                f"{relation_name(schema_name=schema_name, name=test_case.expected_table_name)}"
            ),
            config=config,
        )
        assert int(str(rows[0][0])) == test_case.expected_row_count
        source_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            sql=(
                f"SELECT COUNT(*) FROM {relation_name(schema_name=schema_name, name='raw_orders')}"
            ),
            config=config,
        )
        assert int(str(source_rows[0][0])) == test_case.expected_row_count
    finally:
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerScenarioLocalReplayE2ETestCase(
            description="captures SQL Server fixtures and replays transpilable SQL locally",
            model_sql=(
                "MODEL (description 'Test model.', materialized table);\n\n"
                "SELECT\n"
                "  customer_id,\n"
                "  CAST(CAST(event_ts AS DATE) AS DATETIME2) AS event_day,\n"
                "  SUM(CASE WHEN amount_cents >= 1000 THEN amount_cents ELSE 0 END)"
                " AS large_amount_cents,\n"
                "  COUNT(*) AS event_count\n"
                'FROM __source("raw_events")\n'
                "GROUP BY customer_id, CAST(CAST(event_ts AS DATE) AS DATETIME2)\n"
            ),
            scenario_sql=(
                "SCENARIO (description 'Test scenario.');\n\n"
                "WITH\n"
                "__source__raw_events AS (\n"
                "  SELECT 10 AS customer_id, CAST('2026-01-01 08:15:00' AS DATETIME2)"
                " AS event_ts, 1500 AS amount_cents\n"
                "  UNION ALL\n"
                "  SELECT 10 AS customer_id, CAST('2026-01-01 10:30:00' AS DATETIME2)"
                " AS event_ts, 500 AS amount_cents\n"
                "),\n"
                "__expected__event_rollup AS (\n"
                "  SELECT 10 AS customer_id,"
                " CAST(CAST('2026-01-01 00:00:00' AS DATE) AS DATETIME2) AS event_day,"
                " 1500 AS large_amount_cents, 2 AS event_count\n"
                ")\n"
                "SELECT 1\n"
            ),
            expected_stdout_fragments=(
                "transpilable_event_rollup",
                "PASS",
                "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
            ),
            expected_local_rows=((10, 1500, 2),),
            local_rows_sql=(
                "SELECT customer_id, large_amount_cents, event_count "
                "FROM __sqb_local__model__event_rollup ORDER BY customer_id"
            ),
        ),
        SqlServerScenarioLocalReplayE2ETestCase(
            description="reports SQL Server local transpilation failures as X607",
            scenario_name="local_transpile_error",
            model_sql=(
                "MODEL (description 'Test model.', materialized table);\n\n"
                "SELECT customer_id, amount_cents\n"
                'FROM __source("raw_events")\n'
            ),
            scenario_sql=(
                "SCENARIO (description 'Test scenario.');\n\n"
                "WITH\n"
                "__source__raw_events AS (\n"
                "  SELECT 10 AS customer_id, 1500 AS amount_cents\n"
                "),\n"
                "__expected__event_rollup AS (\n"
                "  SELECT 10 AS customer_id, 1500 AS amount_cents\n"
                ")\n"
                "SELECT 1\n"
            ),
            expected_stdout_fragments=(
                "local_transpile_error",
                "ERROR",
                "error[X607]",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
            expected_return_code=1,
            corrupt_capture_dialect=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sqlserver_scenario_capture_when_replaying_locally_then_transpilable_sql_passes(
    tmp_path: Path,
    test_case: SqlServerScenarioLocalReplayE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    schema_name: str = build_unique_schema_name(prefix="sqb_scenario_local")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sqlserver_scenario_local_replay",
        repo_files=build_real_warehouse_local_replay_project_files(
            project_toml=build_sqlserver_project_toml(
                project_name="sqlserver_scenario_local_replay",
                schema_name=schema_name,
                config=config,
            ),
            model_sql=test_case.model_sql,
            scenario_sql=test_case.scenario_sql,
            scenario_name=test_case.scenario_name,
        ),
    )
    ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)

    try:
        capture_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "scenario", "capture", test_case.scenario_name),
            project_dir=project_dir,
        )
        assert capture_result.returncode == 0, capture_result.stdout + capture_result.stderr
        maybe_corrupt_scenario_snapshot_dialect(
            project_dir=project_dir,
            scenario_name=test_case.scenario_name,
            enabled=test_case.corrupt_capture_dialect,
        )

        replay_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "scenario", "test", test_case.scenario_name, "--local"),
            project_dir=project_dir,
        )

        assert replay_result.returncode == test_case.expected_return_code, (
            replay_result.stdout + replay_result.stderr
        )
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in replay_result.stdout
        assert_optional_local_replay_rows(
            project_dir=project_dir,
            scenario_name=test_case.scenario_name,
            local_rows_sql=test_case.local_rows_sql,
            expected_local_rows=test_case.expected_local_rows,
        )
    finally:
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerSourceLoaderE2ETestCase(
            description="source loader build writes and reads SQL Server rows",
            expected_rows=(("7", "loaded-dev"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_project_when_building_on_sqlserver_then_model_reads_loaded_rows(
    tmp_path: Path,
    test_case: SqlServerSourceLoaderE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    schema_name: str = build_unique_schema_name(prefix="sqb_e2e_loader")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sqlserver_source_loader",
        repo_files={
            "sqlbuild_project.toml": build_sqlserver_project_toml(
                project_name="sqlserver_source_loader",
                schema_name=schema_name,
                config=config,
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
                "      - name: status\n"
                "        type: NVARCHAR(100)\n"
            ),
            "python/loaders/raw_orders.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    return [{'order_id': 7, 'status': 'loaded-dev'}]\n"
            ),
            "models/stg_orders.sql": (
                'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT order_id, status FROM __source("raw_orders")'
            ),
        },
    )
    ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--select", "stg_orders"),
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT order_id, status FROM "
                f"{relation_name(schema_name=schema_name, name='stg_orders')} "
                "ORDER BY order_id"
            ),
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
    finally:
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerSourceDeferralE2ETestCase(
            description="SQL Server loader writes dev while model reads prod deferred source",
            expected_model_rows=(("99", "prod-source"),),
            expected_loader_rows=(("7", "loaded-dev"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_deferral_env_when_building_on_sqlserver_then_reads_prod_and_writes_dev(
    tmp_path: Path,
    test_case: SqlServerSourceDeferralE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    dev_schema_name: str = build_unique_schema_name(prefix="sqlbuild_defer_dev")
    prod_schema_name: str = build_unique_schema_name(prefix="sqlbuild_defer_prod")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sqlserver_source_deferral",
        repo_files={
            "sqlbuild_project.toml": build_sqlserver_source_deferral_project_toml(
                project_name="sqlserver_source_deferral",
                dev_schema_name=dev_schema_name,
                prod_schema_name=prod_schema_name,
                config=config,
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
                "      - name: status\n"
                "        type: NVARCHAR(100)\n"
            ),
            "python/loaders/raw_orders.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    return [{'order_id': 7, 'status': 'loaded-dev'}]\n"
            ),
            "models/stg_orders.sql": (
                'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT order_id, status FROM __source("raw_orders")'
            ),
        },
    )
    ensure_sqlserver_schema_ready(schema_name=dev_schema_name, config=config)
    ensure_sqlserver_schema_ready(schema_name=prod_schema_name, config=config)

    try:
        execute_sqlserver_sql(
            config=config,
            sql=(
                f"CREATE TABLE {relation_name(schema_name=prod_schema_name, name='raw_orders')} "
                "(order_id INT, status NVARCHAR(100))"
            ),
        )
        execute_sqlserver_sql(
            config=config,
            sql=(
                f"INSERT INTO {relation_name(schema_name=prod_schema_name, name='raw_orders')} "
                "VALUES (99, 'prod-source')"
            ),
        )

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--select", "stg_orders"),
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        model_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT order_id, status FROM "
                f"{relation_name(schema_name=dev_schema_name, name='stg_orders')} "
                "ORDER BY order_id"
            ),
        )
        loader_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT order_id, status FROM "
                f"{relation_name(schema_name=dev_schema_name, name='raw_orders')} "
                "ORDER BY order_id"
            ),
        )
        assert stringify_warehouse_rows(model_rows) == test_case.expected_model_rows
        assert stringify_warehouse_rows(loader_rows) == test_case.expected_loader_rows
    finally:
        cleanup_sqlserver_schema(schema_name=dev_schema_name, config=config)
        cleanup_sqlserver_schema(schema_name=prod_schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerLoaderWaffleShopE2ETestCase(
            description="loader focused waffle shop grows across repeated SQL Server builds",
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
def test_given_loader_waffle_shop_when_building_on_sqlserver_then_dag_grows_models(
    tmp_path: Path,
    test_case: SqlServerLoaderWaffleShopE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    schema_name: str = build_unique_schema_name(prefix="sqb_load_waffle")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="loader_waffle_shop",
        repo_files=build_loader_waffle_shop_project_files(
            project_toml=build_sqlserver_project_toml(
                project_name="loader_waffle_shop",
                schema_name=schema_name,
                config=config,
            )
        ),
    )
    ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)

    try:
        for _ in range(2):
            result: subprocess.CompletedProcess[str] = run_sqb(
                command=("--no-color", "build", "--select", "+customer_revenue"),
                project_dir=project_dir,
            )
            assert result.returncode == test_case.expected_return_code, (
                result.stdout + result.stderr
            )

        rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT customer_id, plan_name, revenue_cents, order_count FROM "
                f"{relation_name(schema_name=schema_name, name='customer_revenue')} "
                "ORDER BY customer_id"
            ),
        )
        event_count_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT COUNT(*) FROM "
                f"{relation_name(schema_name=schema_name, name='__loader__fetch_order_events')}"
            ),
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
        assert int(str(event_count_rows[0][0])) == test_case.expected_event_count
    finally:
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerSourceLoaderStrategiesE2ETestCase(
            description="source loader strategies apply expected rows on SQL Server",
            expected_countries=(("1", "US", "United States"), ("2", "CA", "Canada")),
            expected_webhook_event_counts=(("101", "signup", "2"), ("102", "checkout", "2")),
            expected_order_events=(("201", "1000"), ("202", "2500"), ("203", "3000")),
            expected_customers=(("1", "pro"), ("2", "trial"), ("3", "enterprise")),
            expected_loader_status=(("1", "loaded", "self_managed"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_strategy_project_when_loading_twice_on_sqlserver_then_write_modes_apply(
    tmp_path: Path,
    test_case: SqlServerSourceLoaderStrategiesE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    project_dir: Path
    schema_name: str
    project_dir, schema_name = prepare_sqlserver_source_loader_strategies(
        tmp_path=tmp_path,
        config=config,
    )
    ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)

    try:
        first_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "load", "--concurrency", "4"),
            project_dir=project_dir,
        )
        second_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "load", "--concurrency", "4"),
            project_dir=project_dir,
        )

        assert first_result.returncode == test_case.expected_return_code, (
            first_result.stdout + first_result.stderr
        )
        assert second_result.returncode == test_case.expected_return_code, (
            second_result.stdout + second_result.stderr
        )
        countries: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT country_id, country_code, country_name FROM "
                f"{relation_name(schema_name=schema_name, name='raw_countries')} "
                "ORDER BY country_id"
            ),
        )
        webhook_event_counts: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT event_id, event_name, COUNT(*) FROM "
                f"{relation_name(schema_name=schema_name, name='raw_webhook_events')} "
                "GROUP BY event_id, event_name ORDER BY event_id"
            ),
        )
        order_events: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT event_id, amount_cents FROM "
                f"{relation_name(schema_name=schema_name, name='raw_order_events')} "
                "ORDER BY event_id"
            ),
        )
        customers: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT customer_id, plan_name FROM "
                f"{relation_name(schema_name=schema_name, name='raw_customers')} "
                "ORDER BY customer_id"
            ),
        )
        loader_status: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
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
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerSourceLoaderDagE2ETestCase(
            description="chained source loader runs on SQL Server",
            command=("--no-color", "load", "--select", "+raw_events"),
            expected_rows=(("1", "loaded"), ("2", "loaded")),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_loading_on_sqlserver_then_runs_loader_dag(
    tmp_path: Path,
    test_case: SqlServerSourceLoaderDagE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    schema_name: str = build_unique_schema_name(prefix="sqb_load_dag")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_dag_behavior",
        repo_files=build_schema_behavior_project_files(
            source_yaml=(
                "sources:\n"
                "  - name: raw_events\n    description: Test source raw_events.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: event_id\n"
                "        type: INTEGER\n"
                "      - name: status\n"
                "        type: NVARCHAR(100)\n"
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
                "    cursor = ctx.query(\n"
                "        f'SELECT event_id FROM {events.destination} ORDER BY event_id'\n"
                "    )\n"
                "    rows = cursor.fetchall()\n"
                "    return [{'event_id': row[0], 'status': 'loaded'} for row in rows]\n"
            ),
        ),
    )
    (project_dir / "sqlbuild_project.toml").write_text(
        build_sqlserver_project_toml(
            project_name="source_loader_dag_behavior",
            schema_name=schema_name,
            config=config,
        ),
        encoding="utf-8",
    )
    ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT event_id, status FROM "
                f"{relation_name(schema_name=schema_name, name='raw_events')} "
                "ORDER BY event_id"
            ),
        )
        intermediate_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT COUNT(*) FROM "
                f"{relation_name(schema_name=schema_name, name='__loader__fetch_events')}"
            ),
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
        assert int(str(intermediate_rows[0][0])) == 2
    finally:
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerIntermediateDagStrategyE2ETestCase(
            description="SQL Server append intermediate accumulates rows across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='append', cursor_column='load_seq', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "    {'name': 'amount', 'type': 'INTEGER'},\n"
                "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    if ctx.current_cursor_value is None:\n"
                "        next_seq = 1\n"
                "    else:\n"
                "        next_seq = ctx.current_cursor_value + 1\n"
                "    return [\n"
                "        {'event_id': next_seq, 'amount': next_seq * 100, 'load_seq': next_seq}\n"
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
            expected_intermediate_rows=(("1", "100"), ("2", "200")),
            expected_terminal_rows=(("1", "100"), ("2", "200")),
        ),
        SqlServerIntermediateDagStrategyE2ETestCase(
            description="SQL Server merge intermediate updates and adds rows across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(\n"
                "    write_strategy='merge',\n"
                "    unique_key='event_id',\n"
                "    cursor_column='load_seq',\n"
                "    columns=[\n"
                "        {'name': 'event_id', 'type': 'INTEGER'},\n"
                "        {'name': 'amount', 'type': 'INTEGER'},\n"
                "        {'name': 'load_seq', 'type': 'INTEGER'},\n"
                "    ],\n"
                ")\n"
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
            expected_intermediate_rows=(("1", "150"), ("2", "200"), ("3", "300")),
            expected_terminal_rows=(("1", "150"), ("2", "200"), ("3", "300")),
        ),
        SqlServerIntermediateDagStrategyE2ETestCase(
            description="SQL Server delete insert intermediate replaces cursor window across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='delete_insert', cursor_column='load_seq', columns=[\n"
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
                "        {'event_id': 2, 'amount': 250, 'load_seq': 1},\n"
                "        {'event_id': 3, 'amount': 300, 'load_seq': 1},\n"
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
            expected_intermediate_rows=(("2", "250"), ("3", "300")),
            expected_terminal_rows=(("2", "250"), ("3", "300")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_intermediate_strategy_project_when_loading_twice_on_sqlserver_then_strategy_applies(
    tmp_path: Path,
    test_case: SqlServerIntermediateDagStrategyE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    schema_name: str = build_unique_schema_name(prefix="sqb_load_dag_strategy")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_dag_strategy_behavior",
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
    (project_dir / "sqlbuild_project.toml").write_text(
        build_sqlserver_project_toml(
            project_name="source_loader_dag_strategy_behavior",
            schema_name=schema_name,
            config=config,
        ),
        encoding="utf-8",
    )
    ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)

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
        intermediate_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT event_id, amount FROM "
                f"{relation_name(schema_name=schema_name, name='__loader__fetch_events')} "
                "ORDER BY event_id, amount"
            ),
        )
        terminal_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
            config=config,
            sql=(
                "SELECT event_id, amount FROM "
                f"{relation_name(schema_name=schema_name, name='raw_events')} "
                "ORDER BY event_id, amount"
            ),
        )
        assert stringify_warehouse_rows(intermediate_rows) == test_case.expected_intermediate_rows
        assert stringify_warehouse_rows(terminal_rows) == test_case.expected_terminal_rows
    finally:
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerSnapshotE2ETestCase(
            description="executes snapshot scd2 matrix on SQL Server",
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
def test_given_snapshot_project_when_building_on_sqlserver_then_scd2_history_is_valid(
    tmp_path: Path,
    test_case: SqlServerSnapshotE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    schema_name: str = build_unique_schema_name(prefix="sqb_snapshot")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sqlserver_snapshot_project",
        repo_files=adapt_sqlserver_project_files(
            build_real_warehouse_snapshot_project_files(
                project_toml=build_sqlserver_project_toml(
                    project_name="sqlserver_snapshot_project",
                    schema_name=schema_name,
                    config=config,
                ),
            )
        ),
    )
    ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)

    try:
        initial_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"),
            project_dir=project_dir,
        )
        assert initial_result.returncode == 0, initial_result.stdout + initial_result.stderr
        assert_sqlserver_snapshot_matrix_rows(
            schema_name=schema_name,
            config=config,
            expected_current_rows=test_case.expected_current_rows_after_initial_build,
            expected_historical_timestamp_rows=test_case.expected_historical_timestamp_rows,
            expected_historical_check_rows=test_case.expected_historical_check_rows,
        )

        (project_dir / "models" / "current_customers.sql").write_text(
            adapt_sqlserver_sql(
                build_current_customers_model_sql(plan="blocked", updated_at="2026-01-02 00:00:00")
            ),
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
        assert_current_sqlserver_snapshot_rows_from_case(
            schema_name=schema_name,
            config=config,
            expected_rows=test_case.expected_current_rows_after_initial_build,
        )

        (project_dir / "models" / "current_customers.sql").write_text(
            adapt_sqlserver_sql(
                build_current_customers_model_sql(plan="pro", updated_at="2026-01-02 00:00:00")
            ),
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
        assert_current_sqlserver_snapshot_rows_from_case(
            schema_name=schema_name,
            config=config,
            expected_rows=test_case.expected_current_rows_after_recovery,
        )
    finally:
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerSnapshotApplyE2ETestCase(
            description="applies existing-target snapshot changes on SQL Server",
            expected_current_check_rows=(
                ("1", "active", "0"),
                ("1", "paused", "1"),
                ("2", "active", "1"),
            ),
            expected_current_delete_rows=(
                ("1", "basic", "0"),
                ("1", "pro", "1"),
                ("2", "trial", "0"),
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
def test_given_existing_snapshot_targets_when_building_on_sqlserver_then_apply_sql_succeeds(
    tmp_path: Path,
    test_case: SqlServerSnapshotApplyE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    schema_name: str = build_unique_schema_name(prefix="sqb_snapshot_apply")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sqlserver_snapshot_apply_project",
        repo_files=adapt_sqlserver_project_files(
            build_real_warehouse_existing_snapshot_project_files(
                project_toml=build_sqlserver_project_toml(
                    project_name="sqlserver_snapshot_apply_project",
                    schema_name=schema_name,
                    config=config,
                ),
            )
        ),
    )
    ensure_sqlserver_schema_ready(schema_name=schema_name, config=config)

    try:
        initial_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"),
            project_dir=project_dir,
        )
        assert initial_result.returncode == 0, initial_result.stdout + initial_result.stderr

        (project_dir / "models" / "current_check_customers.sql").write_text(
            build_current_check_customers_model_sql(changed=True), encoding="utf-8"
        )
        (project_dir / "models" / "current_delete_customers.sql").write_text(
            adapt_sqlserver_sql(build_current_delete_customers_model_sql(changed=True)),
            encoding="utf-8",
        )
        (project_dir / "models" / "historical_timestamp_extracts.sql").write_text(
            adapt_sqlserver_sql(build_historical_timestamp_extracts_model_sql(changed=True)),
            encoding="utf-8",
        )
        (project_dir / "models" / "historical_check_daily.sql").write_text(
            adapt_sqlserver_sql(build_historical_check_daily_model_sql(changed=True)),
            encoding="utf-8",
        )

        apply_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"),
            project_dir=project_dir,
        )
        assert apply_result.returncode == 0, apply_result.stdout + apply_result.stderr
        assert_sqlserver_snapshot_apply_rows(
            schema_name=schema_name,
            config=config,
            expected_current_check_rows=test_case.expected_current_check_rows,
            expected_current_delete_rows=test_case.expected_current_delete_rows,
            expected_historical_timestamp_rows=test_case.expected_historical_timestamp_rows,
            expected_historical_check_rows=test_case.expected_historical_check_rows,
        )
    finally:
        cleanup_sqlserver_schema(schema_name=schema_name, config=config)
