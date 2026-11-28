from __future__ import annotations

import json
import subprocess
from collections import defaultdict
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.bigquery._test_types import (
    BigQueryBuildE2ETestCase,
    BigQueryCliTestCase,
    BigQueryCloneE2ETestCase,
    BigQueryDbtProfileE2ETestCase,
    BigQueryDiffE2ETestCase,
    BigQueryErrorE2ETestCase,
    BigQueryIntermediateDagStrategyE2ETestCase,
    BigQueryModelBuildE2ETestCase,
    BigQueryNodeResultE2ETestCase,
    BigQueryScenarioRemoteE2ETestCase,
    BigQuerySnapshotApplyE2ETestCase,
    BigQuerySnapshotE2ETestCase,
    BigQuerySourceDeferralE2ETestCase,
    BigQuerySourceLoaderSchemaEvolutionE2ETestCase,
    BigQuerySourceLoaderStrategiesE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.bigquery.helpers import (
    assert_bigquery_snapshot_apply_rows,
    assert_bigquery_snapshot_matrix_rows,
    assert_current_bigquery_snapshot_rows,
    bigquery_relation_row_count,
    build_bigquery_local_config,
    build_bigquery_project_toml,
    build_bigquery_source_deferral_project_toml,
    cleanup_bigquery_dataset,
    ensure_bigquery_dataset_ready,
    execute_bigquery_sql,
    fetch_bigquery_rows,
    list_bigquery_scenario_relation_names,
    prepare_bigquery_diff_project,
    prepare_bigquery_query_source,
    prepare_bigquery_source_loader_strategies,
    prepare_bigquery_waffle_shop,
    relation_name,
    write_local_environment_override,
)
from tests.e2e.src.sqlbuild.cli.commands.main.load.helpers import (
    build_loader_waffle_shop_project_files,
    build_schema_behavior_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_real_warehouse_remote_scenario_project_files,
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
from tests.integration.src.sqlbuild.adapters.bigquery.helpers import (
    build_bigquery_connection_config,
    build_unique_dataset_name,
)
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files


@pytest.mark.dbt
@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryDbtProfileE2ETestCase(
            description="dbt init generated project builds through BigQuery dbt profile",
            schema_prefix="sqlbuild_dbt_profile",
            expected_toml_fragments=(
                'adapter = "bigquery"',
                'source = "dbt_profile"',
                'profile = "analytics"',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_dbt_profile_when_running_dbt_init_then_builds_profile_lifecycle(
    tmp_path: Path,
    test_case: BigQueryDbtProfileE2ETestCase,
) -> None:
    dataset_name: str = build_unique_dataset_name(prefix=test_case.schema_prefix)
    config: dict[str, object] = build_bigquery_connection_config(schema=dataset_name)
    project_id: str = str(config["project"])
    try:
        ensure_bigquery_dataset_ready(dataset_name=dataset_name)
        assert_dbt_profile_lifecycle(
            tmp_path=tmp_path,
            profiles_yml=(
                "analytics:\n"
                "  target: dev\n"
                "  outputs:\n"
                "    dev:\n"
                "      type: bigquery\n"
                "      method: oauth\n"
                f"      project: {project_id}\n"
                f"      dataset: {dataset_name}\n"
                f"      location: {config['location']}\n"
            ),
            env=None,
            fetch_rows=lambda sql: fetch_bigquery_rows(dataset_name=dataset_name, sql=sql),
            no_profile_tables_exist=lambda: (
                fetch_bigquery_rows(
                    dataset_name=dataset_name,
                    sql=(
                        "SELECT table_name FROM "
                        f"`{project_id}.{dataset_name}.INFORMATION_SCHEMA.TABLES` "
                        "WHERE table_name IN ('dbt_orders', 'downstream_orders') "
                        "ORDER BY table_name"
                    ),
                )
                == ()
            ),
            dbt_orders_sql=(
                "SELECT order_id FROM "
                f"{relation_name(dataset_name=dataset_name, name='dbt_orders')} "
                "ORDER BY order_id"
            ),
            downstream_orders_sql=(
                "SELECT order_id FROM "
                f"{relation_name(dataset_name=dataset_name, name='downstream_orders')} "
                "ORDER BY order_id"
            ),
            expected_toml_fragments=test_case.expected_toml_fragments,
        )
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryNodeResultE2ETestCase(
            description="direct node results persist and read on bigquery",
            expected_rows=(
                ("asset", "produce_result", "success"),
                ("check", "check_produce_result", "success"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_result_when_running_check_on_bigquery_then_persists_node_results(
    tmp_path: Path,
    test_case: BigQueryNodeResultE2ETestCase,
) -> None:
    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_node_results")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="bigquery_node_results",
        repo_files={
            "sqlbuild_project.toml": build_bigquery_project_toml(
                project_name="bigquery_node_results",
                dataset_name=dataset_name,
            ),
            "python/nodes/tasks/results/asset.py": (
                "from sqlbuild.assets import asset\n\n"
                "@asset\n"
                "def produce_result(ctx):\n"
                "    '''Test task produce_result.'''\n    return ctx.result(payload={'value': 42}, metadata={'source': 'bigquery'})\n"
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
    try:
        ensure_bigquery_dataset_ready(dataset_name=dataset_name)
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
        rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT node_type, node_name, status "
                f"FROM {relation_name(dataset_name=dataset_name, name='_sqlbuild_node_results')} "
                "WHERE node_name IN ('produce_result', 'check_produce_result') "
                "ORDER BY node_type, node_name"
            ),
        )
        assert rows == test_case.expected_rows
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryCliTestCase(
            description="source freshness uses bigquery table metadata",
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
def test_given_physical_source_without_freshness_when_running_on_bigquery_then_uses_metadata(
    tmp_path: Path,
    test_case: BigQueryCliTestCase,
) -> None:
    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_freshness_meta")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="bigquery_freshness_metadata",
        repo_files={
            "sqlbuild_project.toml": build_bigquery_project_toml(
                project_name="bigquery_freshness_metadata",
                dataset_name=dataset_name,
            ),
            "sql/sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                f"    schema: {dataset_name}\n"
                "    table: raw_orders\n"
            ),
            "sql/models/orders.sql": (
                'MODEL (description "Test model orders.", materialized table);\n\nSELECT id FROM __source("raw_orders")\n'
            ),
        },
    )
    try:
        ensure_bigquery_dataset_ready(dataset_name=dataset_name)
        raw_orders_relation: str = relation_name(dataset_name=dataset_name, name="raw_orders")
        execute_bigquery_sql(
            dataset_name=dataset_name,
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
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryCliTestCase(
            description="query command uses bigquery local override",
            command=("query", "SELECT id AS ID, name AS NAME FROM {source} ORDER BY ID"),
            expected_stdout_fragments=("┌", "│ alice │", "│ bob   │", "2 rows"),
        ),
        BigQueryCliTestCase(
            description="query command renders json output",
            command=(
                "query",
                "SELECT id AS ID, name AS NAME FROM {source} ORDER BY ID LIMIT 1",
                "--format",
                "json",
            ),
            expected_stdout_fragments=('"ID": 1', '"NAME": "alice"'),
        ),
        BigQueryCliTestCase(
            description="query command renders long output",
            command=(
                "query",
                "SELECT id AS ID, name AS NAME FROM {source} ORDER BY ID LIMIT 1",
                "--format",
                "long",
            ),
            expected_stdout_fragments=("ID   | 1", "NAME | alice"),
        ),
        BigQueryCliTestCase(
            description="query command prints ok for ddl statements",
            command=("query", "CREATE OR REPLACE TABLE {ddl_target} (id INT64)"),
            expected_stdout_fragments=("OK",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_local_config_when_running_query_then_outputs_expected_rows(
    tmp_path: Path,
    test_case: BigQueryCliTestCase,
) -> None:
    project_dir: Path
    dataset_name: str
    project_dir, dataset_name = prepare_bigquery_waffle_shop(tmp_path=tmp_path)
    ensure_bigquery_dataset_ready(dataset_name=dataset_name)
    source_name: str = prepare_bigquery_query_source(dataset_name=dataset_name)
    ddl_target: str = relation_name(dataset_name=dataset_name, name="query_target")
    command: tuple[str, ...] = tuple(
        part.format(source=source_name, ddl_target=ddl_target) for part in test_case.command
    )

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(command=command, project_dir=project_dir)

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        fragment: str
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in result.stdout
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQuerySourceDeferralE2ETestCase(
            description="bigquery loader writes dev while model reads prod deferred source",
            expected_model_rows=(("99", "prod-source"),),
            expected_loader_rows=(("7", "loaded-dev"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_deferral_env_when_building_on_bigquery_then_reads_prod_and_writes_dev(
    tmp_path: Path,
    test_case: BigQuerySourceDeferralE2ETestCase,
) -> None:
    dev_dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_e2e_defer_dev")
    prod_dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_e2e_defer_prod")
    location: str = str(build_bigquery_connection_config(schema=dev_dataset_name)["location"])
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="bigquery_source_deferral",
        repo_files={
            "sqlbuild_project.toml": build_bigquery_source_deferral_project_toml(
                project_name="bigquery_source_deferral",
                dev_dataset_name=dev_dataset_name,
                prod_dataset_name=prod_dataset_name,
            ),
            "sqlbuild_local.toml": build_bigquery_local_config(location=location),
            "sql/sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
                "      - name: status\n"
                "        type: STRING\n"
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
    ensure_bigquery_dataset_ready(dataset_name=dev_dataset_name)
    ensure_bigquery_dataset_ready(dataset_name=prod_dataset_name)

    try:
        execute_bigquery_sql(
            dataset_name=prod_dataset_name,
            sql=(
                "CREATE OR REPLACE TABLE "
                f"{relation_name(dataset_name=prod_dataset_name, name='raw_orders')} "
                "AS SELECT 99 AS order_id, 'prod-source' AS status"
            ),
        )

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--select", "stg_orders"),
            project_dir=project_dir,
        )

        assert result.returncode == 0, result.stdout + result.stderr
        model_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dev_dataset_name,
            sql=(
                "SELECT order_id, status FROM "
                f"{relation_name(dataset_name=dev_dataset_name, name='stg_orders')} "
                "ORDER BY order_id"
            ),
        )
        loader_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dev_dataset_name,
            sql=(
                "SELECT order_id, status FROM "
                f"{relation_name(dataset_name=dev_dataset_name, name='raw_orders')} "
                "ORDER BY order_id"
            ),
        )
        assert stringify_warehouse_rows(model_rows) == test_case.expected_model_rows
        assert stringify_warehouse_rows(loader_rows) == test_case.expected_loader_rows
    finally:
        cleanup_bigquery_dataset(dataset_name=dev_dataset_name)
        cleanup_bigquery_dataset(dataset_name=prod_dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryScenarioRemoteE2ETestCase(
            description="runs bigquery scenario remotely and retains inspectable artifacts",
            expected_stdout_fragments=(
                "remote_event_rollup",
                "Retained (1):",
                "source    raw_events -> __sqb_",
                "model     stg_events -> __sqb_",
                "model     event_rollup -> __sqb_",
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
def test_given_bigquery_scenario_when_running_remotely_then_cleans_up_and_retains_artifacts(
    tmp_path: Path,
    test_case: BigQueryScenarioRemoteE2ETestCase,
) -> None:
    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_scenario_remote")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="bigquery_scenario_remote",
        repo_files=build_real_warehouse_remote_scenario_project_files(
            project_toml=build_bigquery_project_toml(
                project_name="bigquery_scenario_remote",
                dataset_name=dataset_name,
            ),
        ),
    )
    ensure_bigquery_dataset_ready(dataset_name=dataset_name)

    try:
        cleanup_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "scenario", "test", "remote_event_rollup"),
            project_dir=project_dir,
        )
        assert cleanup_result.returncode == 0, cleanup_result.stdout + cleanup_result.stderr
        assert list_bigquery_scenario_relation_names(dataset_name=dataset_name) == ()

        retain_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "scenario", "test", "remote_event_rollup", "--retain"),
            project_dir=project_dir,
        )

        assert retain_result.returncode == 0, retain_result.stdout + retain_result.stderr
        expected_fragment: str
        for expected_fragment in test_case.expected_stdout_fragments:
            assert expected_fragment in retain_result.stdout
        retained_names: tuple[str, ...] = list_bigquery_scenario_relation_names(
            dataset_name=dataset_name
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
                bigquery_relation_row_count(dataset_name=dataset_name, relation=matches[0])
                == expected_count
            )
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQuerySnapshotE2ETestCase(
            description="executes snapshot scd2 matrix on bigquery",
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
def test_given_snapshot_project_when_building_on_bigquery_then_scd2_history_is_valid(
    tmp_path: Path,
    test_case: BigQuerySnapshotE2ETestCase,
) -> None:
    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_snapshot")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="bigquery_snapshot_project",
        repo_files=build_real_warehouse_snapshot_project_files(
            project_toml=build_bigquery_project_toml(
                project_name="bigquery_snapshot_project",
                dataset_name=dataset_name,
            ),
        ),
    )

    try:
        initial_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"),
            project_dir=project_dir,
        )
        assert initial_result.returncode == 0, initial_result.stdout + initial_result.stderr
        assert_bigquery_snapshot_matrix_rows(
            dataset_name=dataset_name,
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
        assert_current_bigquery_snapshot_rows(
            dataset_name=dataset_name,
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
        assert_current_bigquery_snapshot_rows(
            dataset_name=dataset_name,
            expected_rows=test_case.expected_current_rows_after_recovery,
        )
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQuerySnapshotApplyE2ETestCase(
            description="applies existing-target snapshot changes on bigquery",
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
def test_given_existing_snapshot_targets_when_building_on_bigquery_then_apply_sql_succeeds(
    tmp_path: Path,
    test_case: BigQuerySnapshotApplyE2ETestCase,
) -> None:
    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_snapshot_apply")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="bigquery_snapshot_apply_project",
        repo_files=build_real_warehouse_existing_snapshot_project_files(
            project_toml=build_bigquery_project_toml(
                project_name="bigquery_snapshot_apply_project",
                dataset_name=dataset_name,
            ),
        ),
    )

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
        assert_bigquery_snapshot_apply_rows(
            dataset_name=dataset_name,
            expected_current_check_rows=test_case.expected_current_check_rows,
            expected_current_delete_rows=test_case.expected_current_delete_rows,
            expected_historical_timestamp_rows=test_case.expected_historical_timestamp_rows,
            expected_historical_check_rows=test_case.expected_historical_check_rows,
        )
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryBuildE2ETestCase(
            description="waffle shop full build succeeds on bigquery",
            command=("--no-color", "build", "--concurrency", "4"),
            expected_table_name="fact_orders",
            expected_row_count=10,
            expected_fact_order_rows=(
                (1, "Classic Belgian", "sweet", 1700, "completed", "success"),
                (3, "Chicken and Waffle", "savory", 4350, "completed", "success"),
                (10, "Classic Belgian", "sweet", 3400, "placed", None),
            ),
            expected_udf_rows=((1, True), (10, False)),
            expected_python_udf_rows=((1, True), (10, False)),
            expected_daily_revenue_rows=(
                ("2026-04-01", 3, 6, 7100),
                ("2026-04-02", 3, 3, 2550),
                ("2026-04-03", 1, 1, 950),
            ),
            expected_stdout_fragments=("Execution", "OK"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_running_full_build_on_bigquery_then_expected_table_exists(
    tmp_path: Path,
    test_case: BigQueryBuildE2ETestCase,
) -> None:
    project_dir: Path
    dataset_name: str
    project_dir, dataset_name = prepare_bigquery_waffle_shop(tmp_path=tmp_path)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        fragment: str
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in result.stdout
        rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT COUNT(*) FROM "
                f"{relation_name(dataset_name=dataset_name, name=test_case.expected_table_name)}"
            ),
        )
        row_count: object = rows[0][0]
        assert isinstance(row_count, int)
        assert row_count == test_case.expected_row_count
        fact_order_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT order_id, waffle_name, waffle_category, line_total_cents, "
                "order_status, payment_status FROM "
                f"{relation_name(dataset_name=dataset_name, name='fact_orders')} "
                "WHERE order_id IN (1, 3, 10) ORDER BY order_id"
            ),
        )
        assert fact_order_rows == test_case.expected_fact_order_rows
        udf_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT order_id, is_completed_order FROM "
                f"{relation_name(dataset_name=dataset_name, name='fact_orders')} "
                "WHERE order_id IN (1, 10) ORDER BY order_id"
            ),
        )
        assert udf_rows == test_case.expected_udf_rows
        python_udf_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT order_id, is_completed_order_py FROM "
                f"{relation_name(dataset_name=dataset_name, name='fact_orders')} "
                "WHERE order_id IN (1, 10) ORDER BY order_id"
            ),
        )
        assert python_udf_rows == test_case.expected_python_udf_rows
        daily_revenue_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT CAST(revenue_date AS STRING), order_count, waffles_sold, "
                "total_revenue_cents FROM "
                f"{relation_name(dataset_name=dataset_name, name='daily_revenue')} "
                "ORDER BY revenue_date"
            ),
        )
        assert daily_revenue_rows == test_case.expected_daily_revenue_rows
        run_dir: Path = project_dir / "target" / "run" / "sql" / "models"
        hourly_sql: str = (run_dir / "marts" / "hourly_order_activity.sql").read_text(
            encoding="utf-8"
        )
        daily_sql: str = (run_dir / "marts" / "daily_activity_rollup.sql").read_text(
            encoding="utf-8"
        )
        contextual_sql: str = (
            run_dir / "marts" / "hourly_activity_with_daily_context.sql"
        ).read_text(encoding="utf-8")
        order_status_sql: str = (run_dir / "intermediate" / "order_status_index.sql").read_text(
            encoding="utf-8"
        )
        diagnostic_path: Path = tuple((project_dir / "logs").glob("*/*/diagnostics.jsonl"))[0]
        diagnostic_records: tuple[dict[str, object], ...] = tuple(
            map(json.loads, diagnostic_path.read_text(encoding="utf-8").splitlines())
        )
        assert "TIMESTAMP_TRUNC(" in hourly_sql
        assert "TIMESTAMP_TRUNC(" in daily_sql
        assert "TIMESTAMP_TRUNC(" in contextual_sql
        assert "`" in order_status_sql
        sql_diagnostics: tuple[dict[str, object], ...] = tuple(
            filter(lambda record: record["logger"] == "sqlbuild.sql", diagnostic_records)
        )
        assert sql_diagnostics
        for diagnostic in sql_diagnostics:
            fields: object = diagnostic["fields"]
            assert isinstance(fields, dict)
            assert "sql_digest" in fields
            assert "sql_text" not in fields
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQuerySourceLoaderStrategiesE2ETestCase(
            description="source loader strategies apply expected rows on bigquery",
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
def test_given_loader_strategy_project_when_loading_twice_on_bigquery_then_write_modes_apply(
    tmp_path: Path,
    test_case: BigQuerySourceLoaderStrategiesE2ETestCase,
) -> None:
    project_dir: Path
    dataset_name: str
    project_dir, dataset_name = prepare_bigquery_source_loader_strategies(tmp_path=tmp_path)
    ensure_bigquery_dataset_ready(dataset_name=dataset_name)

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

        countries: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT country_id, country_code, country_name FROM "
                f"{relation_name(dataset_name=dataset_name, name='raw_countries')} "
                "ORDER BY country_id"
            ),
        )
        webhook_event_counts: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT event_id, event_name, COUNT(*) FROM "
                f"{relation_name(dataset_name=dataset_name, name='raw_webhook_events')} "
                "GROUP BY event_id, event_name ORDER BY event_id"
            ),
        )
        order_events: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT event_id, amount_cents FROM "
                f"{relation_name(dataset_name=dataset_name, name='raw_order_events')} "
                "ORDER BY event_id"
            ),
        )
        customers: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT customer_id, plan_name FROM "
                f"{relation_name(dataset_name=dataset_name, name='raw_customers')} "
                "ORDER BY customer_id"
            ),
        )
        loader_status: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT status_id, status_name, loaded_by FROM "
                f"{relation_name(dataset_name=dataset_name, name='raw_loader_status')} "
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
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQuerySourceLoaderSchemaEvolutionE2ETestCase(
            description="source loader schema evolution adds late columns on bigquery",
            command=("--no-color", "load"),
            expected_rows=(("1", None), ("2", "late-note")),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_schema_evolution_project_when_loading_twice_on_bigquery_then_target_evolves(
    tmp_path: Path,
    test_case: BigQuerySourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_e2e_load_schema")
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
                "      - name: load_seq\n"
                "        type: INTEGER\n"
            ),
            node_files={
                **node_module_files(
                    "python/nodes/loaders/source_rows.py",
                    "from sqlbuild.loaders import loader\n\n"
                    "@loader(write_strategy='append', cursor_column='load_seq')\n"
                    "def raw_events(ctx):\n"
                    "    '''Test loader raw_events.'''\n    if ctx.cursor_value is None:\n"
                    "        return [{'event_id': 1, 'load_seq': 1}]\n"
                    "    return [{'event_id': 2, 'load_seq': 2, 'note': 'late-note'}]\n",
                )
            },
        ),
    )
    (project_dir / "sqlbuild_project.toml").write_text(
        build_bigquery_project_toml(
            project_name="source_loader_schema_behavior",
            dataset_name=dataset_name,
        ),
        encoding="utf-8",
    )
    ensure_bigquery_dataset_ready(dataset_name=dataset_name)

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
        rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT event_id, note FROM "
                f"{relation_name(dataset_name=dataset_name, name='raw_events')} "
                "ORDER BY event_id"
            ),
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQuerySourceLoaderSchemaEvolutionE2ETestCase(
            description="chained source loader runs on bigquery",
            command=("--no-color", "load", "--select", "+raw_events"),
            expected_rows=(("1", "loaded"), ("2", "loaded")),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_chained_loader_project_when_loading_on_bigquery_then_runs_loader_dag(
    tmp_path: Path,
    test_case: BigQuerySourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_e2e_load_dag")
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
                "        type: INTEGER\n"
                "      - name: status\n"
                "        type: VARCHAR\n"
            ),
            node_files={
                **node_module_files(
                    "python/nodes/loaders/source_rows.py",
                    "from sqlbuild.loaders import loader\n\n"
                    "@loader(write_strategy='replace', columns=[\n"
                    "    {'name': 'event_id', 'type': 'INTEGER'},\n"
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
        build_bigquery_project_toml(
            project_name="source_loader_dag_behavior",
            dataset_name=dataset_name,
        ),
        encoding="utf-8",
    )
    ensure_bigquery_dataset_ready(dataset_name=dataset_name)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT event_id, status FROM "
                f"{relation_name(dataset_name=dataset_name, name='raw_events')} "
                "ORDER BY event_id"
            ),
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
        assert (
            bigquery_relation_row_count(
                dataset_name=dataset_name, relation="__loader__fetch_events"
            )
            == 2
        )
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryIntermediateDagStrategyE2ETestCase(
            description="bigquery append intermediate accumulates rows across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='append', cursor_column='load_seq', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "    {'name': 'amount', 'type': 'INTEGER'},\n"
                "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
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
        BigQueryIntermediateDagStrategyE2ETestCase(
            description="bigquery merge intermediate updates and adds rows across DAG loads",
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
        BigQueryIntermediateDagStrategyE2ETestCase(
            description="bigquery delete insert intermediate replaces cursor window across DAG loads",
            loader_py=(
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='delete_insert', cursor_column='load_seq', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "    {'name': 'amount', 'type': 'INTEGER'},\n"
                "    {'name': 'load_seq', 'type': 'INTEGER'},\n"
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
def test_given_intermediate_strategy_project_when_loading_twice_on_bigquery_then_strategy_applies(
    tmp_path: Path,
    test_case: BigQueryIntermediateDagStrategyE2ETestCase,
) -> None:
    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_e2e_load_dag_strategy")
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
                "        type: INTEGER\n"
                "      - name: amount\n"
                "        type: INTEGER\n"
            ),
            node_files={
                **node_module_files("python/nodes/loaders/source_rows.py", test_case.loader_py)
            },
        ),
    )
    (project_dir / "sqlbuild_project.toml").write_text(
        build_bigquery_project_toml(
            project_name="source_loader_dag_strategy_behavior",
            dataset_name=dataset_name,
        ),
        encoding="utf-8",
    )
    ensure_bigquery_dataset_ready(dataset_name=dataset_name)

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
        intermediate_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT event_id, amount FROM "
                f"{relation_name(dataset_name=dataset_name, name='__loader__fetch_events')} "
                "ORDER BY event_id, amount"
            ),
        )
        terminal_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT event_id, amount FROM "
                f"{relation_name(dataset_name=dataset_name, name='raw_events')} "
                "ORDER BY event_id, amount"
            ),
        )
        assert stringify_warehouse_rows(intermediate_rows) == test_case.expected_intermediate_rows
        assert stringify_warehouse_rows(terminal_rows) == test_case.expected_terminal_rows
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQuerySourceLoaderSchemaEvolutionE2ETestCase(
            description="loader focused waffle shop grows across repeated bigquery builds",
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
def test_given_loader_waffle_shop_when_building_on_bigquery_then_dag_grows_models(
    tmp_path: Path,
    test_case: BigQuerySourceLoaderSchemaEvolutionE2ETestCase,
) -> None:
    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_e2e_load_waffle")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="loader_waffle_shop",
        repo_files=build_loader_waffle_shop_project_files(
            project_toml=build_bigquery_project_toml(
                project_name="loader_waffle_shop",
                dataset_name=dataset_name,
            )
        ),
    )
    ensure_bigquery_dataset_ready(dataset_name=dataset_name)

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

        rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dataset_name,
            sql=(
                "SELECT customer_id, plan_name, revenue_cents, order_count FROM "
                f"{relation_name(dataset_name=dataset_name, name='customer_revenue')} "
                "ORDER BY customer_id"
            ),
        )
        event_count: int = bigquery_relation_row_count(
            dataset_name=dataset_name, relation="__loader__fetch_order_events"
        )
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
        assert event_count == 4
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryModelBuildE2ETestCase(
            description="hourly_order_activity uses timestamp_trunc",
            model_name="hourly_order_activity",
            expected_sql_fragment="TIMESTAMP_TRUNC(",
        ),
        BigQueryModelBuildE2ETestCase(
            description="daily_activity_rollup uses timestamp_trunc",
            model_name="daily_activity_rollup",
            expected_sql_fragment="TIMESTAMP_TRUNC(",
        ),
        BigQueryModelBuildE2ETestCase(
            description="hourly_activity_with_daily_context uses timestamp_trunc",
            model_name="hourly_activity_with_daily_context",
            expected_sql_fragment="TIMESTAMP_TRUNC(",
        ),
        BigQueryModelBuildE2ETestCase(
            description="order_status_index uses qualified refs",
            model_name="order_status_index",
            expected_sql_fragment="`",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_waffle_shop_model_when_building_then_portable_sql_succeeds(
    tmp_path: Path,
    test_case: BigQueryModelBuildE2ETestCase,
) -> None:
    project_dir: Path
    dataset_name: str
    project_dir, dataset_name = prepare_bigquery_waffle_shop(tmp_path=tmp_path)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"),
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        compiled_files: tuple[Path, ...] = tuple(
            (project_dir / "target" / "run" / "sql" / "models").glob(
                f"**/{test_case.model_name}.sql"
            )
        )
        assert len(compiled_files) == 1
        assert test_case.expected_sql_fragment in compiled_files[0].read_text(encoding="utf-8")
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryErrorE2ETestCase(
            description="query preserves underlying error",
            command=("query", "SELECT missing_column FROM UNNEST([STRUCT(1 AS id)])"),
            expected_error_fragment="missing_column",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_invalid_query_when_running_query_then_underlying_error_is_preserved(
    tmp_path: Path,
    test_case: BigQueryErrorE2ETestCase,
) -> None:
    project_dir: Path
    dataset_name: str
    project_dir, dataset_name = prepare_bigquery_waffle_shop(tmp_path=tmp_path)
    ensure_bigquery_dataset_ready(dataset_name=dataset_name)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code
        assert test_case.expected_error_fragment in result.stdout + result.stderr
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryErrorE2ETestCase(
            description="build preserves underlying error",
            command=("--no-color", "build", "--select", "bigquery_broken_model"),
            expected_error_fragment="missing_column",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_invalid_model_when_building_then_underlying_error_is_preserved(
    tmp_path: Path,
    test_case: BigQueryErrorE2ETestCase,
) -> None:
    project_dir: Path
    dataset_name: str
    project_dir, dataset_name = prepare_bigquery_waffle_shop(tmp_path=tmp_path)
    broken_model: Path = project_dir / "sql" / "models" / "marts" / "bigquery_broken_model.sql"
    broken_model.write_text(
        "MODEL (description 'Test model bigquery_broken_model.', materialized table);\n\nSELECT missing_column FROM UNNEST([STRUCT(1 AS id)])",
        encoding="utf-8",
    )

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command,
            project_dir=project_dir,
        )

        assert result.returncode == test_case.expected_return_code
        assert test_case.expected_error_fragment in result.stdout + result.stderr
    finally:
        cleanup_bigquery_dataset(dataset_name=dataset_name)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryDiffE2ETestCase(
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
        BigQueryDiffE2ETestCase(
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
        BigQueryDiffE2ETestCase(
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
        BigQueryDiffE2ETestCase(
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
        BigQueryDiffE2ETestCase(
            description="bounded diff reports mismatch inside bounded window",
            mutation_sql=(
                "UPDATE stg_orders SET amount_cents = amount_cents + 5 WHERE order_id = 2",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--bounded",
                "2",
                "--select",
                "stg_orders",
            ),
            expected_stdout_fragments=("amount_cents", "mismatches=1", "order_id=2 | 200 -> 205"),
            expected_return_code=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_project_when_running_diff_then_outputs_expected_summary(
    tmp_path: Path,
    test_case: BigQueryDiffE2ETestCase,
) -> None:
    project_dir: Path
    prod_dataset: str
    dev_dataset: str
    project_dir, prod_dataset, dev_dataset = prepare_bigquery_diff_project(tmp_path=tmp_path)

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
            execute_bigquery_sql(
                dataset_name=dev_dataset,
                sql=statement.replace(
                    "stg_orders",
                    relation_name(dataset_name=dev_dataset, name="stg_orders"),
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
        cleanup_bigquery_dataset(dataset_name=prod_dataset)
        cleanup_bigquery_dataset(dataset_name=dev_dataset)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryCloneE2ETestCase(
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
def test_given_bigquery_project_when_cloning_then_default_uses_zero_copy_and_hard_copy_ctas(
    tmp_path: Path,
    test_case: BigQueryCloneE2ETestCase,
) -> None:
    project_dir: Path
    prod_dataset: str
    dev_dataset: str
    project_dir, prod_dataset, dev_dataset = prepare_bigquery_diff_project(tmp_path=tmp_path)

    try:
        write_local_environment_override(project_dir=project_dir, environment="prod")
        prod_build: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"),
            project_dir=project_dir,
        )
        assert prod_build.returncode == 0, prod_build.stdout + prod_build.stderr
        ensure_bigquery_dataset_ready(dataset_name=dev_dataset)

        default_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.default_command,
            project_dir=project_dir,
        )
        assert default_result.returncode == 0, default_result.stdout + default_result.stderr
        fragment: str
        for fragment in test_case.expected_default_stdout_fragments:
            assert fragment in default_result.stdout
        cloned_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dev_dataset,
            sql=(
                "SELECT order_id, customer_id, amount_cents FROM "
                f"{relation_name(dataset_name=dev_dataset, name='stg_orders')} ORDER BY order_id"
            ),
        )
        assert cloned_rows == test_case.expected_rows

        execute_bigquery_sql(
            dataset_name=dev_dataset,
            sql=f"DROP TABLE {relation_name(dataset_name=dev_dataset, name='stg_orders')}",
        )
        hard_copy_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.hard_copy_command,
            project_dir=project_dir,
        )
        assert hard_copy_result.returncode == 0, hard_copy_result.stdout + hard_copy_result.stderr
        for fragment in test_case.expected_hard_copy_stdout_fragments:
            assert fragment in hard_copy_result.stdout
        copied_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
            dataset_name=dev_dataset,
            sql=(
                "SELECT order_id, customer_id, amount_cents FROM "
                f"{relation_name(dataset_name=dev_dataset, name='stg_orders')} ORDER BY order_id"
            ),
        )
        assert copied_rows == test_case.expected_rows
    finally:
        cleanup_bigquery_dataset(dataset_name=prod_dataset)
        cleanup_bigquery_dataset(dataset_name=dev_dataset)
