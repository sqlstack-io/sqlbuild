"""Integration tests for microbatch incremental execution lifecycle."""

from __future__ import annotations

import dataclasses
from datetime import UTC, datetime
from functools import partial
from pathlib import Path
from typing import Any, cast

import pytest

from sqlbuild._native import (
    MICROBATCH_END_SENTINEL,
    MICROBATCH_START_SENTINEL,
    DiscoveredHookFunction,
    FutureCursorAction,
    FutureCursorsConfig,
    MicrobatchLimitAction,
    OnSchemaChange,
    PythonHookEntry,
    SqlHookEntry,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.auditing.types import AuditRunScope
from sqlbuild.executor.run._helpers.materializations import microbatch as microbatch_module
from sqlbuild.executor.run.models import BatchWindow, ModelExecutionResult
from sqlbuild.executor.run.types import ExecutionPhase
from tests.integration.src.sqlbuild.executor.run.microbatch._test_types import (
    MicrobatchFailureTestCase,
    MicrobatchSuccessTestCase,
)
from tests.integration.src.sqlbuild.executor.run.microbatch.helpers import (
    fail_microbatch_hook,
    insert_microbatch_hook_log,
    messages_with_prefix,
    reconcile_microbatch_batches,
    run_failure_test,
    run_skipped_test,
    run_success_test,
    skip_microbatch_hook,
    verify_failure_state,
    verify_success_state,
)
from tests.unit.src.sqlbuild.cli.output._helpers.helpers import formatted_model_asset

_TS_SOURCE_SQL: str = (
    "CREATE TABLE main.raw_events (  id INTEGER, event_time TIMESTAMP, payload VARCHAR)"
)
_TS_SOURCE_DATA: str = (
    "INSERT INTO main.raw_events VALUES "
    "(1, '2026-01-01 00:30:00', 'a'), "
    "(2, '2026-01-01 01:30:00', 'b'), "
    "(3, '2026-01-01 02:30:00', 'c')"
)

_TS_MODEL_SQL: str = (
    "SELECT id, event_time, payload FROM main.raw_events "
    f"WHERE event_time >= '{MICROBATCH_START_SENTINEL}' "
    f"AND event_time < '{MICROBATCH_END_SENTINEL}'"
)

_INT_SOURCE_SQL: str = (
    "CREATE TABLE main.raw_events (  id INTEGER, batch_id INTEGER, payload VARCHAR)"
)
_INT_SOURCE_DATA: str = (
    "INSERT INTO main.raw_events VALUES (1, 10, 'a'), (2, 30, 'b'), (3, 70, 'c'), (4, 90, 'd')"
)

_INT_MODEL_SQL: str = (
    "SELECT id, batch_id, payload FROM main.raw_events "
    f"WHERE batch_id >= {MICROBATCH_START_SENTINEL} "
    f"AND batch_id < {MICROBATCH_END_SENTINEL}"
)


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchSuccessTestCase(
            description="runtime warn limit executes complete microbatch range",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=3,
            expected_batch_count=3,
            expected_warning_count=1,
            microbatch_limit=2,
            microbatch_limit_action=MicrobatchLimitAction.WARN,
            expected_microbatch_limit_count=3,
            expected_microbatch_limit_warning=True,
            expected_query_results=(("SELECT COUNT(*) FROM main.orders", ((3,),)),),
        ),
        MicrobatchSuccessTestCase(
            description="model-backed cursor input resolves runtime range for microbatch",
            setup_sql=(
                "CREATE TABLE main.fact_orders (id INTEGER, ordered_at TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.fact_orders VALUES "
                "(1, '2026-01-01 00:30:00', 'a'), "
                "(2, '2026-01-01 01:30:00', 'b')",
                "CREATE TABLE main.orders (id INTEGER, activity_hour TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=(
                "SELECT id, DATE_TRUNC('hour', ordered_at) AS activity_hour, payload "
                "FROM main.fact_orders "
                f"WHERE ordered_at >= '{MICROBATCH_START_SENTINEL}' "
                f"AND ordered_at < '{MICROBATCH_END_SENTINEL}'"
            ),
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="activity_hour",
            cursor_type="timestamp",
            cursor_grain="hour",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T02:00:00",
            use_plan_microbatch_range=False,
            cursor_input_relations=(("main.fact_orders", "ordered_at"),),
            cursor_inputs_model_backed=True,
            unique_key=("id",),
            expected_row_count=2,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((1, "a"), (2, "b")),
                ),
            ),
        ),
        MicrobatchSuccessTestCase(
            description="complete overrides skip runtime-produced watermark discovery",
            setup_sql=(
                "CREATE TABLE main.raw_events (id INTEGER, batch_id INTEGER)",
                "INSERT INTO main.raw_events VALUES (1, 10), (2, 20), (3, 21)",
                "CREATE TABLE main.orders (id INTEGER, batch_id INTEGER)",
            ),
            model_sql=(
                "SELECT id, batch_id FROM main.raw_events "
                f"WHERE batch_id >= {MICROBATCH_START_SENTINEL} "
                f"AND batch_id < {MICROBATCH_END_SENTINEL}"
            ),
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="batch_id",
            cursor_type="integer",
            batch_size="5",
            microbatch_start="10",
            microbatch_end="21",
            cursor_input_relations=(("main.missing_runtime_watermark", "batch_id"),),
            cursor_inputs_model_backed=True,
            unique_key=("id",),
            start_cursor_override="10",
            end_cursor_override="20",
            expected_row_count=2,
            expected_batch_count=3,
            expected_query_results=(
                (
                    "SELECT id, batch_id FROM main.orders ORDER BY id",
                    ((1, 10), (2, 20)),
                ),
            ),
        ),
        MicrobatchSuccessTestCase(
            description="timestamp microbatch appends 3 hourly batches",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=3,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((1, "a"), (2, "b"), (3, "c")),
                ),
            ),
            expected_executed_statement_fragments=(
                "CREATE OR REPLACE TABLE main.orders__delta AS SELECT id, event_time, payload",
                "event_time >= '2026-01-01T00:00:00'",
                "event_time < '2026-01-01T01:00:00'",
                'INSERT INTO main.orders ("id", "event_time", "payload") '
                "SELECT id, event_time, payload FROM main.orders__delta",
                "DROP TABLE IF EXISTS main.orders__delta",
            ),
        ),
        MicrobatchSuccessTestCase(
            description="append aligns identical column sets with different physical order",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (payload VARCHAR, id INTEGER, event_time TIMESTAMP)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=3,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((1, "a"), (2, "b"), (3, "c")),
                ),
            ),
        ),
        MicrobatchSuccessTestCase(
            description="integer microbatch appends 2 batches of size 50",
            setup_sql=(
                _INT_SOURCE_SQL,
                _INT_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, batch_id INTEGER, payload VARCHAR)",
            ),
            model_sql=_INT_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="batch_id",
            cursor_type="integer",
            batch_size="50",
            microbatch_start="0",
            microbatch_end="100",
            expected_row_count=4,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((1, "a"), (2, "b"), (3, "c"), (4, "d")),
                ),
            ),
        ),
        MicrobatchSuccessTestCase(
            description="full-refresh microbatch rebuilds aside and swaps after all batches",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.orders VALUES (99, '2025-01-01 00:00:00', 'old')",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            is_full_refresh=True,
            cursor_input_relations=(("main.raw_events", "event_time"),),
            expected_row_count=3,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((1, "a"), (2, "b"), (3, "c")),
                ),
                (
                    "SELECT id, payload FROM main.__sqb_prev__orders ORDER BY id",
                    ((99, "old"),),
                ),
            ),
        ),
        MicrobatchSuccessTestCase(
            description="equal-bound full refresh executes and reports one limited batch",
            setup_sql=(
                _TS_SOURCE_SQL,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="1970-01-01T00:00:00",
            microbatch_end="1970-01-01T00:00:00",
            is_full_refresh=True,
            cursor_input_relations=(("main.raw_events", "event_time"),),
            expected_row_count=0,
            expected_batch_count=1,
            microbatch_limit=1,
            microbatch_limit_action=MicrobatchLimitAction.ERROR,
        ),
        MicrobatchSuccessTestCase(
            description="full-refresh discovers range from input when output groups away cursor",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.order_activity (event_hour TIMESTAMP, event_count BIGINT)",
            ),
            model_sql=(
                "SELECT DATE_TRUNC('hour', event_time) AS event_hour, COUNT(*) AS event_count "
                "FROM main.raw_events "
                f"WHERE event_time >= '{MICROBATCH_START_SENTINEL}' "
                f"AND event_time < '{MICROBATCH_END_SENTINEL}' "
                "GROUP BY DATE_TRUNC('hour', event_time)"
            ),
            target_schema="main",
            target_name="order_activity",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            is_full_refresh=True,
            cursor_input_relations=(("main.raw_events", "event_time"),),
            expected_row_count=3,
            expected_query_results=(
                (
                    "SELECT CAST(event_hour AS VARCHAR), event_count "
                    "FROM main.order_activity ORDER BY event_hour",
                    (
                        ("2026-01-01 00:00:00", 1),
                        ("2026-01-01 01:00:00", 1),
                        ("2026-01-01 02:00:00", 1),
                    ),
                ),
            ),
        ),
        MicrobatchSuccessTestCase(
            description="delta_and_final audit runs per batch plus once final",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            audit_sql='SELECT id FROM __ref("orders") WHERE id < 0',
            audit_severity="warn",
            audit_run_scope=AuditRunScope.DELTA_AND_FINAL,
            expected_row_count=3,
            expected_audit_count=4,
        ),
        MicrobatchSuccessTestCase(
            description="SQL and Python hooks run once around entire batch loop",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "CREATE TABLE main.hook_log (phase VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            pre_hook=[
                SqlHookEntry(statement="INSERT INTO main.hook_log VALUES ('sql_pre')"),
                PythonHookEntry(name="insert_hook_log", kwargs={"phase": "python_pre"}),
            ],
            post_hook=[
                PythonHookEntry(name="insert_hook_log", kwargs={"phase": "python_post"}),
                SqlHookEntry(statement="INSERT INTO main.hook_log VALUES ('sql_post')"),
            ],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/microbatch.py"),
                    name="insert_hook_log",
                    function=insert_microbatch_hook_log,
                ),
            ),
            expected_row_count=3,
            expected_query_results=(
                (
                    "SELECT phase FROM main.hook_log ORDER BY rowid",
                    (("sql_pre",), ("python_pre",), ("python_post",), ("sql_post",)),
                ),
            ),
        ),
        MicrobatchSuccessTestCase(
            description="delete_insert microbatch replaces matching rows per batch",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.orders VALUES (1, '2026-01-01 00:30:00', 'old_a'), "
                "(2, '2026-01-01 01:30:00', 'old_b')",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            unique_key=("id",),
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=3,
            expected_warning_count=0,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((1, "a"), (2, "b"), (3, "c")),
                ),
            ),
        ),
        MicrobatchSuccessTestCase(
            description="cursor-based delete_insert uses cursor range not unique key",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.orders VALUES "
                "(1, '2026-01-01 00:30:00', 'old_a'), "
                "(10, '2026-01-01 00:45:00', 'stays'), "
                "(2, '2026-01-01 01:30:00', 'old_b')",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=3,
            expected_warning_count=0,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((1, "a"), (2, "b"), (3, "c")),
                ),
            ),
        ),
        MicrobatchSuccessTestCase(
            description="append_new_columns schema change applied on first batch",
            setup_sql=(
                "CREATE TABLE main.raw_events ("
                "  id INTEGER, event_time TIMESTAMP, payload VARCHAR, extra INTEGER"
                ")",
                "INSERT INTO main.raw_events VALUES "
                "(1, '2026-01-01 00:30:00', 'a', 10), "
                "(2, '2026-01-01 01:30:00', 'b', 20)",
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=(
                "SELECT id, event_time, payload, extra FROM main.raw_events "
                f"WHERE event_time >= '{MICROBATCH_START_SENTINEL}' "
                f"AND event_time < '{MICROBATCH_END_SENTINEL}'"
            ),
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.APPEND_NEW_COLUMNS,
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T02:00:00",
            expected_row_count=2,
            expected_column_names=("id", "event_time", "payload", "extra"),
            expected_query_results=(
                (
                    "SELECT id, extra FROM main.orders ORDER BY id",
                    ((1, 10), (2, 20)),
                ),
            ),
        ),
        MicrobatchSuccessTestCase(
            description="empty range produces zero batches and succeeds with warning",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T00:00:00",
            expected_row_count=0,
            expected_warning_count=1,
            expected_delta_cleaned=False,
        ),
        MicrobatchSuccessTestCase(
            description="merge microbatch upserts across batches",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.orders VALUES (1, '2026-01-01 00:30:00', 'old_a')",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="merge",
            unique_key=("id",),
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=3,
            expected_warning_count=0,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((1, "a"), (2, "b"), (3, "c")),
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_microbatch_model_when_executing_then_succeeds(
    test_case: MicrobatchSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_success_test(
        test_case=test_case, adapter=adapter, connection=connection
    )
    assert result.status == test_case.expected_status
    verify_success_state(result=result, test_case=test_case, connection=connection)


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchSuccessTestCase(
            description="runtime watermark cap executes latest batches only",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=2,
            expected_batch_count=2,
            expected_warning_count=1,
            expected_query_results=(("SELECT id FROM main.orders ORDER BY id", ((2,), (3,))),),
            microbatch_limit=2,
            microbatch_limit_action=MicrobatchLimitAction.CAP_FROM_END,
            expected_microbatch_limit_count=3,
            expected_microbatch_limit_warning=True,
        ),
        MicrobatchSuccessTestCase(
            description="integer cap from start executes earliest batches only",
            setup_sql=(
                _INT_SOURCE_SQL,
                _INT_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, batch_id INTEGER, payload VARCHAR)",
            ),
            model_sql=_INT_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="batch_id",
            cursor_type="integer",
            batch_size="25",
            microbatch_start="0",
            microbatch_end="100",
            expected_row_count=2,
            expected_batch_count=2,
            expected_warning_count=1,
            expected_query_results=(("SELECT id FROM main.orders ORDER BY id", ((1,), (2,))),),
            microbatch_limit=2,
            microbatch_limit_action=MicrobatchLimitAction.CAP_FROM_START,
            expected_microbatch_limit_count=4,
            expected_microbatch_limit_warning=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deferred_watermark_range_when_capping_then_selected_batches_execute(
    test_case: MicrobatchSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_success_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert result.microbatch_limit_count == test_case.expected_microbatch_limit_count
    assert result.microbatch_limit_warning is not None
    verify_success_state(result=result, test_case=test_case, connection=connection)


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchFailureTestCase(
            description="warn limit survives failed pre-hook serialization",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            pre_hook=[SqlHookEntry(statement="SELECT * FROM missing_warn_hook_table")],
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
            expected_error_fragment="missing_warn_hook_table",
            expected_row_count=0,
            microbatch_limit=2,
            microbatch_limit_action=MicrobatchLimitAction.WARN,
            expected_microbatch_limit_count=3,
            expected_microbatch_limit_warning=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_warn_limit_when_pre_hook_fails_then_execution_metadata_is_preserved(
    test_case: MicrobatchFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_failure_test(
        test_case=test_case, adapter=adapter, connection=connection
    )
    asset: dict[str, object] = formatted_model_asset(result)
    microbatch: dict[str, object] = cast(dict[str, object], asset["microbatch"])

    assert result.microbatch_limit_count == test_case.expected_microbatch_limit_count
    assert (
        result.microbatch_limit_warning is not None
    ) is test_case.expected_microbatch_limit_warning
    assert result.warning_messages == (result.microbatch_limit_warning,)
    assert microbatch["limit"] == 2
    assert microbatch["count"] == 3
    assert microbatch["action"] == "warn"
    assert microbatch["warning"] == result.microbatch_limit_warning


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchSuccessTestCase(
            description="warn limit survives skipped pre-hook serialization",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=0,
            pre_hook=[PythonHookEntry(name="skip_hook", kwargs={"reason": "not today"})],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/microbatch.py"),
                    name="skip_hook",
                    function=skip_microbatch_hook,
                ),
            ),
            microbatch_limit=2,
            microbatch_limit_action=MicrobatchLimitAction.WARN,
            expected_microbatch_limit_count=3,
            expected_microbatch_limit_warning=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_warn_limit_when_pre_hook_skips_then_execution_metadata_is_preserved(
    test_case: MicrobatchSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_skipped_test(
        test_case=test_case, adapter=adapter, connection=connection
    )
    asset: dict[str, object] = formatted_model_asset(result)
    microbatch: dict[str, object] = cast(dict[str, object], asset["microbatch"])

    assert result.microbatch_limit_count == test_case.expected_microbatch_limit_count
    assert (
        result.microbatch_limit_warning is not None
    ) is test_case.expected_microbatch_limit_warning
    assert result.warning_messages == (result.microbatch_limit_warning,)
    assert microbatch["limit"] == 2
    assert microbatch["count"] == 3
    assert microbatch["action"] == "warn"
    assert microbatch["warning"] == result.microbatch_limit_warning


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchFailureTestCase(
            description="reconciliation expansion is rejected before pre-hook",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "CREATE TABLE main.hook_log (phase VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="3h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            pre_hook=[SqlHookEntry(statement="INSERT INTO main.hook_log VALUES ('pre')")],
            expected_failed_phase=ExecutionPhase.STAGING,
            expected_error_fragment="planned 3 batches",
            expected_row_count=0,
            expected_query_results=(("SELECT * FROM main.hook_log", ()),),
            microbatch_limit=2,
            microbatch_limit_action=MicrobatchLimitAction.ERROR,
            expected_microbatch_limit_count=3,
            expected_microbatch_limit_warning=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_reconciliation_expands_work_when_enforcing_limit_then_final_set_is_rejected(
    test_case: MicrobatchFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    reconciled_batches: tuple[BatchWindow, ...] = (
        BatchWindow(start="2026-01-01T00:00:00", end="2026-01-01T01:00:00", index=0),
        BatchWindow(start="2026-01-01T01:00:00", end="2026-01-01T02:00:00", index=1),
        BatchWindow(start="2026-01-01T02:00:00", end="2026-01-01T03:00:00", index=2),
    )
    monkeypatch.setattr(
        microbatch_module,
        "_run_microbatch_reconciliation",
        partial(reconcile_microbatch_batches, reconciled_batches=reconciled_batches),
    )

    result: ModelExecutionResult = run_failure_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert result.microbatch_limit_count == test_case.expected_microbatch_limit_count
    verify_failure_state(result=result, test_case=test_case, connection=connection)


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchSuccessTestCase(
            description="reconciliation reduction permits final batch set",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "CREATE TABLE main.hook_log (phase VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            pre_hook=[SqlHookEntry(statement="INSERT INTO main.hook_log VALUES ('pre')")],
            expected_row_count=1,
            expected_batch_count=1,
            expected_query_results=(("SELECT phase FROM main.hook_log", (("pre",),)),),
            microbatch_limit=2,
            microbatch_limit_action=MicrobatchLimitAction.ERROR,
            expected_microbatch_limit_count=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_reconciliation_reduces_work_when_enforcing_limit_then_final_set_executes(
    test_case: MicrobatchSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    reconciled_batches: tuple[BatchWindow, ...] = (
        BatchWindow(start="2026-01-01T00:00:00", end="2026-01-01T01:00:00", index=0),
    )
    monkeypatch.setattr(
        microbatch_module,
        "_run_microbatch_reconciliation",
        partial(reconcile_microbatch_batches, reconciled_batches=reconciled_batches),
    )

    result: ModelExecutionResult = run_success_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert result.microbatch_limit_count == test_case.expected_microbatch_limit_count
    verify_success_state(result=result, test_case=test_case, connection=connection)


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchFailureTestCase(
            description="runtime error limit blocks pre-hook and all batches",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "CREATE TABLE main.hook_log (phase VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            pre_hook=[SqlHookEntry(statement="INSERT INTO main.hook_log VALUES ('pre')")],
            expected_failed_phase=ExecutionPhase.STAGING,
            expected_error_fragment="MICROBATCH LIMIT EXCEEDED",
            expected_row_count=0,
            microbatch_limit=2,
            microbatch_limit_action=MicrobatchLimitAction.ERROR,
            expected_query_results=(
                ("SELECT COUNT(*) FROM main.hook_log", ((0,),)),
                ("SELECT COUNT(*) FROM main.orders", ((0,),)),
            ),
        ),
        MicrobatchFailureTestCase(
            description="runtime cap evidence survives pre-hook failure",
            setup_sql=(
                "CREATE TABLE main.future_input (id INTEGER, event_time TIMESTAMP)",
                "INSERT INTO main.future_input VALUES (1, '2500-01-01')",
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP)",
            ),
            model_sql=(
                "SELECT * FROM main.future_input "
                f"WHERE event_time >= '{MICROBATCH_START_SENTINEL}' "
                f"AND event_time < '{MICROBATCH_END_SENTINEL}'"
            ),
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="event_time",
            cursor_type="timestamp",
            cursor_grain="second",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T01:00:00",
            use_plan_microbatch_range=False,
            cursor_input_relations=(("main.future_input", "event_time"),),
            cursor_inputs_model_backed=True,
            pre_hook=[SqlHookEntry(statement="SELECT * FROM missing_pre_hook_table")],
            future_cursor_config=FutureCursorsConfig("2d", FutureCursorAction.CAP),
            invocation_time=datetime(2026, 9, 1, 12, 0, tzinfo=UTC),
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
            expected_error_fragment="missing_pre_hook_table",
            expected_row_count=0,
            expected_has_future_cursor_safety=True,
        ),
        MicrobatchFailureTestCase(
            description="runtime error blocks microbatch pre-hook side effects",
            setup_sql=(
                "CREATE TABLE main.future_input (id INTEGER, event_time TIMESTAMP)",
                "INSERT INTO main.future_input VALUES (1, '2500-01-01')",
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP)",
                "CREATE TABLE main.hook_log (phase VARCHAR)",
            ),
            model_sql=(
                "SELECT * FROM main.future_input "
                f"WHERE event_time >= '{MICROBATCH_START_SENTINEL}' "
                f"AND event_time < '{MICROBATCH_END_SENTINEL}'"
            ),
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="event_time",
            cursor_type="timestamp",
            cursor_grain="second",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T01:00:00",
            use_plan_microbatch_range=False,
            cursor_input_relations=(("main.future_input", "event_time"),),
            cursor_inputs_model_backed=True,
            pre_hook=[SqlHookEntry(statement="INSERT INTO main.hook_log VALUES ('pre')")],
            future_cursor_config=FutureCursorsConfig("2d", FutureCursorAction.ERROR),
            invocation_time=datetime(2026, 9, 1, 12, 0, tzinfo=UTC),
            expected_failed_phase=ExecutionPhase.STAGING,
            expected_error_fragment="future cursor safety limit exceeded",
            expected_row_count=0,
            expected_query_results=(("SELECT * FROM main.hook_log", ()),),
        ),
        MicrobatchFailureTestCase(
            description="pre_hook failure blocks all batches",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            pre_hook=[SqlHookEntry(statement="SELECT * FROM nonexistent_table_for_hook")],
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
            expected_row_count=0,
        ),
        MicrobatchFailureTestCase(
            description="python pre_hook failure blocks all batches",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            pre_hook=[
                PythonHookEntry(name="fail_hook", kwargs={"message": "microbatch pre failed"})
            ],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/microbatch.py"),
                    name="fail_hook",
                    function=fail_microbatch_hook,
                ),
            ),
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
            expected_error_fragment='pre_hooks[0] python("fail_hook") failed: microbatch pre failed',
            expected_row_count=0,
        ),
        MicrobatchFailureTestCase(
            description="delta audit error on batch stops remaining batches",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            audit_sql='SELECT id FROM __ref("orders") WHERE id > 0',
            audit_severity="error",
            audit_run_scope=AuditRunScope.DELTA_AND_FINAL,
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_error_fragment="batch 0",
            expected_audit_count=1,
            expected_row_count=0,
        ),
        MicrobatchFailureTestCase(
            description="delta audit failure on later batch preserves earlier completed batches",
            setup_sql=(
                "CREATE TABLE main.raw_events (  id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.raw_events VALUES "
                "(1, '2026-01-01 00:30:00', 'a'), "
                "(2, '2026-01-01 01:30:00', 'b')",
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T02:00:00",
            audit_sql='SELECT id FROM __ref("orders") WHERE id = 2',
            audit_severity="error",
            audit_run_scope=AuditRunScope.DELTA_AND_FINAL,
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_error_fragment="batch 1",
            expected_audit_count=2,
            expected_row_count=1,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((1, "a"),),
                ),
            ),
            expected_delta_retained=True,
        ),
        MicrobatchFailureTestCase(
            description="post_hook failure after multiple successful batches",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            post_hook=[SqlHookEntry(statement="SELECT * FROM nonexistent_table_for_hook")],
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_row_count=3,
        ),
        MicrobatchFailureTestCase(
            description="python post_hook failure after multiple successful batches",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            post_hook=[
                PythonHookEntry(name="fail_hook", kwargs={"message": "microbatch post failed"})
            ],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/microbatch.py"),
                    name="fail_hook",
                    function=fail_microbatch_hook,
                ),
            ),
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_error_fragment='post_hooks[0] python("fail_hook") failed: microbatch post failed',
            expected_row_count=3,
        ),
        MicrobatchFailureTestCase(
            description="full-refresh microbatch failure keeps live target and rebuild data",
            setup_sql=(
                "CREATE TABLE main.raw_events (  id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.raw_events VALUES "
                "(1, '2026-01-01 00:30:00', 'a'), "
                "(2, '2026-01-01 01:30:00', 'b')",
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.orders VALUES (99, '2025-01-01 00:00:00', 'old')",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T02:00:00",
            is_full_refresh=True,
            cursor_input_relations=(("main.raw_events", "event_time"),),
            audit_sql='SELECT id FROM __ref("orders") WHERE id = 2',
            audit_severity="error",
            audit_run_scope=AuditRunScope.DELTA_AND_FINAL,
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_error_fragment="batch 1",
            expected_audit_count=2,
            expected_row_count=1,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((99, "old"),),
                ),
                (
                    "SELECT id, payload FROM main.__sqb_rebuild__orders ORDER BY id",
                    ((1, "a"),),
                ),
            ),
            expected_delta_retained=True,
        ),
        MicrobatchFailureTestCase(
            description="full-refresh delete_insert fails clearly when output omits cursor",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.order_activity (event_hour TIMESTAMP, event_count BIGINT)",
            ),
            model_sql=(
                "SELECT DATE_TRUNC('hour', event_time) AS event_hour, COUNT(*) AS event_count "
                "FROM main.raw_events "
                f"WHERE event_time >= '{MICROBATCH_START_SENTINEL}' "
                f"AND event_time < '{MICROBATCH_END_SENTINEL}' "
                "GROUP BY DATE_TRUNC('hour', event_time)"
            ),
            target_schema="main",
            target_name="order_activity",
            incremental_strategy="delete_insert",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            is_full_refresh=True,
            cursor_input_relations=(("main.raw_events", "event_time"),),
            expected_failed_phase=ExecutionPhase.DML,
            expected_error_fragment="microbatch cursor column 'event_time' is not produced",
        ),
        MicrobatchFailureTestCase(
            description="schema change fail rejects before any batch DML in microbatch",
            setup_sql=(
                "CREATE TABLE main.raw_events ("
                "  id INTEGER, event_time TIMESTAMP, payload VARCHAR, extra INTEGER"
                ")",
                "INSERT INTO main.raw_events VALUES (1, '2026-01-01 00:30:00', 'a', 10)",
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.orders VALUES (99, '2025-01-01 00:00:00', 'old')",
            ),
            model_sql=(
                "SELECT id, event_time, payload, extra FROM main.raw_events "
                f"WHERE event_time >= '{MICROBATCH_START_SENTINEL}' "
                f"AND event_time < '{MICROBATCH_END_SENTINEL}'"
            ),
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.FAIL,
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T02:00:00",
            expected_failed_phase=ExecutionPhase.SCHEMA_CHANGE,
            expected_error_fragment="on_schema_change is set to fail",
            expected_row_count=1,
            expected_delta_retained=True,
        ),
        MicrobatchFailureTestCase(
            description="final audit error after all batches reports target already updated",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            audit_sql='SELECT id FROM __ref("orders") WHERE id > 0',
            audit_severity="error",
            audit_run_scope=AuditRunScope.FINAL,
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_error_fragment="final audit for 'orders' failed after target update",
            expected_audit_count=1,
            expected_row_count=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_microbatch_model_when_execution_fails_then_reports_correct_phase(
    test_case: MicrobatchFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_failure_test(
        test_case=test_case, adapter=adapter, connection=connection
    )
    assert result.failed_phase == test_case.expected_failed_phase
    verify_failure_state(result=result, test_case=test_case, connection=connection)


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchSuccessTestCase(
            description="progress callback fires per batch with timing",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=3,
            expected_batch_count=3,
        ),
        MicrobatchSuccessTestCase(
            description="batch count populated without progress callback",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=3,
            expected_batch_count=3,
        ),
        MicrobatchSuccessTestCase(
            description="planner-owned effective batch uses coarser input grain",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            cursor_grain="hour",
            cursor_input_relations=(("main.raw_events", "event_time"),),
            cursor_input_grain="day",
            batch_size="effective",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=3,
            expected_batch_count=1,
        ),
        MicrobatchSuccessTestCase(
            description="planner-owned fixed batch ignores coarser input grain",
            setup_sql=(
                _TS_SOURCE_SQL,
                _TS_SOURCE_DATA,
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=_TS_MODEL_SQL,
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            cursor_column="event_time",
            cursor_type="timestamp",
            cursor_grain="hour",
            cursor_input_relations=(("main.raw_events", "event_time"),),
            cursor_input_grain="day",
            batch_size="1h",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T03:00:00",
            expected_row_count=3,
            expected_batch_count=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_microbatch_model_when_executing_then_reports_batch_progress(
    test_case: MicrobatchSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    progress_messages: list[str] = []
    patched_case: MicrobatchSuccessTestCase = dataclasses.replace(
        test_case, on_progress=progress_messages.append
    )
    result: ModelExecutionResult = run_success_test(
        test_case=patched_case, adapter=adapter, connection=connection
    )

    assert test_case.expected_batch_count is not None
    assert result.batch_count == test_case.expected_batch_count
    assert len(progress_messages) == test_case.expected_batch_count * 2 + 1
    assert progress_messages[0].startswith("runtime plan resolved:")
    assert any("batch 1/" in message for message in progress_messages[1:])
    assert any(message.rstrip().endswith("s") for message in progress_messages)


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchSuccessTestCase(
            description="runtime-owned range is reported before model-backed batches start",
            setup_sql=(
                "CREATE TABLE main.fact_orders (id INTEGER, ordered_at TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.fact_orders VALUES "
                "(1, '2026-01-01 00:30:00', 'a'), "
                "(2, '2026-01-01 01:30:00', 'b')",
                "CREATE TABLE main.orders (id INTEGER, activity_hour TIMESTAMP, payload VARCHAR)",
            ),
            model_sql=(
                "SELECT id, DATE_TRUNC('hour', ordered_at) AS activity_hour, payload "
                "FROM main.fact_orders "
                f"WHERE ordered_at >= '{MICROBATCH_START_SENTINEL}' "
                f"AND ordered_at < '{MICROBATCH_END_SENTINEL}'"
            ),
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="activity_hour",
            cursor_type="timestamp",
            cursor_grain="hour",
            batch_size="effective",
            microbatch_start="2026-01-01T00:00:00",
            microbatch_end="2026-01-01T02:00:00",
            use_plan_microbatch_range=False,
            cursor_input_relations=(("main.fact_orders", "ordered_at"),),
            cursor_inputs_model_backed=True,
            unique_key=("id",),
            expected_row_count=2,
            expected_progress_message=(
                "resolved runtime range 2026-01-01T00:00:00 -> 2026-01-01T01:00:00 (2 batches x 1h)"
            ),
            expected_cursor_range_start="2026-01-01T00:00:00",
            expected_cursor_range_end="2026-01-01T02:00:00",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_runtime_owned_range_when_executing_then_reports_resolution_before_batches(
    test_case: MicrobatchSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    progress_messages: list[str] = []
    result: ModelExecutionResult = run_success_test(
        test_case=dataclasses.replace(test_case, on_progress=progress_messages.append),
        adapter=adapter,
        connection=connection,
    )

    assert test_case.expected_progress_message is not None
    runtime_messages: tuple[str, ...] = messages_with_prefix(
        messages=progress_messages, prefix="runtime plan resolved:"
    )
    assert len(runtime_messages) == 1
    assert "2 batches x 1h" in runtime_messages[0]
    assert result.cursor_range_start == test_case.expected_cursor_range_start
    assert result.cursor_range_end == test_case.expected_cursor_range_end
