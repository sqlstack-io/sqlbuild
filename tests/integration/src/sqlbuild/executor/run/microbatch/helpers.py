"""Test helpers for microbatch executor integration tests."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import replace
from pathlib import Path
from typing import Any, cast

from sqlbuild._native import (
    MICROBATCH_END_SENTINEL,
    MICROBATCH_START_SENTINEL,
    AuditPlanEntry,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CursorBounds,
    CursorInputRelation,
    DiscoveredHookFunction,
    IncrementalMode,
    IncrementalStrategy,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditSeverity,
)
from sqlbuild.executor.run._helpers.materializations.microbatch import execute_microbatch_entry
from sqlbuild.executor.run._helpers.validation.cursor_bounds import resolve_runtime_cursor_bounds
from sqlbuild.executor.run.models import (
    BatchWindow,
    HookContext,
    HookSkipResult,
    ModelExecutionResult,
    ModelMaterializationContext,
    RuntimeCursorInputRelation,
    RuntimeCursorSpec,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.observability import EventDispatcher, LifecycleEvent
from tests.integration.src.sqlbuild.executor.run.microbatch._test_types import (
    MicrobatchFailureTestCase,
    MicrobatchSuccessTestCase,
)

_STRATEGY_TO_ACTION: dict[str, PlanAction] = {
    IncrementalStrategy.APPEND: PlanAction.INCREMENTAL_APPEND,
    IncrementalStrategy.DELETE_INSERT: PlanAction.INCREMENTAL_DELETE_INSERT,
    IncrementalStrategy.MERGE: PlanAction.INCREMENTAL_MERGE,
}


def capture_lifecycle_events() -> tuple[EventDispatcher, list[LifecycleEvent]]:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    return dispatcher, events


def lifecycle_events_with_prefix(
    *, events: list[LifecycleEvent], prefix: str
) -> tuple[LifecycleEvent, ...]:
    return tuple(filter(lambda event: event.event_type.startswith(prefix), events))


def lifecycle_events_by_type(
    *, events: tuple[LifecycleEvent, ...], event_type: str
) -> tuple[LifecycleEvent, ...]:
    return tuple(filter(lambda event: event.event_type == event_type, events))


def fail_microbatch_cleanup(**_: object) -> None:
    raise RuntimeError("cleanup failed")


def resolve_nonempty_terminal_bounds(
    *, adapter: DuckDbAdapter, connection: Any, mode: str
) -> CursorBounds | None:
    connection.execute("CREATE TABLE main.target_events (event_time TIMESTAMP)")
    connection.execute("CREATE TABLE main.archive_events (event_time TIMESTAMP)")
    connection.execute("CREATE TABLE main.live_events (event_time TIMESTAMP)")
    connection.execute("INSERT INTO main.archive_events VALUES ('2025-11-15'), ('2026-02-01')")
    connection.execute("INSERT INTO main.live_events VALUES ('2026-07-02')")
    return resolve_runtime_cursor_bounds(
        adapter=adapter,
        connection=connection,
        target_relation="main.target_events",
        target_database=None,
        target_schema="main",
        target_name="target_events",
        spec=RuntimeCursorSpec(
            cursor_column="event_time",
            cursor_type="timestamp",
            cursor_grain="day",
            cursor_start="2025-01-01",
            cursor_input_relations=(
                RuntimeCursorInputRelation(
                    relation="main.archive_events",
                    cursor_column="event_time",
                    terminal_cursor_start="2025-01-01",
                    terminal_cursor_end="2025-12-01",
                ),
                RuntimeCursorInputRelation(relation="main.live_events", cursor_column="event_time"),
            ),
            cursor_watermark_mode=mode,
        ),
    )


def messages_with_prefix(*, messages: list[str], prefix: str) -> tuple[str, ...]:
    return tuple(filter(lambda message: message.startswith(prefix), messages))


def insert_microbatch_hook_log(ctx: HookContext, phase: str) -> None:
    ctx.execute_sql(f"INSERT INTO {ctx.destination.schema}.hook_log VALUES ('{phase}')")


def fail_microbatch_hook(ctx: HookContext, message: str) -> None:
    raise RuntimeError(message)


def skip_microbatch_hook(ctx: HookContext, reason: str) -> HookSkipResult:
    return ctx.skip(reason=reason)


def reconcile_microbatch_batches(
    *, state: Any, history: Any, reconciled_batches: tuple[BatchWindow, ...], **_: Any
) -> tuple[Any, Any, tuple[BatchWindow, ...]]:
    return state, history, reconciled_batches


def build_microbatch_plan_entry(
    *,
    test_case: MicrobatchSuccessTestCase | MicrobatchFailureTestCase,
) -> ModelPlanEntry:
    """Build a ModelPlanEntry for microbatch execution tests."""

    target_schema: str | None = test_case.target_schema
    target_name: str = test_case.target_name
    qualified: str = f"{target_schema or ''}.{target_name}".lstrip(".")
    action, reason = {
        False: (_STRATEGY_TO_ACTION[test_case.incremental_strategy], PlanReason.NORMAL_INCREMENTAL),
        True: (PlanAction.CREATE_TABLE, PlanReason.FULL_REFRESH),
    }[test_case.is_full_refresh]

    microbatch_range: CursorBounds | None = {
        True: None,
        False: CursorBounds(
            start=test_case.microbatch_start,
            end=test_case.microbatch_end,
        ),
    }[test_case.is_full_refresh or not test_case.use_plan_microbatch_range]
    cursor_input_relations: tuple[CursorInputRelation, ...] = tuple(
        CursorInputRelation(
            relation=relation,
            cursor_column=cursor_column,
            cursor_grain=test_case.cursor_input_grain,
            is_model_backed=test_case.cursor_inputs_model_backed,
            is_runtime_produced=test_case.cursor_inputs_model_backed,
        )
        for relation, cursor_column in test_case.cursor_input_relations
    )

    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
        name="orders",
        relative_path=Path("sql/models/orders.sql"),
        materialization_type=MaterializationType.INCREMENTAL,
        action=action,
        reason=reason,
        destination=CompiledRelationLocation(
            database=None,
            schema=target_schema,
            name=target_name,
            qualified_name=qualified,
        ),
        fingerprint_query_sql=test_case.model_sql,
        resolved_sql=test_case.model_sql,
        logical_ddl="",
        incremental_strategy=test_case.incremental_strategy,
        incremental_mode=IncrementalMode.MICROBATCH,
        cursor_column=test_case.cursor_column,
        cursor_type=test_case.cursor_type,
        cursor_grain=test_case.cursor_grain,
        cursor_bounds=CursorBounds(start=MICROBATCH_START_SENTINEL, end=MICROBATCH_END_SENTINEL),
        cursor_input_relations=cursor_input_relations,
        batch_size=test_case.batch_size,
        microbatch_range=microbatch_range,
        start_cursor_override=test_case.start_cursor_override,
        end_cursor_override=test_case.end_cursor_override,
        unique_key=test_case.unique_key,
        on_schema_change=test_case.on_schema_change,
        pre_hooks=test_case.pre_hook,
        post_hooks=test_case.post_hook,
        microbatch_limit=test_case.microbatch_limit,
        microbatch_limit_action=test_case.microbatch_limit_action,
    )


def build_integer_reconciliation_plan_entry() -> ModelPlanEntry:
    """Build the integer microbatch entry used by row-count reconciliation tests."""

    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
        name="orders",
        relative_path=Path("sql/models/orders.sql"),
        materialization_type=MaterializationType.INCREMENTAL,
        action=PlanAction.INCREMENTAL_DELETE_INSERT,
        reason=PlanReason.NORMAL_INCREMENTAL,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="orders",
            qualified_name="main.orders",
        ),
        fingerprint_query_sql="SELECT id FROM main.orders",
        resolved_sql="SELECT id FROM main.orders",
        logical_ddl="",
        incremental_strategy="delete_insert",
        incremental_mode=IncrementalMode.MICROBATCH,
        cursor_column="id",
        cursor_type="integer",
        batch_size="1",
    )


def run_success_test(
    *,
    test_case: MicrobatchSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> ModelExecutionResult:
    """Execute a microbatch success test case and return the result."""

    result: ModelExecutionResult = _execute_test(
        test_case=test_case, adapter=adapter, connection=connection
    )
    assert result.status == ExecutionStatus.SUCCESS
    return result


def run_failure_test(
    *,
    test_case: MicrobatchFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> ModelExecutionResult:
    """Execute a microbatch failure test case and return the result."""

    result: ModelExecutionResult = _execute_test(
        test_case=test_case, adapter=adapter, connection=connection
    )
    assert result.status == ExecutionStatus.FAILED
    return result


def run_skipped_test(
    *,
    test_case: MicrobatchSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> ModelExecutionResult:
    """Execute a microbatch hook-skip test case and return the result."""

    result: ModelExecutionResult = _execute_test(
        test_case=test_case, adapter=adapter, connection=connection
    )
    assert result.status == ExecutionStatus.SKIPPED
    return result


def verify_success_state(
    *,
    result: ModelExecutionResult,
    test_case: MicrobatchSuccessTestCase,
    connection: Any,
) -> None:
    """Verify warehouse state and result for a successful microbatch execution."""

    target_qualified: str = _build_target_qualified(
        target_schema=test_case.target_schema, target_name=test_case.target_name
    )
    query_result: Any = connection.execute(f"SELECT COUNT(*) FROM {target_qualified}")
    actual_count: int = query_result.fetchone()[0]
    assert actual_count == test_case.expected_row_count
    assert len(result.audit_results) == test_case.expected_audit_count
    assert len(result.warning_messages) == test_case.expected_warning_count
    normalized_batch_count: int | None = {None: result.batch_count}.get(
        test_case.expected_batch_count, test_case.expected_batch_count
    )
    assert result.batch_count == normalized_batch_count
    assert (result.future_cursor_safety is not None) is test_case.expected_has_future_cursor_safety
    state_table_count: int = connection.execute(
        "SELECT COUNT(*) FROM information_schema.tables "
        "WHERE table_schema = 'main' AND table_name = '_sqlbuild_microbatches'"
    ).fetchone()[0]
    assert state_table_count == 0

    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        cursor: Any = connection.execute(query)
        actual_rows: tuple[tuple[object, ...], ...] = tuple(tuple(row) for row in cursor.fetchall())
        assert actual_rows == expected_rows, (
            f"Query: {query}\nExpected: {expected_rows}\nActual: {actual_rows}"
        )

    cursor = connection.execute(f"SELECT * FROM {target_qualified} LIMIT 0")
    actual_names: tuple[str, ...] = tuple(desc[0] for desc in cursor.description)
    name_normalizations: dict[tuple[str, ...], tuple[str, ...]] = {(): ()}
    normalized_names: tuple[str, ...] = name_normalizations.get(
        test_case.expected_column_names, actual_names
    )
    assert normalized_names == test_case.expected_column_names

    delta_qualified: str = _build_target_qualified(
        target_schema=test_case.target_schema,
        target_name=f"{test_case.target_name}__delta",
    )
    delta_exists: bool = _relation_exists(connection, delta_qualified)
    normalized_delta_exists: bool = {False: False}.get(
        test_case.expected_delta_cleaned, delta_exists
    )
    assert not normalized_delta_exists

    statement_output: str = "\n".join(e.content for e in result.lifecycle_events)
    expected_fragment: str
    for expected_fragment in test_case.expected_executed_statement_fragments:
        assert expected_fragment in statement_output


def verify_failure_state(
    *,
    result: ModelExecutionResult,
    test_case: MicrobatchFailureTestCase,
    connection: Any,
) -> None:
    """Verify result fields and warehouse state for a failed microbatch execution."""

    assert result.failed_phase == test_case.expected_failed_phase
    assert (result.future_cursor_safety is not None) is test_case.expected_has_future_cursor_safety
    assert len(result.audit_results) == test_case.expected_audit_count

    assert (test_case.expected_error_fragment or "") in (result.error_message or "")
    _FAILURE_ROW_COUNT_VERIFIERS.get(
        test_case.expected_row_count, _verify_present_failure_row_count
    )(connection=connection, test_case=test_case)

    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        cursor: Any = connection.execute(query)
        actual_rows: tuple[tuple[object, ...], ...] = tuple(tuple(row) for row in cursor.fetchall())
        assert actual_rows == expected_rows, (
            f"Query: {query}\nExpected: {expected_rows}\nActual: {actual_rows}"
        )

    delta_qualified: str = _build_target_qualified(
        target_schema=test_case.target_schema,
        target_name=f"{test_case.target_name}__delta",
    )
    delta_exists: bool = _relation_exists(connection, delta_qualified)
    normalized_delta_exists: bool = {False: False}.get(
        test_case.expected_delta_retained, delta_exists
    )
    assert normalized_delta_exists == test_case.expected_delta_retained


def _execute_test(
    *,
    test_case: MicrobatchSuccessTestCase | MicrobatchFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> ModelExecutionResult:
    """Set up and execute a microbatch test case."""

    sql: str
    for sql in test_case.setup_sql:
        connection.execute(sql)

    entry: ModelPlanEntry = build_microbatch_plan_entry(test_case=test_case)
    entry = replace(
        entry,
        future_cursor_config=test_case.future_cursor_config,
        invocation_time=test_case.invocation_time,
    )
    model_audits: tuple[AuditPlanEntry, ...] = _build_model_audits(test_case)
    target_qualified: str = _build_target_qualified(
        target_schema=test_case.target_schema, target_name=test_case.target_name
    )
    model_locations: dict[str, CompiledRelationLocation] = {
        "orders": CompiledRelationLocation(
            database=None,
            schema=test_case.target_schema,
            name=test_case.target_name,
            qualified_name=target_qualified,
        ),
    }

    return execute_microbatch_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations=model_locations,
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=model_audits,
            run_id="test_run",
            query_change_tracking=getattr(test_case, "query_change_tracking", True),
            hook_functions=cast(
                tuple[DiscoveredHookFunction, ...], getattr(test_case, "hook_functions", ())
            ),
        ),
        declared_columns=(),
        is_full_refresh=test_case.is_full_refresh,
        on_progress=getattr(test_case, "on_progress", None),
    )


def _build_model_audits(
    test_case: MicrobatchSuccessTestCase | MicrobatchFailureTestCase,
) -> tuple[AuditPlanEntry, ...]:
    """Build model audits from test case audit config."""

    resolved_target_name: str = _build_target_qualified(
        target_schema=test_case.target_schema,
        target_name=test_case.target_name,
    )
    audit: tuple[AuditPlanEntry, ...] = (
        AuditPlanEntry(
            key=CompiledObjectKey(resource_type=CompiledResourceType.AUDIT, name="test_audit"),
            definition_name="test_audit",
            name="test_audit",
            resolved_sql=(test_case.audit_sql or "").replace(
                '__ref("orders")', resolved_target_name
            ),
            unresolved_sql=test_case.audit_sql or "",
            attachment_kind=AuditAttachmentKind.MODEL,
            severity=AuditSeverity(test_case.audit_severity),
            requested_run_scope=test_case.audit_run_scope,
            effective_run_scope=test_case.audit_run_scope,
            attached_target_name="orders",
        ),
    )
    return {True: (), False: audit}[test_case.audit_sql is None]


def _build_target_qualified(*, target_schema: str | None, target_name: str) -> str:
    return f"{target_schema or ''}.{target_name}".lstrip(".")


def _relation_exists(connection: Any, qualified_name: str) -> bool:
    schema: str
    name: str
    schema, _, name = qualified_name.rpartition(".")
    name = name or schema
    schema = schema.removesuffix(name)
    cursor: Any = connection.execute(
        "SELECT 1 FROM information_schema.tables "
        f"WHERE table_name = '{name}'" + f" AND table_schema = '{schema}'" * bool(schema)
    )
    return cursor.fetchone() is not None


def _ignore_failure_row_count(*, connection: Any, test_case: MicrobatchFailureTestCase) -> None:
    del connection, test_case


def _verify_present_failure_row_count(
    *, connection: Any, test_case: MicrobatchFailureTestCase
) -> None:
    target_qualified: str = _build_target_qualified(
        target_schema=test_case.target_schema, target_name=test_case.target_name
    )
    actual_count: int = connection.execute(f"SELECT COUNT(*) FROM {target_qualified}").fetchone()[0]
    assert actual_count == test_case.expected_row_count


_FAILURE_ROW_COUNT_VERIFIERS: dict[object, Callable[..., None]] = {None: _ignore_failure_row_count}
