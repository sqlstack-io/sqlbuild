from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse._test_types import (
    ClickHouseBuildE2ETestCase,
    ClickHouseSnapshotApplyE2ETestCase,
    ClickHouseSnapshotE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse.helpers import (
    build_clickhouse_project_toml,
    build_orders_project_files,
    build_unique_schema_name,
    cleanup_clickhouse_schema,
    create_clickhouse_schema,
    execute_clickhouse_sql,
    fetch_clickhouse_rows,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
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

_FIRST_EVENTS: str = (
    "(1, 1, 100, '2026-01-01 08:00:00'), (2, 2, 200, '2026-01-01 09:00:00'), "
    "(3, 1, 50, '2026-01-02 10:00:00'), (4, 3, 300, '2026-01-02 11:00:00'), "
    "(5, 2, 25, '2026-01-03 12:00:00'), (6, 3, 75, '2026-01-04 13:00:00')"
)
_LATER_EVENTS: str = "(7, 1, 10, '2026-01-05 08:00:00'), (8, 2, 20, '2026-01-06 09:00:00')"


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseBuildE2ETestCase(
            description="views, tables, seeds, functions, merge and concurrent microbatches",
            expected_customer_rows=(
                ("1", "north", "150", "75.0"),
                ("2", "south", "225", "112.5"),
                ("3", "east", "375", "187.5"),
            ),
            expected_order_totals=(("6", "750"),),
            expected_name_rows=(("1", "ada lovelace"), ("2", "bo"), ("3", "cy")),
            expected_daily_rows_after_first_build=(
                ("2026-01-01", "2"),
                ("2026-01-02", "2"),
                ("2026-01-03", "1"),
                ("2026-01-04", "1"),
            ),
            expected_daily_rows_after_second_build=(
                ("2026-01-01", "2"),
                ("2026-01-02", "2"),
                ("2026-01-03", "1"),
                ("2026-01-04", "1"),
                ("2026-01-05", "1"),
                ("2026-01-06", "1"),
            ),
            expected_latest_rows=(
                ("1", "3", "2026-01-05 08:00:00"),
                ("2", "3", "2026-01-06 09:00:00"),
                ("3", "2", "2026-01-04 13:00:00"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_orders_project_when_building_twice_on_clickhouse_then_every_strategy_lands(
    tmp_path: Path,
    test_case: ClickHouseBuildE2ETestCase,
    clickhouse_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_orders")
    config: dict[str, object] = clickhouse_e2e_config
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clickhouse_orders",
        repo_files=build_orders_project_files(schema_name=schema_name, config=config),
    )
    create_clickhouse_schema(config=config, schema_name=schema_name)
    execute_clickhouse_sql(
        config=config,
        statements=(
            f"CREATE TABLE {schema_name}.raw_events (event_id Int64, customer_id Int64, "
            "amount Int64, event_time DateTime64(6, 'UTC')) ENGINE = MergeTree ORDER BY event_id",
            f"INSERT INTO {schema_name}.raw_events VALUES {_FIRST_EVENTS}",
            f"CREATE TABLE {schema_name}.raw_customers (customer_id Int64, "
            "customer_name String, version Int64) ENGINE = ReplacingMergeTree(version) "
            "ORDER BY customer_id",
            f"SYSTEM STOP MERGES {schema_name}.raw_customers",
            f"INSERT INTO {schema_name}.raw_customers VALUES (1, 'ada', 1), (2, 'bo', 1)",
            f"INSERT INTO {schema_name}.raw_customers VALUES (1, 'ada lovelace', 2), (3, 'cy', 1)",
        ),
    )
    try:
        first: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )
        assert first.returncode == 0, first.stdout + first.stderr
        assert stringify_warehouse_rows(
            fetch_clickhouse_rows(
                config=config,
                sql=f"SELECT customer_id, region_name, total_amount, half_total "
                f"FROM {schema_name}.customer_totals ORDER BY customer_id",
            )
        ) == (test_case.expected_customer_rows)
        assert stringify_warehouse_rows(
            fetch_clickhouse_rows(
                config=config,
                sql=f"SELECT count(), sum(amount) FROM {schema_name}.stg_events",
            )
        ) == (test_case.expected_order_totals)
        assert fetch_clickhouse_rows(
            config=config, sql=f"SELECT count() FROM {schema_name}.raw_customers"
        ) == ((4,),)
        assert stringify_warehouse_rows(
            fetch_clickhouse_rows(
                config=config,
                sql=f"SELECT customer_id, customer_name FROM {schema_name}.customer_names "
                "ORDER BY customer_id",
            )
        ) == (test_case.expected_name_rows)
        daily_sql: str = (
            f"SELECT toDate(event_time) AS day, count() FROM {schema_name}.daily_events "
            "GROUP BY day ORDER BY day"
        )
        assert stringify_warehouse_rows(fetch_clickhouse_rows(config=config, sql=daily_sql)) == (
            test_case.expected_daily_rows_after_first_build
        )

        execute_clickhouse_sql(
            config=config,
            statements=(f"INSERT INTO {schema_name}.raw_events VALUES {_LATER_EVENTS}",),
        )
        second: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )
        assert second.returncode == 0, second.stdout + second.stderr
        assert "Concurrent microbatch execution is enabled" in second.stdout + second.stderr
        assert stringify_warehouse_rows(fetch_clickhouse_rows(config=config, sql=daily_sql)) == (
            test_case.expected_daily_rows_after_second_build
        )
        assert stringify_warehouse_rows(
            fetch_clickhouse_rows(
                config=config,
                sql=f"SELECT customer_id, event_count, toString(toDateTime(last_event_time)) "
                f"FROM {schema_name}.customer_latest ORDER BY customer_id",
            )
        ) == (test_case.expected_latest_rows)
        artifacts: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
            config=config,
            sql=f"SELECT name FROM system.tables WHERE database = '{schema_name}' "
            "AND (name LIKE '%__sqb_%' OR name LIKE '__sqb_%')",
        )
        assert artifacts == ()
        state_engines: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
            config=config,
            sql=f"SELECT DISTINCT engine FROM system.tables WHERE database = '{schema_name}' "
            "AND startsWith(name, '_sqlbuild_')",
        )
        assert state_engines == (("MergeTree",),)

        query: subprocess.CompletedProcess[str] = run_sqb(
            command=(
                "--no-color",
                "query",
                f"SELECT sum(total_amount) AS revenue FROM {schema_name}.customer_totals",
            ),
            project_dir=project_dir,
        )
        assert query.returncode == 0, query.stdout + query.stderr
        assert "revenue" in query.stdout and "780" in query.stdout
    finally:
        cleanup_clickhouse_schema(config=config, schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseSnapshotE2ETestCase(
            description="executes the snapshot matrix on clickhouse",
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
def test_given_snapshot_project_when_building_on_clickhouse_then_scd2_history_is_valid(
    tmp_path: Path,
    test_case: ClickHouseSnapshotE2ETestCase,
    clickhouse_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_snapshot")
    config: dict[str, object] = clickhouse_e2e_config
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clickhouse_snapshot_project",
        repo_files=build_real_warehouse_snapshot_project_files(
            project_toml=build_clickhouse_project_toml(
                project_name="clickhouse_snapshot_project",
                schema_name=schema_name,
                config=config,
            )
        ),
    )
    create_clickhouse_schema(config=config, schema_name=schema_name)
    current_sql: str = (
        "SELECT customer_id, region_id, plan, toDate(effective_from), toDate(effective_to) "
        f"FROM {schema_name}.current_customer_snapshot "
        "ORDER BY customer_id, region_id, effective_from"
    )
    try:
        initial: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"), project_dir=project_dir
        )
        assert initial.returncode == 0, initial.stdout + initial.stderr
        assert stringify_warehouse_rows(fetch_clickhouse_rows(config=config, sql=current_sql)) == (
            test_case.expected_current_rows_after_initial_build
        )
        assert stringify_warehouse_rows(
            fetch_clickhouse_rows(
                config=config,
                sql="SELECT customer_id, plan, toDate(valid_from), toDate(valid_to) "
                f"FROM {schema_name}.historical_customer_snapshot ORDER BY customer_id, valid_from",
            )
        ) == (test_case.expected_historical_timestamp_rows)
        assert stringify_warehouse_rows(
            fetch_clickhouse_rows(
                config=config,
                sql="SELECT customer_id, status, toDate(valid_from), toDate(valid_to) "
                f"FROM {schema_name}.historical_membership_snapshot "
                "ORDER BY customer_id, valid_from",
            )
        ) == (test_case.expected_historical_check_rows)

        (project_dir / "sql" / "models" / "current_customers.sql").write_text(
            build_current_customers_model_sql(plan="blocked", updated_at="2026-01-02 00:00:00"),
            encoding="utf-8",
        )
        failure: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--select", "+current_customer_snapshot"),
            project_dir=project_dir,
        )
        assert failure.returncode == 1, failure.stdout + failure.stderr
        for fragment in test_case.expected_failure_fragments:
            assert fragment in failure.stdout + failure.stderr
        assert stringify_warehouse_rows(fetch_clickhouse_rows(config=config, sql=current_sql)) == (
            test_case.expected_current_rows_after_initial_build
        )

        (project_dir / "sql" / "models" / "current_customers.sql").write_text(
            build_current_customers_model_sql(plan="pro", updated_at="2026-01-02 00:00:00"),
            encoding="utf-8",
        )
        recovery: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--select", "+current_customer_snapshot"),
            project_dir=project_dir,
        )
        assert recovery.returncode == 0, recovery.stdout + recovery.stderr
        assert stringify_warehouse_rows(fetch_clickhouse_rows(config=config, sql=current_sql)) == (
            test_case.expected_current_rows_after_recovery
        )
    finally:
        cleanup_clickhouse_schema(config=config, schema_name=schema_name)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseSnapshotApplyE2ETestCase(
            description="applies existing-target snapshot changes on clickhouse",
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
def test_given_existing_snapshot_targets_when_building_on_clickhouse_then_changes_apply(
    tmp_path: Path,
    test_case: ClickHouseSnapshotApplyE2ETestCase,
    clickhouse_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_snapshot_apply")
    config: dict[str, object] = clickhouse_e2e_config
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clickhouse_snapshot_apply_project",
        repo_files=build_real_warehouse_existing_snapshot_project_files(
            project_toml=build_clickhouse_project_toml(
                project_name="clickhouse_snapshot_apply_project",
                schema_name=schema_name,
                config=config,
            )
        ),
    )
    create_clickhouse_schema(config=config, schema_name=schema_name)
    try:
        initial: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"), project_dir=project_dir
        )
        assert initial.returncode == 0, initial.stdout + initial.stderr
        models: Path = project_dir / "sql" / "models"
        (models / "current_check_customers.sql").write_text(
            build_current_check_customers_model_sql(changed=True), encoding="utf-8"
        )
        (models / "current_delete_customers.sql").write_text(
            build_current_delete_customers_model_sql(changed=True), encoding="utf-8"
        )
        (models / "historical_timestamp_extracts.sql").write_text(
            build_historical_timestamp_extracts_model_sql(changed=True), encoding="utf-8"
        )
        (models / "historical_check_daily.sql").write_text(
            build_historical_check_daily_model_sql(changed=True), encoding="utf-8"
        )

        applied: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--concurrency", "4"), project_dir=project_dir
        )
        assert applied.returncode == 0, applied.stdout + applied.stderr
        for name, columns, expected in (
            ("current_check_snapshot", "status", test_case.expected_current_check_rows),
            ("current_delete_snapshot", "plan", test_case.expected_current_delete_rows),
        ):
            rows: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
                config=config,
                sql=f"SELECT customer_id, {columns}, valid_to IS NULL FROM {schema_name}.{name} "
                f"ORDER BY customer_id, {columns}, valid_to IS NULL",
            )
            assert stringify_warehouse_rows(
                tuple((row[0], row[1], bool(row[2])) for row in rows)
            ) == (expected)
        for name, columns, expected in (
            ("historical_timestamp_snapshot", "plan", test_case.expected_historical_timestamp_rows),
            ("historical_check_snapshot", "status", test_case.expected_historical_check_rows),
        ):
            rows = fetch_clickhouse_rows(
                config=config,
                sql=f"SELECT customer_id, {columns}, toDate(valid_from), toDate(valid_to) "
                f"FROM {schema_name}.{name} ORDER BY customer_id, valid_from",
            )
            assert stringify_warehouse_rows(rows) == expected
        leftovers: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
            config=config,
            sql=f"SELECT name FROM system.tables WHERE database = '{schema_name}' "
            "AND name LIKE '%__sqb_snap%'",
        )
        assert leftovers == ()
    finally:
        cleanup_clickhouse_schema(config=config, schema_name=schema_name)
