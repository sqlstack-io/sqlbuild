"""Adapter contract tests for direct microbatch state SQL."""

from __future__ import annotations

from dataclasses import replace

import pytest

from sqlbuild._native import (
    MICROBATCH_TABLE_NAME,
    CursorType,
    build_microbatch_create_table_sql,
    build_microbatch_insert_many_sql,
    build_microbatch_read_scope_sql,
    deterministic_microbatch_event_id,
)
from sqlbuild._native import cursor_parse as parse
from sqlbuild._native import cursor_render as render
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from sqlbuild.microbatches.constants import MICROBATCH_COLUMNS
from sqlbuild.microbatches.models import MicrobatchEvent, MicrobatchScope
from sqlbuild.microbatches.types import MicrobatchRecordType
from tests.unit.src.sqlbuild.microbatches._test_types import (
    MicrobatchAwareOffsetIdentityTestCase,
    MicrobatchDdlAdapterTestCase,
    MicrobatchSqlBehaviorTestCase,
)
from tests.unit.src.sqlbuild.microbatches.helpers import (
    SCOPE,
    completion_for_sql,
)


@pytest.mark.parametrize(
    "test_case",
    (
        MicrobatchAwareOffsetIdentityTestCase(
            description="stored aware offsets retain deterministic identity",
            partition_start="2026-01-01T08:00:00+08:00",
            partition_end="2026-01-01T09:00:00+08:00",
            expected_event_id="781f0c13965b66aba3b4db7cfe8cb58d50e983bc5ac41b659048072de849f572",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_stored_aware_offsets_when_parsing_then_deterministic_identity_is_unchanged(
    test_case: MicrobatchAwareOffsetIdentityTestCase,
) -> None:
    stored_id: str = deterministic_microbatch_event_id(
        scope=SCOPE,
        record_type=MicrobatchRecordType.PARTITION_COMPLETION,
        partition_start=test_case.partition_start,
        partition_end=test_case.partition_end,
        completion_reason="initial:F2",
    )
    round_trip_id: str = deterministic_microbatch_event_id(
        scope=SCOPE,
        record_type=MicrobatchRecordType.PARTITION_COMPLETION,
        partition_start=render(
            value=parse(raw=test_case.partition_start, cursor_type=CursorType.TIMESTAMP)
        ),
        partition_end=render(
            value=parse(raw=test_case.partition_end, cursor_type=CursorType.TIMESTAMP)
        ),
        completion_reason="initial:F2",
    )

    assert stored_id == test_case.expected_event_id
    assert round_trip_id == test_case.expected_event_id


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchDdlAdapterTestCase(
            description="duckdb", adapter=DuckDbAdapter(), expected_table_name=MICROBATCH_TABLE_NAME
        ),
        MicrobatchDdlAdapterTestCase(
            description="snowflake",
            adapter=SnowflakeAdapter(),
            expected_table_name=MICROBATCH_TABLE_NAME,
        ),
        MicrobatchDdlAdapterTestCase(
            description="bigquery",
            adapter=BigQueryAdapter(),
            expected_table_name=MICROBATCH_TABLE_NAME,
        ),
        MicrobatchDdlAdapterTestCase(
            description="databricks",
            adapter=DatabricksAdapter(),
            expected_table_name=MICROBATCH_TABLE_NAME,
        ),
        MicrobatchDdlAdapterTestCase(
            description="postgres",
            adapter=PostgresAdapter(),
            expected_table_name=MICROBATCH_TABLE_NAME,
        ),
        MicrobatchDdlAdapterTestCase(
            description="sqlserver",
            adapter=SqlServerAdapter(),
            expected_table_name=MICROBATCH_TABLE_NAME,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_builtin_adapter_when_building_state_ddl_then_all_columns_are_declared(
    test_case: MicrobatchDdlAdapterTestCase,
) -> None:
    adapter: BaseAdapter = test_case.adapter
    sql: str = build_microbatch_create_table_sql(
        database="warehouse",
        schema="analytics",
        render_qualified_name=adapter.render_qualified_name,
        render_framework_type=adapter.render_framework_type,
    )

    assert "CREATE TABLE IF NOT EXISTS" in sql
    assert test_case.expected_table_name in sql
    for column in MICROBATCH_COLUMNS:
        assert column in sql
    assert "rows_affected BIGINT" in sql
    assert "observed_row_count BIGINT" in sql


@pytest.mark.parametrize(
    "test_case",
    [MicrobatchSqlBehaviorTestCase(description="sqlserver guard", expected_statement_count=1)],
    ids=lambda case: case.description,
)
def test_given_sqlserver_adapter_when_rendering_state_ddl_then_uses_supported_guard(
    test_case: MicrobatchSqlBehaviorTestCase,
) -> None:
    sql: str = SqlServerAdapter().render_create_microbatch_state_table_sql(
        database="warehouse", schema="analytics"
    )

    assert sql.startswith("IF NOT EXISTS (SELECT 1 FROM information_schema.tables")
    assert "CREATE TABLE warehouse.analytics._sqlbuild_microbatches" in sql
    assert "CREATE TABLE IF NOT EXISTS" not in sql
    assert len((sql,)) == test_case.expected_statement_count


@pytest.mark.parametrize(
    "test_case",
    [MicrobatchSqlBehaviorTestCase(description="indexed access paths", expected_statement_count=5)],
    ids=lambda case: case.description,
)
def test_given_indexed_adapter_when_rendering_state_indexes_then_all_access_paths_are_covered(
    test_case: MicrobatchSqlBehaviorTestCase,
) -> None:
    sqls: tuple[str, ...] = DuckDbAdapter().render_create_microbatch_state_index_sqls(
        database=None, schema="analytics"
    )

    assert len(sqls) == test_case.expected_statement_count
    assert all("CREATE INDEX IF NOT EXISTS" in sql for sql in sqls)
    assert any("scope_kind" in sql and "physical_generation_id" in sql for sql in sqls)
    assert any("replay_requirement_id" in sql for sql in sqls)
    assert any("partition_start" in sql and "partition_end" in sql for sql in sqls)


@pytest.mark.parametrize(
    "test_case",
    [MicrobatchSqlBehaviorTestCase(description="completion insert", expected_statement_count=1)],
    ids=lambda case: case.description,
)
def test_given_completion_when_building_insert_then_provenance_and_numeric_rows_are_encoded(
    test_case: MicrobatchSqlBehaviorTestCase,
) -> None:
    event: MicrobatchEvent = completion_for_sql()

    sql: str = build_microbatch_insert_many_sql(
        events=(event,), render_qualified_name=DuckDbAdapter().render_qualified_name
    )

    assert "INSERT INTO analytics._sqlbuild_microbatches" in sql
    assert "'partition_completion'" in sql
    assert "'replay_on_change'" in sql
    assert "'recovery'" in sql
    assert "'F2'" in sql
    assert "'fingerprint''definition'" in sql
    assert sql == (
        "INSERT INTO analytics._sqlbuild_microbatches (event_id, record_type, scope_kind, "
        "scope_key, model_name, target_database, target_schema, target_name, "
        "physical_generation_id, origin_run_id, origin_run_started_at, execution_run_id, "
        "execution_run_started_at, run_type, completion_type, run_start, run_end, "
        "partition_start, partition_end, batch_size, cursor_column, cursor_type, "
        "cursor_grain, model_version_hash, definition_hash, fingerprint_status, "
        "replay_requirement_id, required_model_version_hash, previous_model_version_hash, "
        "replay_policy, rows_affected, completed_at, coverage_source, observed_row_count, "
        "observed_at, synthetic_reason, unaccounted_policy, created_at) SELECT event_id, "
        "record_type, scope_kind, scope_key, model_name, target_database, target_schema, "
        "target_name, physical_generation_id, origin_run_id, origin_run_started_at, "
        "execution_run_id, execution_run_started_at, run_type, completion_type, run_start, "
        "run_end, partition_start, partition_end, batch_size, cursor_column, cursor_type, "
        "cursor_grain, model_version_hash, definition_hash, fingerprint_status, "
        "replay_requirement_id, required_model_version_hash, previous_model_version_hash, "
        "replay_policy, rows_affected, completed_at, coverage_source, observed_row_count, "
        "observed_at, synthetic_reason, unaccounted_policy, created_at FROM (SELECT 'event-1'"
        " AS event_id, 'partition_completion' AS record_type, 'direct_logical' AS scope_kind,"
        " 'duckdb:analytics.orders' AS scope_key, 'orders' AS model_name, NULL AS "
        "target_database, 'analytics' AS target_schema, 'orders' AS target_name, '*' AS "
        "physical_generation_id, 'origin-run' AS origin_run_id, CAST(NULL AS TIMESTAMP) AS "
        "origin_run_started_at, 'execution-run' AS execution_run_id, CAST(NULL AS TIMESTAMP) "
        "AS execution_run_started_at, 'replay_on_change' AS run_type, 'recovery' AS "
        "completion_type, '0' AS run_start, '1' AS run_end, '0' AS partition_start, '1' AS "
        "partition_end, '1' AS batch_size, 'batch_id' AS cursor_column, 'integer' AS "
        "cursor_type, NULL AS cursor_grain, 'F2' AS model_version_hash, "
        "'fingerprint''definition' AS definition_hash, 'known' AS fingerprint_status, NULL AS"
        " replay_requirement_id, NULL AS required_model_version_hash, NULL AS "
        "previous_model_version_hash, NULL AS replay_policy, CAST(0 AS BIGINT) AS "
        "rows_affected, CAST(NULL AS TIMESTAMP) AS completed_at, NULL AS coverage_source, "
        "CAST(NULL AS BIGINT) AS observed_row_count, CAST(NULL AS TIMESTAMP) AS observed_at, "
        "NULL AS synthetic_reason, NULL AS unaccounted_policy, "
        "CAST('2026-01-01T00:00:00+00:00' AS TIMESTAMP) AS created_at) AS incoming WHERE NOT "
        "EXISTS (SELECT 1 FROM analytics._sqlbuild_microbatches AS existing WHERE "
        "existing.event_id = incoming.event_id)"
    )
    assert len((sql,)) == test_case.expected_statement_count


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchDdlAdapterTestCase(
            description="duckdb", adapter=DuckDbAdapter(), expected_table_name=MICROBATCH_TABLE_NAME
        ),
        MicrobatchDdlAdapterTestCase(
            description="snowflake",
            adapter=SnowflakeAdapter(),
            expected_table_name=MICROBATCH_TABLE_NAME,
        ),
        MicrobatchDdlAdapterTestCase(
            description="bigquery",
            adapter=BigQueryAdapter(),
            expected_table_name=MICROBATCH_TABLE_NAME,
        ),
        MicrobatchDdlAdapterTestCase(
            description="databricks",
            adapter=DatabricksAdapter(),
            expected_table_name=MICROBATCH_TABLE_NAME,
        ),
        MicrobatchDdlAdapterTestCase(
            description="postgres",
            adapter=PostgresAdapter(),
            expected_table_name=MICROBATCH_TABLE_NAME,
        ),
        MicrobatchDdlAdapterTestCase(
            description="sqlserver",
            adapter=SqlServerAdapter(),
            expected_table_name=MICROBATCH_TABLE_NAME,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_builtin_adapter_when_rendering_direct_event_inserts_then_typed_columns_are_cast(
    test_case: MicrobatchDdlAdapterTestCase,
) -> None:
    event: MicrobatchEvent = completion_for_sql()

    single_sql: str = build_microbatch_insert_many_sql(
        events=(event,), render_qualified_name=test_case.adapter.render_qualified_name
    )
    bulk_sql: str = build_microbatch_insert_many_sql(
        events=(event, replace(event, event_id="second-event")),
        render_qualified_name=test_case.adapter.render_qualified_name,
    )

    for sql, expected_row_count in ((single_sql, 1), (bulk_sql, 2)):
        assert test_case.expected_table_name in sql
        assert "CAST('2026-01-01T00:00:00+00:00' AS TIMESTAMP)" in sql
        assert sql.count("CAST(NULL AS TIMESTAMP)") == 4 * expected_row_count
        assert sql.count("CAST(0 AS BIGINT)") == expected_row_count
        assert sql.count("CAST(NULL AS BIGINT)") == expected_row_count


@pytest.mark.parametrize(
    "test_case",
    [MicrobatchSqlBehaviorTestCase(description="generation filters", expected_statement_count=2)],
    ids=lambda case: case.description,
)
def test_given_wildcard_and_concrete_generations_when_reading_then_filter_is_optional(
    test_case: MicrobatchSqlBehaviorTestCase,
) -> None:
    scope: MicrobatchScope = completion_for_sql().scope

    wildcard_sql: str = build_microbatch_read_scope_sql(
        scope=scope, render_qualified_name=DuckDbAdapter().render_qualified_name
    )
    concrete_sql: str = build_microbatch_read_scope_sql(
        scope=MicrobatchScope(
            scope_kind=scope.scope_kind,
            scope_key=scope.scope_key,
            model_name=scope.model_name,
            target_database=scope.target_database,
            target_schema=scope.target_schema,
            target_name=scope.target_name,
            physical_generation_id="generation-1",
        ),
        render_qualified_name=DuckDbAdapter().render_qualified_name,
    )

    assert "physical_generation_id =" not in wildcard_sql
    assert "physical_generation_id = 'generation-1'" in concrete_sql
    assert wildcard_sql.endswith("ORDER BY created_at, event_id")
    assert len((wildcard_sql, concrete_sql)) == test_case.expected_statement_count


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchSqlBehaviorTestCase(
            description="deterministic identity", expected_statement_count=4
        )
    ],
    ids=lambda case: case.description,
)
def test_given_logical_event_identity_when_constructing_ids_then_ids_are_stable_and_distinct(
    test_case: MicrobatchSqlBehaviorTestCase,
) -> None:
    first: str = deterministic_microbatch_event_id(
        scope=SCOPE,
        record_type=MicrobatchRecordType.SYNTHETIC_COMPLETION,
        partition_start="1",
        partition_end="2",
        completion_reason="completion_history_missing:synthesize",
    )
    independent: str = deterministic_microbatch_event_id(
        scope=SCOPE,
        record_type=MicrobatchRecordType.SYNTHETIC_COMPLETION,
        partition_start="1",
        partition_end="2",
        completion_reason="completion_history_missing:synthesize",
    )
    distinct: tuple[str, ...] = (
        first,
        deterministic_microbatch_event_id(
            scope=SCOPE,
            record_type=MicrobatchRecordType.SYNTHETIC_COMPLETION,
            partition_start="2",
            partition_end="3",
            completion_reason="completion_history_missing:synthesize",
        ),
        deterministic_microbatch_event_id(
            scope=SCOPE,
            record_type=MicrobatchRecordType.SYNTHETIC_COMPLETION,
            partition_start="1",
            partition_end="2",
            completion_reason="completion_history_missing:recover_empty",
        ),
        deterministic_microbatch_event_id(
            scope=replace(SCOPE, physical_generation_id="generation-2"),
            record_type=MicrobatchRecordType.SYNTHETIC_COMPLETION,
            partition_start="1",
            partition_end="2",
            completion_reason="completion_history_missing:synthesize",
        ),
    )

    assert first == independent
    assert len(set(distinct)) == test_case.expected_statement_count
