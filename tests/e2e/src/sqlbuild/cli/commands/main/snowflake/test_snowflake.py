from __future__ import annotations

import json
import subprocess
from collections import defaultdict
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.dbt.helpers import (
    assert_dbt_local_replay_rows,
)
from tests.e2e.src.sqlbuild.cli.commands.main.load.helpers import (
    build_loader_waffle_shop_project_files,
    build_schema_behavior_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    assert_optional_local_replay_rows,
    build_real_warehouse_local_replay_project_files,
    build_real_warehouse_remote_scenario_project_files,
    maybe_corrupt_scenario_snapshot_dialect,
)
from tests.e2e.src.sqlbuild.cli.commands.main.snowflake._test_types import (
    SnowflakeBuildE2ETestCase,
    SnowflakeCliTestCase,
    SnowflakeCloneE2ETestCase,
    SnowflakeDbtProfileE2ETestCase,
    SnowflakeDbtScenarioLocalReplayE2ETestCase,
    SnowflakeDiffE2ETestCase,
    SnowflakeIntermediateDagStrategyE2ETestCase,
    SnowflakeNodeResultE2ETestCase,
    SnowflakeScenarioLocalReplayE2ETestCase,
    SnowflakeScenarioRemoteE2ETestCase,
    SnowflakeSnapshotApplyE2ETestCase,
    SnowflakeSnapshotE2ETestCase,
    SnowflakeSourceDeferralE2ETestCase,
    SnowflakeSourceLoaderSchemaEvolutionE2ETestCase,
    SnowflakeSourceLoaderStrategiesE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.snowflake.helpers import (
    assert_current_snowflake_snapshot_rows,
    assert_snowflake_snapshot_apply_rows,
    assert_snowflake_snapshot_matrix_rows,
    build_snowflake_dbt_profiles_yml,
    build_snowflake_local_config,
    build_snowflake_project_toml,
    build_snowflake_source_deferral_project_toml,
    cleanup_snowflake_schema,
    ensure_query_schema_ready,
    execute_snowflake_sql,
    fetch_snowflake_rows,
    list_snowflake_scenario_relation_names,
    prepare_snowflake_diff_project,
    prepare_snowflake_source_loader_strategies,
    prepare_snowflake_waffle_shop,
    relation_name,
    snowflake_relation_row_count,
    write_local_environment_override,
    write_snowflake_dbt_scenario_date_trunc_model,
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
    prepare_dbt_profile_workspace,
    prepare_inline_project,
    run_sqb,
    stringify_warehouse_rows,
)
from tests.integration.src.sqlbuild.adapters.snowflake.helpers import (
    build_snowflake_connection_config,
    build_unique_schema_name,
)


@pytest.mark.dbt
@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeDbtProfileE2ETestCase(
            description="dbt init generated project builds through Snowflake PAT dbt profile",
            schema_prefix="sqlbuild_dbt_profile",
            expected_toml_fragments=(
                'adapter = "snowflake"',
                'source = "dbt_profile"',
                'profile = "analytics"',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_dbt_profile_when_running_dbt_init_then_builds_profile_lifecycle(
    tmp_path: Path,
    test_case: SnowflakeDbtProfileE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix=test_case.schema_prefix)
    config: dict[str, object] = build_snowflake_connection_config(schema=schema_name)
    database_name: str = str(config["database"])
    try:
        ensure_query_schema_ready(schema_name=schema_name)
        assert_dbt_profile_lifecycle(
            tmp_path=tmp_path,
            profiles_yml=(
                "analytics:\n"
                "  target: dev\n"
                "  outputs:\n"
                "    dev:\n"
                "      type: snowflake\n"
                f"      account: {config['account']}\n"
                f"      user: {config['user']}\n"
                "      authenticator: programmatic_access_token\n"
                f"      token: {config['token']}\n"
                f"      role: {config['role']}\n"
                f"      warehouse: {config['warehouse']}\n"
                f"      database: {database_name}\n"
                f"      schema: {schema_name}\n"
            ),
            env=None,
            fetch_rows=lambda sql: fetch_snowflake_rows(schema_name=schema_name, sql=sql),
            no_profile_tables_exist=lambda: (
                fetch_snowflake_rows(
                    schema_name=schema_name,
                    sql=(
                        f"SELECT LOWER(table_name) FROM {database_name}.information_schema.tables "
                        f"WHERE UPPER(table_schema) = UPPER('{schema_name}') "
                        "AND LOWER(table_name) IN ('dbt_orders', 'downstream_orders') "
                        "ORDER BY LOWER(table_name)"
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
            unexpected_toml_fragments=(str(config["token"]),),
        )
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeNodeResultE2ETestCase(
            description="direct node results persist and read on snowflake",
            expected_rows=(
                ("asset", "produce_result", "success"),
                ("check", "check_produce_result", "success"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_result_when_running_check_on_snowflake_then_persists_node_results(
    tmp_path: Path,
    test_case: SnowflakeNodeResultE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_node_results")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_node_results",
        repo_files={
            "sqlbuild_project.toml": build_snowflake_project_toml(
                project_name="snowflake_node_results",
                schema_name=schema_name,
            ),
            "python/nodes/tasks/results.py": (
                "from sqlbuild.assets import asset\n\n"
                "@asset\n"
                "def produce_result(ctx):\n"
                "    '''Test task produce_result.'''\n    return ctx.result(payload={'value': 42}, metadata={'source': 'snowflake'})\n"
            ),
            "python/nodes/checks/results.py": (
                "from sqlbuild.checks import check\n"
                "from python.nodes.tasks.results import produce_result\n\n"
                "@check(depends_on=produce_result)\n"
                "def check_produce_result(ctx):\n"
                "    '''Test check check_produce_result.'''\n    return ctx.result_of(node_function=produce_result).payload['value'] == 42\n"
            ),
        },
    )
    try:
        ensure_query_schema_ready(schema_name=schema_name)
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
        rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT node_type, node_name, status "
                f"FROM {relation_name(schema_name=schema_name, name='_sqlbuild_node_results')} "
                "WHERE node_name IN ('produce_result', 'check_produce_result') "
                "ORDER BY node_type, node_name"
            ),
        )
        assert rows == test_case.expected_rows
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeCliTestCase(
            description="source freshness uses snowflake table metadata",
            command=("--no-color", "freshness", "--select", "raw_orders"),
            expected_stdout_fragments=(
                "Observed (1)",
                "raw_orders  value ",
                "kind timestamp",
                "via adapter",
                "OBSERVED=1  CHANGED=0  UNCHANGED=0  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_physical_source_without_freshness_when_running_on_snowflake_then_uses_metadata(
    tmp_path: Path,
    test_case: SnowflakeCliTestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_freshness_meta")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_freshness_metadata",
        repo_files={
            "sqlbuild_project.toml": build_snowflake_project_toml(
                project_name="snowflake_freshness_metadata",
                schema_name=schema_name,
            ),
            "sql/sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                f"    schema: {schema_name}\n"
                "    table: raw_orders\n"
            ),
            "sql/models/orders.sql": (
                'MODEL (description "Test model orders.", materialized table);\n\nSELECT id FROM __source("raw_orders")\n'
            ),
        },
    )
    try:
        ensure_query_schema_ready(schema_name=schema_name)
        raw_orders_relation: str = relation_name(schema_name=schema_name, name="raw_orders")
        execute_snowflake_sql(
            schema_name=schema_name,
            sql=f"CREATE OR REPLACE TABLE {raw_orders_relation} AS SELECT 1 AS id",
        )

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        fragment: str
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in result.stdout, result.stdout
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeCliTestCase(
            description="query command uses snowflake local override",
            command=(
                "query",
                "SELECT CURRENT_DATABASE() AS database_name, CURRENT_SCHEMA() AS schema_name",
                "--format",
                "long",
            ),
            expected_stdout_fragments=("DATABASE_NAME | SQB_DB", "SCHEMA_NAME   |"),
            expected_schema_fragment="SQLBUILD_E2E_",
        ),
        SnowflakeCliTestCase(
            description="query command renders json output",
            command=("query", "SELECT 1 AS id, 'alice' AS name", "--format", "json"),
            expected_stdout_fragments=('"ID": 1', '"NAME": "alice"'),
        ),
        SnowflakeCliTestCase(
            description="query command renders a table with Snowflake types by default",
            command=("query", "SELECT 1 AS id, 'alice' AS name"),
            expected_stdout_fragments=("┌", "NUMBER(1,0)", "TEXT", "alice", "1 row"),
        ),
        SnowflakeCliTestCase(
            description="query command prints ok for ddl statements",
            command=("query", "CREATE OR REPLACE TEMP TABLE __sqb_query_temp (id INTEGER)"),
            expected_stdout_fragments=("OK",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_local_config_when_running_query_then_outputs_expected_rows(
    tmp_path: Path,
    test_case: SnowflakeCliTestCase,
) -> None:
    project_dir: Path
    schema_name: str
    project_dir, schema_name = prepare_snowflake_waffle_shop(tmp_path=tmp_path)
    ensure_query_schema_ready(schema_name=schema_name)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command, project_dir=project_dir
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        fragment: str
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in result.stdout
        assert "Cost" in result.stdout
        run_artifacts: tuple[Path, ...] = tuple(
            (project_dir / "target" / "executions").glob("*/run.json")
        )
        assert len(run_artifacts) == 1
        run_payload: dict[str, Any] = json.loads(run_artifacts[0].read_text(encoding="utf-8"))
        assert run_payload["adapter_name"] == "snowflake"
        assert run_payload["build_status"] == "success"
        assert run_payload["cost"]["rate_source"] == "default"
        assert "estimated_compute_credits" in run_payload["cost"]
        assert (run_artifacts[0].parent / "statements.jsonl").is_file()
        cost_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "cost", "latest", "--json"),
            project_dir=project_dir,
        )
        assert cost_result.returncode == 0, cost_result.stdout + cost_result.stderr
        cost_payload: dict[str, Any] = json.loads(cost_result.stdout)
        assert cost_payload["schema_version"] == 1
        assert cost_payload["run_id"] == run_payload["run_id"]
        assert test_case.expected_schema_fragment in result.stdout
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeSourceDeferralE2ETestCase(
            description="snowflake loader writes dev while model reads prod deferred source",
            expected_model_rows=(("99", "prod-source"),),
            expected_loader_rows=(("7", "loaded-dev"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_deferral_env_when_building_on_snowflake_then_reads_prod_and_writes_dev(
    tmp_path: Path,
    test_case: SnowflakeSourceDeferralE2ETestCase,
) -> None:
    dev_schema_name: str = build_unique_schema_name(prefix="sqlbuild_e2e_defer_dev")
    prod_schema_name: str = build_unique_schema_name(prefix="sqlbuild_e2e_defer_prod")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_source_deferral",
        repo_files={
            "sqlbuild_project.toml": build_snowflake_source_deferral_project_toml(
                project_name="snowflake_source_deferral",
                dev_schema_name=dev_schema_name,
                prod_schema_name=prod_schema_name,
            ),
            "sqlbuild_local.toml": build_snowflake_local_config(schema_name=dev_schema_name),
            "sql/sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
                "      - name: status\n"
                "        type: VARCHAR\n"
            ),
            "python/nodes/loaders/raw_orders.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    return [{'order_id': 7, 'status': 'loaded-dev'}]\n"
            ),
            "sql/models/stg_orders.sql": (
                'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT order_id, status FROM __source("raw_orders")'
            ),
        },
    )
    ensure_query_schema_ready(schema_name=dev_schema_name)
    ensure_query_schema_ready(schema_name=prod_schema_name)

    try:
        execute_snowflake_sql(
            schema_name=prod_schema_name,
            sql=(
                "CREATE OR REPLACE TABLE "
                f"{relation_name(schema_name=prod_schema_name, name='raw_orders')} "
                "AS SELECT 99 AS order_id, 'prod-source' AS status"
            ),
        )

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--select", "stg_orders"),
            project_dir=project_dir,
        )

        assert result.returncode == 0, result.stdout + result.stderr
        model_rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=dev_schema_name,
            sql=(
                "SELECT order_id, status FROM "
                f"{relation_name(schema_name=dev_schema_name, name='stg_orders')} "
                "ORDER BY order_id"
            ),
        )
        loader_rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=dev_schema_name,
            sql=(
                "SELECT order_id, status FROM "
                f"{relation_name(schema_name=dev_schema_name, name='raw_orders')} "
                "ORDER BY order_id"
            ),
        )
        assert stringify_warehouse_rows(model_rows) == test_case.expected_model_rows
        assert stringify_warehouse_rows(loader_rows) == test_case.expected_loader_rows
    finally:
        cleanup_snowflake_schema(schema_name=dev_schema_name)
        cleanup_snowflake_schema(schema_name=prod_schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeSnapshotApplyE2ETestCase(
            description="applies existing-target snapshot changes on snowflake",
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
def test_given_existing_snapshot_targets_when_building_on_snowflake_then_apply_sql_succeeds(
    tmp_path: Path,
    test_case: SnowflakeSnapshotApplyE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_snapshot_apply")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_snapshot_apply_project",
        repo_files=build_real_warehouse_existing_snapshot_project_files(
            project_toml=build_snowflake_project_toml(
                project_name="snowflake_snapshot_apply_project",
                schema_name=schema_name,
            ),
        ),
    )
    ensure_query_schema_ready(schema_name=schema_name)

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
        assert_snowflake_snapshot_apply_rows(
            schema_name=schema_name,
            expected_current_check_rows=test_case.expected_current_check_rows,
            expected_current_delete_rows=test_case.expected_current_delete_rows,
            expected_historical_timestamp_rows=test_case.expected_historical_timestamp_rows,
            expected_historical_check_rows=test_case.expected_historical_check_rows,
        )
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeScenarioLocalReplayE2ETestCase(
            description="captures snowflake fixtures and replays transpilable SQL locally",
            model_sql=(
                "MODEL (description 'Test model.', materialized table);\n\n"
                "SELECT\n"
                "  customer_id,\n"
                "  DATE_TRUNC('DAY', event_ts) AS event_day,\n"
                "  SUM(IFF(amount_cents >= 1000, amount_cents, 0)) AS large_amount_cents,\n"
                "  COUNT(*) AS event_count\n"
                'FROM __source("raw_events")\n'
                "GROUP BY customer_id, DATE_TRUNC('DAY', event_ts)\n"
            ),
            scenario_sql=(
                "SCENARIO (description 'Test scenario.');\n\n"
                "WITH\n"
                "__source__raw_events AS (\n"
                "  SELECT 10 AS customer_id, TO_TIMESTAMP_NTZ('2026-01-01 08:15:00') "
                "AS event_ts, 1500 AS amount_cents\n"
                "  UNION ALL\n"
                "  SELECT 10 AS customer_id, TO_TIMESTAMP_NTZ('2026-01-01 10:30:00') "
                "AS event_ts, 500 AS amount_cents\n"
                "),\n"
                "__expected__event_rollup AS (\n"
                "  SELECT 10 AS customer_id, "
                "DATE_TRUNC('DAY', TO_TIMESTAMP_NTZ('2026-01-01 00:00:00')) AS event_day, "
                "1500 AS large_amount_cents, 2 AS event_count\n"
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
        SnowflakeScenarioLocalReplayE2ETestCase(
            description="reports snowflake local transpilation failures as X607",
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
        SnowflakeScenarioLocalReplayE2ETestCase(
            description="reports snowflake local DuckDB execution failures as X608",
            scenario_name="local_execution_error",
            model_sql=(
                "MODEL (description 'Test model.', materialized table);\n\n"
                "SELECT\n"
                "  customer_id,\n"
                "  __sqb_missing_local_function(amount_cents) AS amount_cents\n"
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
                "local_execution_error",
                "ERROR",
                "error[X608]",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
            expected_return_code=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_scenario_capture_when_replaying_locally_then_transpilable_sql_passes(
    tmp_path: Path,
    test_case: SnowflakeScenarioLocalReplayE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_scenario_local")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_scenario_local_replay",
        repo_files=build_real_warehouse_local_replay_project_files(
            project_toml=build_snowflake_project_toml(
                project_name="snowflake_scenario_local_replay",
                schema_name=schema_name,
            ),
            model_sql=test_case.model_sql,
            scenario_sql=test_case.scenario_sql,
            scenario_name=test_case.scenario_name,
        ),
    )
    ensure_query_schema_ready(schema_name=schema_name)

    try:
        capture_result: subprocess.CompletedProcess[str] = run_sqb(
            command=(
                "--no-color",
                "scenario",
                "capture",
                test_case.scenario_name,
            ),
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
        fragment: str
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in replay_result.stdout
        assert_optional_local_replay_rows(
            project_dir=project_dir,
            scenario_name=test_case.scenario_name,
            local_rows_sql=test_case.local_rows_sql,
            expected_local_rows=test_case.expected_local_rows,
        )
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.dbt
@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeDbtScenarioLocalReplayE2ETestCase(
            description="captures snowflake dbt scenario and replays date_trunc locally",
            scenario_name="dbt_event_rollup",
            expected_stdout_fragments=(
                "dbt_event_rollup",
                "expect    expected event_rollup",
                "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
            ),
            expected_local_rows=((10, 2),),
            local_rows_sql=(
                "SELECT customer_id, event_count "
                "FROM __sqb_local__model__event_rollup ORDER BY customer_id"
            ),
        ),
        SnowflakeDbtScenarioLocalReplayE2ETestCase(
            description="reports snowflake dbt local transpilation failure as X607",
            scenario_name="dbt_event_rollup",
            expected_stdout_fragments=(
                "dbt_event_rollup",
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
def test_given_snowflake_dbt_scenario_capture_when_replaying_locally_then_transpiles_to_duckdb(
    tmp_path: Path,
    test_case: SnowflakeDbtScenarioLocalReplayE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_dbt_scenario_local")
    config: dict[str, object] = build_snowflake_connection_config(schema=schema_name)
    workspace: Path = prepare_dbt_profile_workspace(
        tmp_path=tmp_path,
        profiles_yml=build_snowflake_dbt_profiles_yml(schema_name=schema_name, config=config),
    )
    sqlbuild_project_dir: Path = workspace / "sqlbuild_project"
    try:
        ensure_query_schema_ready(schema_name=schema_name)
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
        )
        assert init_result.returncode == 0, init_result.stdout + init_result.stderr
        write_snowflake_dbt_scenario_date_trunc_model(sqlbuild_project_dir=sqlbuild_project_dir)
        execute_snowflake_sql(
            schema_name=schema_name,
            sql=(
                f"CREATE TABLE {relation_name(schema_name=schema_name, name='events')} "
                "(customer_id INTEGER, event_ts TIMESTAMP_NTZ)"
            ),
        )

        capture_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "dbt", "scenario", "capture", test_case.scenario_name),
            project_dir=sqlbuild_project_dir,
        )
        assert capture_result.returncode == 0, capture_result.stdout + capture_result.stderr
        maybe_corrupt_scenario_snapshot_dialect(
            project_dir=sqlbuild_project_dir,
            scenario_name=test_case.scenario_name,
            enabled=test_case.corrupt_capture_dialect,
        )

        replay_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "dbt", "scenario", "test", test_case.scenario_name, "--local"),
            project_dir=sqlbuild_project_dir,
        )

        assert replay_result.returncode == test_case.expected_return_code, (
            replay_result.stdout + replay_result.stderr
        )
        fragment: str
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in replay_result.stdout
        assert_dbt_local_replay_rows(
            project_dir=sqlbuild_project_dir,
            scenario_name=test_case.scenario_name,
            rows_sql=test_case.local_rows_sql,
            expected_rows=test_case.expected_local_rows,
        )
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeSnapshotE2ETestCase(
            description="executes snapshot scd2 matrix on snowflake",
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
def test_given_snapshot_project_when_building_on_snowflake_then_scd2_history_is_valid(
    tmp_path: Path,
    test_case: SnowflakeSnapshotE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_snapshot")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_snapshot_project",
        repo_files=build_real_warehouse_snapshot_project_files(
            project_toml=build_snowflake_project_toml(
                project_name="snowflake_snapshot_project",
                schema_name=schema_name,
            ),
        ),
    )
    ensure_query_schema_ready(schema_name=schema_name)

    try:
        initial_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"),
            project_dir=project_dir,
        )
        assert initial_result.returncode == 0, initial_result.stdout + initial_result.stderr
        assert_snowflake_snapshot_matrix_rows(
            schema_name=schema_name,
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
        fragment: str
        for fragment in test_case.expected_failure_fragments:
            assert fragment in failure_result.stdout + failure_result.stderr
        cost_run_payloads: tuple[dict[str, Any], ...] = tuple(
            json.loads(path.read_text(encoding="utf-8"))
            for path in (project_dir / "target" / "executions").glob("*/run.json")
        )
        assert any(payload["build_status"] == "failed" for payload in cost_run_payloads)
        assert_current_snowflake_snapshot_rows(
            schema_name=schema_name,
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
        assert_current_snowflake_snapshot_rows(
            schema_name=schema_name,
            expected_rows=test_case.expected_current_rows_after_recovery,
        )
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeScenarioRemoteE2ETestCase(
            description="runs snowflake scenario remotely and retains inspectable artifacts",
            expected_stdout_fragments=(
                "remote_event_rollup",
                "Retained relations:",
                "source raw_events -> __sqb_",
                "model  stg_events -> __sqb_",
                "model  event_rollup -> __sqb_",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_retained_suffix_counts={
                "__source__raw_events": 1,
                "__model__stg_events": 1,
                "__model__event_rollup": 1,
            },
            expected_row_counts_by_suffix={
                "__source__raw_events": 2,
                "__model__stg_events": 1,
                "__model__event_rollup": 1,
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_scenario_when_running_remotely_then_cleans_up_and_retains_artifacts(
    tmp_path: Path,
    test_case: SnowflakeScenarioRemoteE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_scenario_remote")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_scenario_remote",
        repo_files=build_real_warehouse_remote_scenario_project_files(
            project_toml=build_snowflake_project_toml(
                project_name="snowflake_scenario_remote",
                schema_name=schema_name,
            ),
        ),
    )
    ensure_query_schema_ready(schema_name=schema_name)

    try:
        cleanup_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "scenario", "test", "remote_event_rollup"),
            project_dir=project_dir,
        )
        assert cleanup_result.returncode == 0, cleanup_result.stdout + cleanup_result.stderr
        assert list_snowflake_scenario_relation_names(schema_name=schema_name) == ()

        retain_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "scenario", "test", "remote_event_rollup", "--retain"),
            project_dir=project_dir,
        )

        assert retain_result.returncode == 0, retain_result.stdout + retain_result.stderr
        expected_fragment: str
        for expected_fragment in test_case.expected_stdout_fragments:
            assert expected_fragment in retain_result.stdout
        retained_names: tuple[str, ...] = list_snowflake_scenario_relation_names(
            schema_name=schema_name
        )
        assert len(retained_names) == sum(test_case.expected_retained_suffix_counts.values())
        retained_names_by_suffix: defaultdict[str, list[str]] = defaultdict(list)
        for relation in retained_names:
            retained_names_by_suffix[relation[relation.index("__", 2) :]].append(relation)
        suffix: str
        for suffix, expected_count in test_case.expected_retained_suffix_counts.items():
            matches: tuple[str, ...] = tuple(retained_names_by_suffix[suffix])
            assert len(matches) == expected_count
        for suffix, expected_count in test_case.expected_row_counts_by_suffix.items():
            matches = tuple(retained_names_by_suffix[suffix])
            assert len(matches) == 1
            assert (
                snowflake_relation_row_count(schema_name=schema_name, relation=matches[0])
                == expected_count
            )
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeBuildE2ETestCase(
            description="waffle shop full build succeeds on snowflake",
            command=("--no-color", "build", "--concurrency", "4"),
            expected_table_name="fact_orders",
            expected_row_count=10,
            expected_udf_rows=((1, True), (10, False)),
            expected_python_udf_rows=((1, True), (10, False)),
            expected_stdout_fragments=("Execution", "OK"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_running_full_build_on_snowflake_then_expected_table_exists(
    tmp_path: Path,
    test_case: SnowflakeBuildE2ETestCase,
) -> None:
    project_dir: Path
    schema_name: str
    project_dir, schema_name = prepare_snowflake_waffle_shop(tmp_path=tmp_path)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        fragment: str
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in result.stdout
        rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT COUNT(*) FROM "
                f"{relation_name(schema_name=schema_name, name=test_case.expected_table_name)}"
            ),
        )
        row_count: object = rows[0][0]
        assert isinstance(row_count, int)
        assert row_count == test_case.expected_row_count
        udf_rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT order_id, is_completed_order FROM "
                f"{relation_name(schema_name=schema_name, name='fact_orders')} "
                "WHERE order_id IN (1, 10) ORDER BY order_id"
            ),
        )
        assert udf_rows == test_case.expected_udf_rows
        python_udf_rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT order_id, is_completed_order_py FROM "
                f"{relation_name(schema_name=schema_name, name='fact_orders')} "
                "WHERE order_id IN (1, 10) ORDER BY order_id"
            ),
        )
        assert python_udf_rows == test_case.expected_python_udf_rows
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeSourceLoaderStrategiesE2ETestCase(
            description="source loader strategies apply expected rows on snowflake",
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
def test_given_loader_strategy_project_when_loading_twice_on_snowflake_then_write_modes_apply(
    tmp_path: Path,
    test_case: SnowflakeSourceLoaderStrategiesE2ETestCase,
) -> None:
    project_dir: Path
    schema_name: str
    project_dir, schema_name = prepare_snowflake_source_loader_strategies(tmp_path=tmp_path)

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

        countries: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT country_id, country_code, country_name FROM "
                f"{relation_name(schema_name=schema_name, name='raw_countries')} "
                "ORDER BY country_id"
            ),
        )
        webhook_event_counts: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT event_id, event_name, COUNT(*) FROM "
                f"{relation_name(schema_name=schema_name, name='raw_webhook_events')} "
                "GROUP BY event_id, event_name ORDER BY event_id"
            ),
        )
        order_events: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT event_id, amount_cents FROM "
                f"{relation_name(schema_name=schema_name, name='raw_order_events')} "
                "ORDER BY event_id"
            ),
        )
        customers: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT customer_id, plan_name FROM "
                f"{relation_name(schema_name=schema_name, name='raw_customers')} "
                "ORDER BY customer_id"
            ),
        )
        loader_status: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
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
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeSourceLoaderSchemaEvolutionE2ETestCase(
            description="source loader schema evolution adds late columns on snowflake",
            command=("--no-color", "load"),
            expected_rows=(("1", None), ("2", "late-note")),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_schema_evolution_project_when_loading_twice_on_snowflake_then_target_evolves(
    tmp_path: Path,
    test_case: SnowflakeSourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_e2e_load_schema")
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
    (project_dir / "sqlbuild_project.toml").write_text(
        build_snowflake_project_toml(
            project_name="source_loader_schema_behavior",
            schema_name=schema_name,
        ),
        encoding="utf-8",
    )

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
        rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT event_id, note FROM "
                f"{relation_name(schema_name=schema_name, name='raw_events')} "
                "ORDER BY event_id"
            ),
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeSourceLoaderSchemaEvolutionE2ETestCase(
            description="chained source loader runs on snowflake",
            command=("--no-color", "load", "--select", "+raw_events"),
            expected_rows=(("1", "loaded"), ("2", "loaded")),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_loading_on_snowflake_then_runs_loader_dag(
    tmp_path: Path,
    test_case: SnowflakeSourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_e2e_load_dag")
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
                "    cursor = ctx.query(\n"
                "        f'SELECT event_id FROM {events.destination} ORDER BY event_id'\n"
                "    )\n"
                "    rows = cursor.fetchall()\n"
                "    return [{'event_id': row[0], 'status': 'loaded'} for row in rows]\n"
            ),
        ),
    )
    (project_dir / "sqlbuild_project.toml").write_text(
        build_snowflake_project_toml(
            project_name="source_loader_dag_behavior",
            schema_name=schema_name,
        ),
        encoding="utf-8",
    )
    ensure_query_schema_ready(schema_name=schema_name)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT event_id, status FROM "
                f"{relation_name(schema_name=schema_name, name='raw_events')} "
                "ORDER BY event_id"
            ),
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
        assert (
            snowflake_relation_row_count(schema_name=schema_name, relation="__loader__fetch_events")
            == 2
        )
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeIntermediateDagStrategyE2ETestCase(
            description="snowflake append intermediate accumulates rows across DAG loads",
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
        SnowflakeIntermediateDagStrategyE2ETestCase(
            description="snowflake merge intermediate updates and adds rows across DAG loads",
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
        SnowflakeIntermediateDagStrategyE2ETestCase(
            description="snowflake delete insert intermediate replaces cursor window across DAG loads",
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
def test_given_intermediate_strategy_project_when_loading_twice_on_snowflake_then_strategy_applies(
    tmp_path: Path,
    test_case: SnowflakeIntermediateDagStrategyE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_e2e_load_dag_strategy")
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
        build_snowflake_project_toml(
            project_name="source_loader_dag_strategy_behavior",
            schema_name=schema_name,
        ),
        encoding="utf-8",
    )
    ensure_query_schema_ready(schema_name=schema_name)

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
        intermediate_rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT event_id, amount FROM "
                f"{relation_name(schema_name=schema_name, name='__loader__fetch_events')} "
                "ORDER BY event_id, amount"
            ),
        )
        terminal_rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT event_id, amount FROM "
                f"{relation_name(schema_name=schema_name, name='raw_events')} "
                "ORDER BY event_id, amount"
            ),
        )
        assert stringify_warehouse_rows(intermediate_rows) == test_case.expected_intermediate_rows
        assert stringify_warehouse_rows(terminal_rows) == test_case.expected_terminal_rows
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeSourceLoaderSchemaEvolutionE2ETestCase(
            description="loader focused waffle shop grows across repeated snowflake builds",
            command=("--no-color", "build", "--select", "+customer_revenue"),
            expected_rows=(
                ("1", "pro", "650", "1"),
                ("2", "plus", "3750", "2"),
                ("3", "enterprise", "1300", "1"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_waffle_shop_when_building_on_snowflake_then_dag_grows_models(
    tmp_path: Path,
    test_case: SnowflakeSourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqlbuild_e2e_load_waffle")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="loader_waffle_shop",
        repo_files=build_loader_waffle_shop_project_files(
            project_toml=build_snowflake_project_toml(
                project_name="loader_waffle_shop",
                schema_name=schema_name,
            )
        ),
    )
    ensure_query_schema_ready(schema_name=schema_name)

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

        rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=schema_name,
            sql=(
                "SELECT customer_id, plan_name, revenue_cents, order_count FROM "
                f"{relation_name(schema_name=schema_name, name='customer_revenue')} "
                "ORDER BY customer_id"
            ),
        )
        event_count: int = snowflake_relation_row_count(
            schema_name=schema_name, relation="__loader__fetch_order_events"
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
        assert event_count == 4
    finally:
        cleanup_snowflake_schema(schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeDiffE2ETestCase(
            description="schema only diff reports clean identical schemas",
            mutation_sql=(),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--schema-only",
                "--select",
                "stg_orders",
            ),
            expected_stdout_fragments=("stg_orders", "No schema differences."),
            expected_return_code=0,
        ),
        SnowflakeDiffE2ETestCase(
            description="full diff reports row mismatch",
            mutation_sql=(
                "UPDATE stg_orders SET amount_cents = amount_cents + 5 WHERE order_id = 1",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "stg_orders",
            ),
            expected_stdout_fragments=("amount_cents", "mismatches=1", "order_id=1 | 100 -> 105"),
            expected_return_code=1,
        ),
        SnowflakeDiffE2ETestCase(
            description="verbose diff shows changed row examples",
            mutation_sql=(
                "UPDATE stg_orders SET amount_cents = amount_cents + 5 WHERE order_id = 1",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--verbose",
                "--select",
                "stg_orders",
            ),
            expected_stdout_fragments=("Examples", "order_id=1 | 100 -> 105"),
            expected_return_code=1,
        ),
        SnowflakeDiffE2ETestCase(
            description="verbose diff shows side only examples",
            mutation_sql=(
                "DELETE FROM stg_orders WHERE order_id = 1",
                "INSERT INTO stg_orders (order_id, customer_id, amount_cents) VALUES (3, 3, 999)",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--verbose",
                "--select",
                "stg_orders",
            ),
            expected_stdout_fragments=("prod only", "order_id=1", "dev only", "order_id=3"),
            expected_return_code=1,
        ),
        SnowflakeDiffE2ETestCase(
            description="bounded diff reports mismatch inside bounded window",
            mutation_sql=(
                "UPDATE stg_orders SET amount_cents = amount_cents + 5 WHERE order_id = 2",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--bounded",
                "7d",
                "--select",
                "stg_orders",
            ),
            expected_stdout_fragments=("amount_cents", "mismatches=1", "order_id=2 | 200 -> 205"),
            expected_return_code=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_project_when_running_diff_then_outputs_expected_summary(
    tmp_path: Path,
    test_case: SnowflakeDiffE2ETestCase,
) -> None:
    project_dir: Path
    prod_schema: str
    dev_schema: str
    project_dir, prod_schema, dev_schema = prepare_snowflake_diff_project(tmp_path=tmp_path)

    try:
        write_local_environment_override(project_dir=project_dir, environment="prod")
        prod_build: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"),
            project_dir=project_dir,
        )
        assert prod_build.returncode == 0, prod_build.stdout + prod_build.stderr

        write_local_environment_override(project_dir=project_dir, environment="dev")
        dev_build: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"),
            project_dir=project_dir,
        )
        assert dev_build.returncode == 0, dev_build.stdout + dev_build.stderr

        statement: str
        for statement in test_case.mutation_sql:
            execute_snowflake_sql(
                schema_name=dev_schema,
                sql=statement.replace(
                    "stg_orders",
                    relation_name(schema_name=dev_schema, name="stg_orders"),
                ),
            )

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        fragment: str
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in result.stdout
    finally:
        cleanup_snowflake_schema(schema_name=prod_schema)
        cleanup_snowflake_schema(schema_name=dev_schema)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeCloneE2ETestCase(
            description="clone defaults to zero copy and hard copy uses CTAS",
            default_command=(
                "--no-color",
                "clone",
                "--from",
                "prod",
                "--to",
                "dev",
                "--select",
                "stg_orders",
            ),
            hard_copy_command=(
                "--no-color",
                "clone",
                "--from",
                "prod",
                "--to",
                "dev",
                "--hard-copy",
                "--select",
                "stg_orders",
            ),
            expected_default_stdout_fragments=(
                "stg_orders",
                "cloned",
                "CLONED=1  COPIED=0",
                "PASS=1  WARN=0  FAIL=0  TOTAL=1",
            ),
            expected_hard_copy_stdout_fragments=(
                "stg_orders",
                "copied",
                "CLONED=0  COPIED=1",
                "PASS=1  WARN=0  FAIL=0  TOTAL=1",
            ),
            expected_rows=((1, 1, 100), (2, 2, 200)),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_project_when_cloning_then_default_uses_zero_copy_and_hard_copy_ctas(
    tmp_path: Path,
    test_case: SnowflakeCloneE2ETestCase,
) -> None:
    project_dir: Path
    prod_schema: str
    dev_schema: str
    project_dir, prod_schema, dev_schema = prepare_snowflake_diff_project(tmp_path=tmp_path)

    try:
        write_local_environment_override(project_dir=project_dir, environment="prod")
        prod_build: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"),
            project_dir=project_dir,
        )
        assert prod_build.returncode == 0, prod_build.stdout + prod_build.stderr
        ensure_query_schema_ready(schema_name=dev_schema)

        default_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.default_command,
            project_dir=project_dir,
        )
        assert default_result.returncode == 0, default_result.stdout + default_result.stderr
        fragment: str
        for fragment in test_case.expected_default_stdout_fragments:
            assert fragment in default_result.stdout
        cloned_rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=dev_schema,
            sql=(
                "SELECT order_id, customer_id, amount_cents FROM "
                f"{relation_name(schema_name=dev_schema, name='stg_orders')} ORDER BY order_id"
            ),
        )
        assert cloned_rows == test_case.expected_rows

        execute_snowflake_sql(
            schema_name=dev_schema,
            sql=f"DROP TABLE {relation_name(schema_name=dev_schema, name='stg_orders')}",
        )
        hard_copy_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.hard_copy_command,
            project_dir=project_dir,
        )
        assert hard_copy_result.returncode == 0, hard_copy_result.stdout + hard_copy_result.stderr
        for fragment in test_case.expected_hard_copy_stdout_fragments:
            assert fragment in hard_copy_result.stdout
        copied_rows: tuple[tuple[object, ...], ...] = fetch_snowflake_rows(
            schema_name=dev_schema,
            sql=(
                "SELECT order_id, customer_id, amount_cents FROM "
                f"{relation_name(schema_name=dev_schema, name='stg_orders')} ORDER BY order_id"
            ),
        )
        assert copied_rows == test_case.expected_rows
    finally:
        cleanup_snowflake_schema(schema_name=prod_schema)
        cleanup_snowflake_schema(schema_name=dev_schema)
