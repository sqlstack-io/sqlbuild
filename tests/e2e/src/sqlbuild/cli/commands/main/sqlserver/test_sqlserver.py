from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.sqlserver._test_types import (
    SqlServerBuildE2ETestCase,
    SqlServerDbtProfileE2ETestCase,
    SqlServerDiffE2ETestCase,
    SqlServerLoaderRefusalE2ETestCase,
    SqlServerNodeResultE2ETestCase,
    SqlServerSnapshotApplyE2ETestCase,
    SqlServerSnapshotE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.sqlserver.helpers import (
    adapt_sqlserver_project_files,
    adapt_sqlserver_sql,
    assert_current_sqlserver_snapshot_rows_from_case,
    assert_sqlserver_snapshot_apply_rows,
    assert_sqlserver_snapshot_matrix_rows,
    build_sqlserver_config,
    build_sqlserver_project_toml,
    build_unique_schema_name,
    cleanup_sqlserver_schema,
    ensure_sqlserver_schema_ready,
    execute_sqlserver_sql,
    fetch_sqlserver_rows,
    prepare_sqlserver_waffle_shop,
    relation_name,
    write_sqlserver_waffle_sources,
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
)


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
            "sql/models/fact_orders.sql": (
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
                ("asset", "produce_result", "success"),
                ("check", "check_produce_result", "success"),
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
            "python/nodes/tasks/results/asset.py": (
                "from sqlbuild.assets import asset\n\n"
                "@asset\n"
                "def produce_result(ctx):\n"
                "    '''Test task produce_result.'''\n    return ctx.result(payload={'value': 42}, metadata={'source': 'sqlserver'})\n"
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
        write_sqlserver_waffle_sources(schema_name=schema_name, config=config)
        first_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )
        second_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

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

        (project_dir / "sql" / "models" / "current_customers.sql").write_text(
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

        (project_dir / "sql" / "models" / "current_customers.sql").write_text(
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

        (project_dir / "sql" / "models" / "current_check_customers.sql").write_text(
            build_current_check_customers_model_sql(changed=True), encoding="utf-8"
        )
        (project_dir / "sql" / "models" / "current_delete_customers.sql").write_text(
            adapt_sqlserver_sql(build_current_delete_customers_model_sql(changed=True)),
            encoding="utf-8",
        )
        (project_dir / "sql" / "models" / "historical_timestamp_extracts.sql").write_text(
            adapt_sqlserver_sql(build_historical_timestamp_extracts_model_sql(changed=True)),
            encoding="utf-8",
        )
        (project_dir / "sql" / "models" / "historical_check_daily.sql").write_text(
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


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerLoaderRefusalE2ETestCase(
            description="a loader is refused at compile, since SQLLoad can't write to SQL Server",
            expected_fragment="loads and loaders can't write to sqlserver yet",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_when_compiling_on_sqlserver_then_it_is_refused(
    tmp_path: Path,
    test_case: SqlServerLoaderRefusalE2ETestCase,
) -> None:
    config: dict[str, object] = build_sqlserver_config()
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sqlserver_loader",
        repo_files={
            "sqlbuild_project.toml": build_sqlserver_project_toml(
                project_name="sqlserver_loader",
                schema_name=build_unique_schema_name(prefix="sqb_loader"),
                config=config,
            ),
            "sql/sources/raw.yml": (
                "sources:\n  - name: raw_orders\n    description: Orders from a loader.\n"
                "    managed: true\n"
            ),
            "python/nodes/loaders/raw_orders/loader.py": (
                "from sqlbuild.loaders import loader\n\n\n@loader\n"
                "def raw_orders(ctx):\n    '''Orders.'''\n    return [{'id': 1}]\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert result.returncode != 0, result.stdout + result.stderr
    assert test_case.expected_fragment in result.stdout + result.stderr
