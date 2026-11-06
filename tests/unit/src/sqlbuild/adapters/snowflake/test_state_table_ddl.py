from __future__ import annotations

from collections.abc import Callable
from typing import Any, cast

import pytest

from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from tests.unit.src.sqlbuild.adapters.snowflake._test_types import (
    SnowflakeNonStateRetentionErrorTestCase,
    SnowflakeStateTableDdlTestCase,
    SnowflakeStateTableExactDdlTestCase,
    SnowflakeStateTableFallbackLifecycleTestCase,
    SnowflakeStateTableFallbackTestCase,
    SnowflakeStateTableRetentionErrorTestCase,
    SnowflakeStateTableSchemaFallbackTestCase,
)
from tests.unit.src.sqlbuild.adapters.snowflake.helpers import (
    CustomInitSnowflakeAdapter,
    FakeSnowflakeRejectingConnection,
    FakeSnowflakeRejectingRawCursor,
    StatementProgressCapture,
    execute_with_statement_progress,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeSnowflakeProgrammingError,
)

_RETENTION_90: str = "DATA_RETENTION_TIME_IN_DAYS = 90"
_STANDARD_EDITION_REJECTION: str = (
    "001008 (22023): SQL compilation error:\n"
    "invalid value [90] for parameter 'DATA_RETENTION_TIME_IN_DAYS'"
)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeStateTableDdlTestCase(
            description="fingerprints",
            render_method="render_create_fingerprint_table_sql",
            expected_prefix="CREATE TABLE IF NOT EXISTS analytics.marts._sqlbuild_fingerprints (",
            expected_suffix=") DATA_RETENTION_TIME_IN_DAYS = 90",
        ),
        SnowflakeStateTableDdlTestCase(
            description="source freshness",
            render_method="render_create_source_freshness_table_sql",
            expected_prefix=(
                "CREATE TABLE IF NOT EXISTS analytics.marts._sqlbuild_source_freshness ("
            ),
            expected_suffix=") DATA_RETENTION_TIME_IN_DAYS = 90",
        ),
        SnowflakeStateTableDdlTestCase(
            description="node results",
            render_method="render_create_node_result_table_sql",
            expected_prefix="CREATE TABLE IF NOT EXISTS analytics.marts._sqlbuild_node_results (",
            expected_suffix=") DATA_RETENTION_TIME_IN_DAYS = 90",
        ),
        SnowflakeStateTableDdlTestCase(
            description="audit results",
            render_method="render_create_audit_result_table_sql",
            expected_prefix="CREATE TABLE IF NOT EXISTS analytics.marts._sqlbuild_audit_results (",
            expected_suffix=") DATA_RETENTION_TIME_IN_DAYS = 90",
        ),
        SnowflakeStateTableDdlTestCase(
            description="janitor events",
            render_method="render_create_janitor_event_table_sql",
            expected_prefix="CREATE TABLE IF NOT EXISTS analytics.marts._sqlbuild_janitor_events (",
            expected_suffix=") DATA_RETENTION_TIME_IN_DAYS = 90",
        ),
        SnowflakeStateTableDdlTestCase(
            description="model migrations",
            render_method="render_create_migration_state_table_sql",
            expected_prefix="CREATE TABLE IF NOT EXISTS analytics.marts._sqlbuild_migrations (",
            expected_suffix=") DATA_RETENTION_TIME_IN_DAYS = 90",
        ),
        SnowflakeStateTableDdlTestCase(
            description="old-name views",
            render_method="render_create_old_name_view_state_table_sql",
            expected_prefix="CREATE TABLE IF NOT EXISTS analytics.marts._sqlbuild_old_name_views (",
            expected_suffix=") DATA_RETENTION_TIME_IN_DAYS = 90",
        ),
        SnowflakeStateTableDdlTestCase(
            description="column migrations",
            render_method="render_create_column_migration_state_table_sql",
            expected_prefix=(
                "CREATE TABLE IF NOT EXISTS analytics.marts._sqlbuild_column_migrations ("
            ),
            expected_suffix=") DATA_RETENTION_TIME_IN_DAYS = 90",
        ),
        SnowflakeStateTableDdlTestCase(
            description="microbatches",
            render_method="render_create_microbatch_state_table_sql",
            expected_prefix="CREATE TABLE IF NOT EXISTS analytics.marts._sqlbuild_microbatches (",
            expected_suffix=") DATA_RETENTION_TIME_IN_DAYS = 90",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_state_table_when_rendering_on_snowflake_then_permanent_with_90_day_retention(
    test_case: SnowflakeStateTableDdlTestCase,
) -> None:
    render: Callable[..., str] = getattr(SnowflakeAdapter(), test_case.render_method)

    sql: str = render(database="analytics", schema="marts")

    assert sql.startswith(test_case.expected_prefix)
    assert sql.endswith(test_case.expected_suffix)
    assert "TRANSIENT" not in sql


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeStateTableExactDdlTestCase(
            description="fingerprints",
            render_method="render_create_fingerprint_table_sql",
            expected_sql=(
                "CREATE TABLE IF NOT EXISTS analytics.marts._sqlbuild_fingerprints ("
                "node_type VARCHAR NOT NULL, node_name VARCHAR NOT NULL, target_database VARCHAR, "
                "target_schema VARCHAR, target_name VARCHAR, run_id VARCHAR NOT NULL, "
                "definition_hash VARCHAR NOT NULL, version_hash VARCHAR NOT NULL, "
                "schema_fingerprint VARCHAR NOT NULL, definition_b64 VARCHAR NOT NULL, "
                "metadata_json_b64 VARCHAR NOT NULL, ts TIMESTAMP NOT NULL, event_id VARCHAR"
                ") DATA_RETENTION_TIME_IN_DAYS = 90"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_state_table_when_rendering_on_snowflake_then_matches_exact_ddl(
    test_case: SnowflakeStateTableExactDdlTestCase,
) -> None:
    render: Callable[..., str] = getattr(SnowflakeAdapter(), test_case.render_method)

    assert render(database="analytics", schema="marts") == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeStateTableFallbackTestCase(
            description="standard edition rejects 90 days",
            adapter_type=SnowflakeAdapter,
            rejection_errno=1008,
            rejection_message=_STANDARD_EDITION_REJECTION,
            expected_executed_tables=(
                "analytics.marts._sqlbuild_fingerprints",
                "analytics.marts._sqlbuild_fingerprints",
                "analytics.marts._sqlbuild_node_results",
                "analytics.marts._sqlbuild_microbatches",
            ),
            expected_executed_retention_days=("90", "1", "1", "1"),
        ),
        SnowflakeStateTableFallbackTestCase(
            description="custom adapter initializer without super",
            adapter_type=CustomInitSnowflakeAdapter,
            rejection_errno=1008,
            rejection_message=_STANDARD_EDITION_REJECTION,
            expected_executed_tables=(
                "analytics.marts._sqlbuild_fingerprints",
                "analytics.marts._sqlbuild_fingerprints",
                "analytics.marts._sqlbuild_node_results",
                "analytics.marts._sqlbuild_microbatches",
            ),
            expected_executed_retention_days=("90", "1", "1", "1"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_account_rejecting_90_days_when_creating_state_tables_then_retries_once_with_1(
    test_case: SnowflakeStateTableFallbackTestCase,
) -> None:
    adapter: SnowflakeAdapter = test_case.adapter_type()
    connection: FakeSnowflakeRejectingConnection = FakeSnowflakeRejectingConnection(
        rejected_fragments=(_RETENTION_90,),
        error=FakeSnowflakeProgrammingError(
            test_case.rejection_message, errno=test_case.rejection_errno
        ),
    )
    fingerprints_sql: str = adapter.render_create_fingerprint_table_sql(
        database="analytics", schema="marts"
    )
    rendered_before_fallback: str = adapter.render_create_node_result_table_sql(
        database="analytics", schema="marts"
    )

    _ = adapter.execute(connection=cast(Any, connection), sql=fingerprints_sql)
    _ = adapter.execute(connection=cast(Any, connection), sql=rendered_before_fallback)
    _ = adapter.execute(
        connection=cast(Any, connection),
        sql=adapter.render_create_microbatch_state_table_sql(database="analytics", schema="marts"),
    )

    assert (
        tuple(
            sql.removeprefix("CREATE TABLE IF NOT EXISTS ").split(" (", 1)[0]
            for sql in connection.executed_sql
        )
        == test_case.expected_executed_tables
    )
    assert (
        tuple(sql.rsplit(" = ", 1)[1] for sql in connection.executed_sql)
        == test_case.expected_executed_retention_days
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeStateTableRetentionErrorTestCase(
            description="same errno for a different parameter",
            errno=1008,
            message=(
                "001008 (22023): SQL compilation error:\n"
                "invalid value [90] for parameter 'MAX_DATA_EXTENSION_TIME_IN_DAYS'"
            ),
            expected_executed_count=1,
            expected_next_retention_days="90",
        ),
        SnowflakeStateTableRetentionErrorTestCase(
            description="retention parameter named by a different error",
            errno=3001,
            message=(
                "003001 (42501): SQL access control error: insufficient privileges to set "
                "parameter 'DATA_RETENTION_TIME_IN_DAYS'"
            ),
            expected_executed_count=1,
            expected_next_retention_days="90",
        ),
        SnowflakeStateTableRetentionErrorTestCase(
            description="missing schema",
            errno=2003,
            message=(
                "002003 (02000): SQL compilation error: Schema 'ANALYTICS.MARTS' does not exist"
            ),
            expected_executed_count=1,
            expected_next_retention_days="90",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unrelated_error_when_creating_state_table_then_propagates_without_retry(
    test_case: SnowflakeStateTableRetentionErrorTestCase,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    error: FakeSnowflakeProgrammingError = FakeSnowflakeProgrammingError(
        test_case.message, errno=test_case.errno
    )
    connection: FakeSnowflakeRejectingConnection = FakeSnowflakeRejectingConnection(
        rejected_fragments=(_RETENTION_90,), error=error
    )
    sql: str = adapter.render_create_audit_result_table_sql(database="analytics", schema="marts")

    with pytest.raises(FakeSnowflakeProgrammingError) as raised:
        _ = adapter.execute(connection=cast(Any, connection), sql=sql)

    assert raised.value is error
    assert len(connection.executed_sql) == test_case.expected_executed_count
    assert (
        adapter.render_create_audit_result_table_sql(database="analytics", schema="marts").rsplit(
            " = ", 1
        )[1]
        == test_case.expected_next_retention_days
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeNonStateRetentionErrorTestCase(
            description="model retention change",
            sql="ALTER TABLE analytics.marts.orders SET DATA_RETENTION_TIME_IN_DAYS = 90",
            expected_executed_sql=(
                "ALTER TABLE analytics.marts.orders SET DATA_RETENTION_TIME_IN_DAYS = 90",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_retention_rejection_when_statement_is_not_state_table_ddl_then_propagates(
    test_case: SnowflakeNonStateRetentionErrorTestCase,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    connection: FakeSnowflakeRejectingConnection = FakeSnowflakeRejectingConnection(
        rejected_fragments=(_RETENTION_90,),
        error=FakeSnowflakeProgrammingError(_STANDARD_EDITION_REJECTION, errno=1008),
    )

    with pytest.raises(FakeSnowflakeProgrammingError):
        _ = adapter.execute(connection=cast(Any, connection), sql=test_case.sql)

    assert tuple(connection.executed_sql) == test_case.expected_executed_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeStateTableFallbackLifecycleTestCase(
            description="retry with 1 day succeeds",
            rejections=(
                (
                    (_RETENTION_90,),
                    FakeSnowflakeProgrammingError(_STANDARD_EDITION_REJECTION, errno=1008),
                ),
            ),
            expected_statement_events=(
                "statement_started",
                "statement_submitted",
                "statement_completed",
            ),
            expected_progress_fail_lines=0,
            expected_executed_retention_days=("90", "1"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fallback_succeeds_when_executing_through_cursor_then_one_successful_statement(
    test_case: SnowflakeStateTableFallbackLifecycleTestCase,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    cursor: FakeSnowflakeRejectingRawCursor = FakeSnowflakeRejectingRawCursor(
        rejections=test_case.rejections
    )
    sql: str = adapter.render_create_fingerprint_table_sql(database="analytics", schema="marts")

    capture: StatementProgressCapture = execute_with_statement_progress(
        adapter=adapter, raw_cursor=cursor, sql=sql
    )

    assert capture.error is None
    assert capture.statement_events == test_case.expected_statement_events
    assert len(capture.statement_fail_lines) == test_case.expected_progress_fail_lines
    assert "statement  " in capture.output
    assert (
        tuple(statement.rsplit(" = ", 1)[1] for statement in cursor.executed_sql)
        == test_case.expected_executed_retention_days
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeStateTableFallbackLifecycleTestCase(
            description="retry with 1 day also fails",
            rejections=(
                (
                    (_RETENTION_90,),
                    FakeSnowflakeProgrammingError(_STANDARD_EDITION_REJECTION, errno=1008),
                ),
                (
                    ("DATA_RETENTION_TIME_IN_DAYS = 1",),
                    FakeSnowflakeProgrammingError(
                        "003001 (42501): SQL access control error: Insufficient privileges",
                        errno=3001,
                    ),
                ),
            ),
            expected_statement_events=(
                "statement_started",
                "statement_submitted",
                "statement_failed",
            ),
            expected_progress_fail_lines=1,
            expected_executed_retention_days=("90", "1"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fallback_fails_when_executing_through_cursor_then_final_failure_published_once(
    test_case: SnowflakeStateTableFallbackLifecycleTestCase,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    cursor: FakeSnowflakeRejectingRawCursor = FakeSnowflakeRejectingRawCursor(
        rejections=test_case.rejections
    )
    sql: str = adapter.render_create_fingerprint_table_sql(database="analytics", schema="marts")

    capture: StatementProgressCapture = execute_with_statement_progress(
        adapter=adapter, raw_cursor=cursor, sql=sql
    )

    assert isinstance(capture.error, FakeSnowflakeProgrammingError)
    assert capture.error.errno == 3001
    assert capture.statement_events == test_case.expected_statement_events
    assert len(capture.statement_fail_lines) == test_case.expected_progress_fail_lines
    assert (
        tuple(statement.rsplit(" = ", 1)[1] for statement in cursor.executed_sql)
        == test_case.expected_executed_retention_days
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeStateTableSchemaFallbackTestCase(
            description="transient schema falls back alone",
            rejecting_schema="scratch",
            accepting_schema="marts",
            expected_executed=(
                ("analytics.scratch._sqlbuild_fingerprints", "90"),
                ("analytics.scratch._sqlbuild_fingerprints", "1"),
                ("analytics.marts._sqlbuild_fingerprints", "90"),
                ("analytics.scratch._sqlbuild_node_results", "1"),
                ("analytics.marts._sqlbuild_node_results", "90"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_one_schema_rejecting_90_days_when_creating_state_tables_then_other_schema_keeps_90(
    test_case: SnowflakeStateTableSchemaFallbackTestCase,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    connection: FakeSnowflakeRejectingConnection = FakeSnowflakeRejectingConnection(
        rejected_fragments=(f".{test_case.rejecting_schema}.", _RETENTION_90),
        error=FakeSnowflakeProgrammingError(_STANDARD_EDITION_REJECTION, errno=1008),
    )
    schemas: tuple[str, str] = (test_case.rejecting_schema, test_case.accepting_schema)

    schema: str
    for schema in schemas:
        _ = adapter.execute(
            connection=cast(Any, connection),
            sql=adapter.render_create_fingerprint_table_sql(database="analytics", schema=schema),
        )
    for schema in schemas:
        _ = adapter.execute(
            connection=cast(Any, connection),
            sql=adapter.render_create_node_result_table_sql(database="analytics", schema=schema),
        )

    assert (
        tuple(
            (
                sql.removeprefix("CREATE TABLE IF NOT EXISTS ").split(" (", 1)[0],
                sql.rsplit(" = ", 1)[1],
            )
            for sql in connection.executed_sql
        )
        == test_case.expected_executed
    )
