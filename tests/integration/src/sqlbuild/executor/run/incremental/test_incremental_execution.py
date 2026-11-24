"""Integration tests for incremental model execution lifecycle."""

from __future__ import annotations

from datetime import UTC, datetime
from pathlib import Path
from typing import Any, ClassVar

import pytest

from sqlbuild._native import (
    MICROBATCH_END_SENTINEL,
    MICROBATCH_START_SENTINEL,
    DiscoveredHookFunction,
    FutureCursorAction,
    FutureCursorsConfig,
    OnSchemaChange,
    PythonHookEntry,
    SqlHookEntry,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.auditing.types import AuditRunScope
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.run.types import ExecutionPhase
from tests.integration.src.sqlbuild.executor.run.incremental._test_types import (
    IncrementalFailureTestCase,
    IncrementalSuccessTestCase,
)
from tests.integration.src.sqlbuild.executor.run.incremental.helpers import (
    fail_incremental_hook,
    insert_incremental_hook_log,
    run_failure_test,
    run_success_test,
    verify_failure_state,
    verify_success_state,
)


class ZeroCopyDuckDbAdapter(DuckDbAdapter):
    """DuckDB test adapter that exercises the cheap-reuse executor path."""

    adapter_name: ClassVar[str] = "duckdb_zero_copy_incremental_test"

    def supports_zero_copy_clone(self) -> bool:
        return True


@pytest.mark.parametrize(
    "test_case",
    [
        IncrementalSuccessTestCase(
            description="future cap resolves before pre-hook mutates upstream",
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
            cursor_input_relations=(("main.future_input", "event_time"),),
            cursor_inputs_model_backed=True,
            pre_hook=[
                SqlHookEntry(statement="DELETE FROM main.future_input"),
                SqlHookEntry(statement="INSERT INTO main.future_input VALUES (2, '2026-01-01')"),
            ],
            future_cursor_config=FutureCursorsConfig("2d", FutureCursorAction.CAP),
            invocation_time=datetime(2026, 9, 1, 12, 0, tzinfo=UTC),
            expected_row_count=0,
            expected_warning_count=1,
            expected_has_future_cursor_safety=True,
        ),
        IncrementalSuccessTestCase(
            description="model-backed cursor input resolves runtime bounds for normal incremental",
            setup_sql=(
                "CREATE TABLE main.fact_orders ("
                "order_id INTEGER, customer_id INTEGER, order_status VARCHAR, "
                "ordered_at TIMESTAMP, line_total_cents INTEGER)",
                "INSERT INTO main.fact_orders VALUES "
                "(1, 10, 'completed', '2026-01-01 00:30:00', 100), "
                "(2, 11, 'completed', '2026-01-01 01:30:00', 200)",
                "CREATE TABLE main.order_status_index ("
                "order_id INTEGER, customer_id INTEGER, order_status VARCHAR, "
                "ordered_at TIMESTAMP, line_total_cents INTEGER)",
            ),
            model_sql=(
                "SELECT order_id, customer_id, order_status, ordered_at, line_total_cents "
                "FROM main.fact_orders"
            ),
            target_schema="main",
            target_name="order_status_index",
            incremental_strategy="delete_insert",
            cursor_column="order_id",
            cursor_type="integer",
            cursor_input_relations=(("main.fact_orders", "order_id"),),
            cursor_inputs_model_backed=True,
            expected_row_count=2,
            expected_query_results=(
                (
                    "SELECT order_id, customer_id FROM main.order_status_index ORDER BY order_id",
                    ((1, 10), (2, 11)),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="complete overrides skip normal runtime-produced watermark discovery",
            setup_sql=(
                "CREATE TABLE main.raw_events (id INTEGER, amount INTEGER)",
                "INSERT INTO main.raw_events VALUES (5, 50), (10, 100), (20, 200), (21, 210)",
                "CREATE TABLE main.orders (id INTEGER, amount INTEGER)",
            ),
            model_sql=("SELECT id, amount FROM main.raw_events WHERE id >= 10 AND id < 21"),
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            cursor_column="id",
            cursor_type="integer",
            cursor_start="10",
            cursor_end="21",
            cursor_input_relations=(("main.missing_runtime_watermark", "id"),),
            cursor_inputs_model_backed=True,
            start_cursor_override="10",
            end_cursor_override="20",
            expected_row_count=2,
            expected_query_results=(
                (
                    "SELECT id, amount FROM main.orders ORDER BY id",
                    ((10, 100), (20, 200)),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="append strategy adds new rows to existing table",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            expected_row_count=2,
            expected_query_results=(
                (
                    "SELECT id, name FROM main.orders ORDER BY id",
                    ((1, "alice"), (2, "bob")),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="delete_insert replaces matching rows by key",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice'), (2, 'bob')",
            ),
            model_sql="SELECT 1 AS id, 'alice_updated' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            unique_key=("id",),
            expected_row_count=2,
            expected_query_results=(
                (
                    "SELECT id, name FROM main.orders ORDER BY id",
                    ((1, "alice_updated"), (2, "bob")),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="merge upserts matching rows and inserts new ones",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice'), (2, 'bob')",
            ),
            model_sql="SELECT 1 AS id, 'alice_updated' AS name UNION ALL SELECT 3, 'charlie'",
            target_schema="main",
            target_name="orders",
            incremental_strategy="merge",
            unique_key=("id",),
            expected_row_count=3,
            expected_query_results=(
                (
                    "SELECT id, name FROM main.orders ORDER BY id",
                    ((1, "alice_updated"), (2, "bob"), (3, "charlie")),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="merge exclusions preserve matched values and populate inserted rows",
            setup_sql=(
                "CREATE TABLE main.orders ("
                "region VARCHAR, id INTEGER, name VARCHAR, ingested_at TIMESTAMP)",
                "INSERT INTO main.orders VALUES "
                "('uk', 1, 'alice', '2026-01-01 00:00:00'), "
                "('uk', 2, 'bob', '2026-01-02 00:00:00')",
            ),
            model_sql=(
                "SELECT 'uk' AS region, 1 AS id, 'alice_updated' AS name, "
                "TIMESTAMP '2026-08-01 00:00:00' AS ingested_at "
                "UNION ALL SELECT 'uk', 3, 'charlie', TIMESTAMP '2026-08-03 00:00:00'"
            ),
            target_schema="main",
            target_name="orders",
            incremental_strategy="merge",
            unique_key=("region", "id"),
            merge_exclude_columns=("INGESTED_AT",),
            expected_row_count=3,
            expected_query_results=(
                (
                    "SELECT region, id, name, CAST(ingested_at AS VARCHAR) "
                    "FROM main.orders ORDER BY id",
                    (
                        ("uk", 1, "alice_updated", "2026-01-01 00:00:00"),
                        ("uk", 2, "bob", "2026-01-02 00:00:00"),
                        ("uk", 3, "charlie", "2026-08-03 00:00:00"),
                    ),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="append_new_columns adds column to target and appends rows",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name, 100 AS amount",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.APPEND_NEW_COLUMNS,
            expected_row_count=2,
            expected_column_names=("id", "name", "amount"),
            expected_query_results=(
                (
                    "SELECT id, name, amount FROM main.orders ORDER BY id",
                    ((1, "alice", None), (2, "bob", 100)),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="append_new_columns preserves undeclared existing target type",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 999 AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.APPEND_NEW_COLUMNS,
            expected_row_count=2,
            expected_warning_count=1,
            expected_query_results=(
                (
                    "SELECT id, name FROM main.orders ORDER BY id",
                    ((1, "alice"), (2, "999")),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="declared type is enforced before append schema comparison",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 999 AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.APPEND_NEW_COLUMNS,
            type_enforcement=True,
            declared_columns=(("name", "VARCHAR"),),
            expected_row_count=2,
            expected_query_results=(
                (
                    "SELECT id, name FROM main.orders ORDER BY id",
                    ((1, "alice"), (2, "999")),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="ignore preserves undeclared existing target type",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 999 AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.IGNORE,
            expected_row_count=2,
            expected_warning_count=1,
            expected_query_results=(
                (
                    "SELECT id, name FROM main.orders ORDER BY id",
                    ((1, "alice"), (2, "999")),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="append_new_columns with removed column uses intersection",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR, old_col VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice', 'old_value')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.APPEND_NEW_COLUMNS,
            expected_row_count=2,
            expected_warning_count=0,
            expected_query_results=(
                (
                    "SELECT id, name, old_col FROM main.orders ORDER BY id",
                    ((1, "alice", "old_value"), (2, "bob", None)),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="on_schema_change ignore does not alter target and warns",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name, 100 AS amount",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.IGNORE,
            expected_row_count=2,
            expected_warning_count=1,
            expected_column_names=("id", "name"),
            expected_query_results=(
                (
                    "SELECT id, name FROM main.orders ORDER BY id",
                    ((1, "alice"), (2, "bob")),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="sync_all_columns adds new and drops removed columns",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR, old_col VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice', 'old_value')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name, 200 AS amount",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.SYNC_ALL_COLUMNS,
            expected_row_count=2,
            expected_column_names=("id", "name", "amount"),
            expected_query_results=(
                (
                    "SELECT id, name, amount FROM main.orders ORDER BY id",
                    ((1, "alice", None), (2, "bob", 200)),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="delta_and_final audit runs in both phases",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            audit_sql='SELECT id FROM __ref("orders") WHERE id < 0',
            audit_severity="warn",
            audit_run_scope=AuditRunScope.DELTA_AND_FINAL,
            expected_row_count=2,
            expected_audit_count=2,
        ),
        IncrementalSuccessTestCase(
            description="final only audit runs once after DML",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            audit_sql='SELECT id FROM __ref("orders") WHERE id < 0',
            audit_severity="warn",
            audit_run_scope=AuditRunScope.FINAL,
            expected_row_count=2,
            expected_audit_count=1,
        ),
        IncrementalSuccessTestCase(
            description="pre and post hooks execute around incremental lifecycle",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
                "CREATE TABLE main.hook_log (phase VARCHAR)",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            pre_hook=[SqlHookEntry(statement="INSERT INTO main.hook_log VALUES ('pre')")],
            post_hook=[SqlHookEntry(statement="INSERT INTO main.hook_log VALUES ('post')")],
            expected_row_count=2,
            expected_query_results=(
                (
                    "SELECT phase FROM main.hook_log ORDER BY phase",
                    (("post",), ("pre",)),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="python pre and post hooks execute around incremental lifecycle",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
                "CREATE TABLE main.hook_log (phase VARCHAR)",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            pre_hook=[PythonHookEntry(name="insert_hook_log", kwargs={"phase": "pre"})],
            post_hook=[PythonHookEntry(name="insert_hook_log", kwargs={"phase": "post"})],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/incremental.py"),
                    name="insert_hook_log",
                    function=insert_incremental_hook_log,
                ),
            ),
            expected_row_count=2,
            expected_query_results=(
                (
                    "SELECT phase FROM main.hook_log ORDER BY phase",
                    (("post",), ("pre",)),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="merge with removed column preserves target-only values quietly",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR, old_col VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice', 'preserved'), (2, 'bob', 'also_kept')",
            ),
            model_sql="SELECT 1 AS id, 'alice_updated' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="merge",
            unique_key=("id",),
            on_schema_change=OnSchemaChange.APPEND_NEW_COLUMNS,
            expected_row_count=2,
            expected_warning_count=0,
            expected_query_results=(
                (
                    "SELECT id, name, old_col FROM main.orders ORDER BY id",
                    ((1, "alice_updated", "preserved"), (2, "bob", "also_kept")),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="delete_insert with composite key replaces correctly",
            setup_sql=(
                "CREATE TABLE main.orders (region VARCHAR, id INTEGER, amount INTEGER)",
                "INSERT INTO main.orders VALUES ('us', 1, 100), ('eu', 1, 200), ('us', 2, 300)",
            ),
            model_sql="SELECT 'us' AS region, 1 AS id, 999 AS amount",
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            unique_key=("region", "id"),
            expected_row_count=3,
            expected_query_results=(
                (
                    "SELECT region, id, amount FROM main.orders ORDER BY region, id",
                    (("eu", 1, 200), ("us", 1, 999), ("us", 2, 300)),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="sync_all_columns alters column type when adapter supports it",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, val VARCHAR)",
                "INSERT INTO main.orders VALUES (1, '100')",
            ),
            model_sql="SELECT 2 AS id, 200 AS val",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.SYNC_ALL_COLUMNS,
            expected_row_count=2,
            expected_query_results=(
                (
                    "SELECT id, val FROM main.orders ORDER BY id",
                    ((1, 100), (2, 200)),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="merge with ignore preserves target schema and uses intersection columns",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR, old_col VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice', 'kept'), (2, 'bob', 'also_kept')",
            ),
            model_sql="SELECT 1 AS id, 'alice_updated' AS name, 999 AS new_col",
            target_schema="main",
            target_name="orders",
            incremental_strategy="merge",
            unique_key=("id",),
            on_schema_change=OnSchemaChange.IGNORE,
            expected_row_count=2,
            expected_warning_count=2,
            expected_column_names=("id", "name", "old_col"),
            expected_query_results=(
                (
                    "SELECT id, name, old_col FROM main.orders ORDER BY id",
                    ((1, "alice_updated", "kept"), (2, "bob", "also_kept")),
                ),
            ),
        ),
        IncrementalSuccessTestCase(
            description="missing target schema warns on incremental when query tracking is enabled",
            setup_sql=(
                "CREATE TABLE orders (id INTEGER, name VARCHAR)",
                "INSERT INTO orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema=None,
            target_name="orders",
            incremental_strategy="append",
            expected_row_count=2,
            expected_warning_count=1,
        ),
        IncrementalSuccessTestCase(
            description="cursor delete_insert uses cursor range over unique_key",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, payload VARCHAR)",
                "INSERT INTO main.orders VALUES "
                "(1, '2026-01-01 00:30:00', 'a'), "
                "(2, '2026-01-01 00:45:00', 'b'), "
                "(3, '2026-01-01 01:30:00', 'c')",
            ),
            model_sql=(
                "SELECT 1 AS id, '2026-01-01 00:30:00'::TIMESTAMP AS event_time, 'new_a' AS payload"
            ),
            target_schema="main",
            target_name="orders",
            incremental_strategy="delete_insert",
            unique_key=("id",),
            cursor_column="event_time",
            cursor_start="2026-01-01T00:00:00",
            cursor_end="2026-01-01T01:00:00",
            expected_row_count=2,
            expected_query_results=(
                (
                    "SELECT id, payload FROM main.orders ORDER BY id",
                    ((1, "new_a"), (3, "c")),
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_table_when_running_incremental_then_succeeds(
    test_case: IncrementalSuccessTestCase,
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
        IncrementalFailureTestCase(
            description="future error blocks pre-hook side effects",
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
            cursor_input_relations=(("main.future_input", "event_time"),),
            cursor_inputs_model_backed=True,
            pre_hook=[SqlHookEntry(statement="INSERT INTO main.hook_log VALUES ('pre')")],
            future_cursor_config=FutureCursorsConfig("2d", FutureCursorAction.ERROR),
            invocation_time=datetime(2026, 9, 1, 12, 0, tzinfo=UTC),
            expected_failed_phase=ExecutionPhase.STAGING,
            expected_error_fragment="future cursor safety limit exceeded",
            expected_query_results=(("SELECT * FROM main.hook_log", ()),),
        ),
        IncrementalFailureTestCase(
            description="failed pre-hook preserves resolved future cap evidence",
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
            cursor_input_relations=(("main.future_input", "event_time"),),
            cursor_inputs_model_backed=True,
            pre_hook=[SqlHookEntry(statement="SELECT * FROM missing_pre_hook_table")],
            future_cursor_config=FutureCursorsConfig("2d", FutureCursorAction.CAP),
            invocation_time=datetime(2026, 9, 1, 12, 0, tzinfo=UTC),
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
            expected_error_fragment="missing_pre_hook_table",
            expected_has_future_cursor_safety=True,
        ),
        IncrementalFailureTestCase(
            description="sync_all_columns fails on incompatible type change",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, val VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'not_a_number')",
            ),
            model_sql="SELECT 2 AS id, 200 AS val",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.SYNC_ALL_COLUMNS,
            expected_failed_phase=ExecutionPhase.SCHEMA_CHANGE,
            expected_error_fragment="Conversion Error",
            expected_staging_relation="main.orders__delta",
            expected_row_count=1,
        ),
        IncrementalFailureTestCase(
            description="on_schema_change fail rejects schema diff before DML",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name, 100 AS amount",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.FAIL,
            expected_failed_phase=ExecutionPhase.SCHEMA_CHANGE,
            expected_error_fragment="on_schema_change is set to fail",
            expected_staging_relation="main.orders__delta",
            expected_row_count=1,
        ),
        IncrementalFailureTestCase(
            description="append_new_columns rejects invalid target-preserving coercion",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, amount INTEGER)",
                "INSERT INTO main.orders VALUES (1, 100)",
            ),
            model_sql="SELECT 2 AS id, 'not-a-number' AS amount",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.APPEND_NEW_COLUMNS,
            expected_failed_phase=ExecutionPhase.TYPE_ENFORCEMENT,
            expected_error_fragment="Could not convert string",
            expected_staging_relation="main.orders__delta",
            expected_row_count=1,
        ),
        IncrementalFailureTestCase(
            description="declared type remains authoritative over existing target type",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, amount VARCHAR)",
                "INSERT INTO main.orders VALUES (1, '100')",
            ),
            model_sql="SELECT 2 AS id, 200::DOUBLE AS amount",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            on_schema_change=OnSchemaChange.APPEND_NEW_COLUMNS,
            type_enforcement=True,
            declared_columns=(("amount", "DOUBLE"),),
            expected_failed_phase=ExecutionPhase.SCHEMA_CHANGE,
            expected_error_fragment="does not support type changes: amount",
            expected_staging_relation="main.orders__delta",
            expected_row_count=1,
        ),
        IncrementalFailureTestCase(
            description="delta audit error blocks DML via override to delta relation",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            audit_sql='SELECT id FROM __ref("orders") WHERE id = 2',
            audit_severity="error",
            audit_run_scope=AuditRunScope.DELTA_AND_FINAL,
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_error_fragment="delta audit for 'orders' failed before target update",
            expected_staging_relation="main.orders__delta",
            expected_audit_count=1,
            expected_row_count=1,
        ),
        IncrementalFailureTestCase(
            description="final audit error after DML reports target already updated",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            audit_sql='SELECT id FROM __ref("orders") WHERE id > 0',
            audit_severity="error",
            audit_run_scope=AuditRunScope.FINAL,
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_error_fragment="final audit for 'orders' failed after target update",
            expected_staging_relation="main.orders__delta",
            expected_promoted_relation="main.orders",
            expected_audit_count=1,
            expected_row_count=2,
        ),
        IncrementalFailureTestCase(
            description="pre_hook failure blocks incremental execution",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            pre_hook=[SqlHookEntry(statement="SELECT * FROM nonexistent_table_for_hook")],
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
            expected_row_count=1,
        ),
        IncrementalFailureTestCase(
            description="python pre_hook failure blocks incremental execution",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            pre_hook=[PythonHookEntry(name="fail_hook", kwargs={"message": "pre boom"})],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/incremental.py"),
                    name="fail_hook",
                    function=fail_incremental_hook,
                ),
            ),
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
            expected_error_fragment='pre_hooks[0] python("fail_hook") failed: pre boom',
            expected_row_count=1,
        ),
        IncrementalFailureTestCase(
            description="post_hook failure after DML marks model failed",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            post_hook=[SqlHookEntry(statement="SELECT * FROM nonexistent_table_for_hook")],
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_staging_relation="main.orders__delta",
            expected_promoted_relation="main.orders",
            expected_row_count=2,
        ),
        IncrementalFailureTestCase(
            description="python post_hook failure after DML marks model failed",
            setup_sql=(
                "CREATE TABLE main.orders (id INTEGER, name VARCHAR)",
                "INSERT INTO main.orders VALUES (1, 'alice')",
            ),
            model_sql="SELECT 2 AS id, 'bob' AS name",
            target_schema="main",
            target_name="orders",
            incremental_strategy="append",
            post_hook=[PythonHookEntry(name="fail_hook", kwargs={"message": "post boom"})],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/incremental.py"),
                    name="fail_hook",
                    function=fail_incremental_hook,
                ),
            ),
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_error_fragment='post_hooks[0] python("fail_hook") failed: post boom',
            expected_staging_relation="main.orders__delta",
            expected_promoted_relation="main.orders",
            expected_row_count=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_table_when_incremental_fails_then_reports_correct_phase(
    test_case: IncrementalFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_failure_test(
        test_case=test_case, adapter=adapter, connection=connection
    )
    assert result.failed_phase == test_case.expected_failed_phase
    verify_failure_state(result=result, test_case=test_case, connection=connection)
