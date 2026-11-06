"""Incremental model execution lifecycle."""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import Any

from sqlbuild._native import (
    CursorBounds,
    IncrementalStrategy,
    ModelPlanEntry,
    OnSchemaChange,
    TableType,
    fit_auxiliary_relation_name,
    future_cursor_cap_warning,
    maximum_start_cap_warning,
)
from sqlbuild._native import cursor_sentinel_to_token as sentinel_to_token
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.relations.main.resolve_qualified_name_parts import (
    resolve_qualified_name_parts,
)
from sqlbuild.adapter.relations.main.resolve_relation_location_qualified_name import (
    resolve_relation_location_qualified_name,
)
from sqlbuild.adapter.type_system.main.types_equal import types_equal
from sqlbuild.diagnostics.main.diagnostics_context import diagnostics_context
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.run._helpers.execution.final_audits import (
    run_delta_scope_audits,
    run_final_scope_audits,
)
from sqlbuild.executor.run._helpers.execution.hook_phases import (
    run_post_hook_phase,
    run_pre_hook_phase,
)
from sqlbuild.executor.run._helpers.execution.results import (
    build_failed_result,
    build_skipped_result,
)
from sqlbuild.executor.run._helpers.execution.schema import inspect_runtime_relation_schema
from sqlbuild.executor.run._helpers.execution.staging import create_intermediate_table
from sqlbuild.executor.run._helpers.execution.table_config import model_output_config
from sqlbuild.executor.run._helpers.reuse.fingerprinting import try_write_fingerprint
from sqlbuild.executor.run._helpers.validation.contracts import validate_runtime_contract
from sqlbuild.executor.run._helpers.validation.cursor_bounds import (
    build_runtime_cursor_spec,
    cursor_window_unavailable_error,
    has_authoritative_cursor_override,
    has_runtime_owned_cursor_watermarks,
    resolve_runtime_cursor_bounds,
    substitute_cursor_sentinels,
)
from sqlbuild.executor.run._helpers.validation.type_enforcement import enforce_types_staged
from sqlbuild.executor.run.constants import DELTA_RELATION_SUFFIX
from sqlbuild.executor.run.exceptions import EmptyCursorInputsError
from sqlbuild.executor.run.models import (
    BoundViewGuard,
    FinalAuditRun,
    HookExecutionResult,
    ModelExecutionResult,
    ModelMaterializationContext,
    PostHookPhaseOutcome,
)
from sqlbuild.executor.run.types import ExecutionPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.runtime.observability.classes.operation_lifecycle import (
    OperationAttributes,
    OperationLifecycle,
)
from sqlbuild.runtime.observability.main.canonicalize_operation_adapter import (
    canonicalize_operation_adapter,
)

_DEFAULT_ON_SCHEMA_CHANGE: OnSchemaChange = OnSchemaChange.APPEND_NEW_COLUMNS
_SCHEMA_CHANGING_POLICIES: frozenset[OnSchemaChange] = frozenset(
    {OnSchemaChange.APPEND_NEW_COLUMNS, OnSchemaChange.SYNC_ALL_COLUMNS}
)


@dataclass(frozen=True)
class _DeltaPreparation:
    """Resolved SQL and runtime cursor bounds after delta relation staging."""

    resolved_sql: str
    runtime_cursor_bounds: CursorBounds | None


@dataclass(frozen=True)
class _IncrementalTarget:
    """Physical destination of one incremental model."""

    database: str | None
    schema: str | None
    table: str
    qualified: str


@dataclass(frozen=True)
class _DeltaTypeInputs:
    """Physical delta and target schema inputs for pre-DML type preparation."""

    target_qualified: str
    delta_qualified: str
    target_database: str | None
    target_schema: str | None
    delta_table: str
    target_columns: tuple[ColumnInfo, ...]
    delta_columns: tuple[ColumnInfo, ...]
    declared_columns: tuple[ColumnInfo, ...]


def execute_incremental_entry(
    *,
    context: ModelMaterializationContext,
    declared_columns: tuple[ColumnInfo, ...],
) -> ModelExecutionResult:
    """Execute one incremental model through its delta/DML lifecycle."""

    entry: ModelPlanEntry = context.entry
    adapter: BaseAdapter = context.adapter
    connection: Any = context.connection
    target_database: str | None = entry.destination.database
    target_schema: str | None = entry.destination.schema
    target_table: str = entry.destination.name
    target_qualified: str = resolve_relation_location_qualified_name(
        adapter=adapter, location=entry.destination
    )
    delta_table: str = fit_auxiliary_relation_name(
        base_name=target_table,
        suffix=DELTA_RELATION_SUFFIX,
        identifier_limit=adapter.maximum_identifier_length(),
    )
    delta_qualified: str = resolve_qualified_name_parts(
        adapter=adapter,
        database=target_database,
        schema=target_schema,
        name=delta_table,
    )
    warnings: list[str] = []
    audit_results: list[AuditExecutionResult] = []
    hook_results: list[HookExecutionResult] = []
    statement_recorder: StatementRecorder = StatementRecorder()

    prepared: _DeltaPreparation | ModelExecutionResult = _prepare_cursor_window(
        context=context,
        targets=_IncrementalTarget(
            database=target_database,
            schema=target_schema,
            table=target_table,
            qualified=target_qualified,
        ),
        statement_recorder=statement_recorder,
    )
    if isinstance(prepared, ModelExecutionResult):
        return prepared
    preparation: _DeltaPreparation = prepared
    context = _context_with_runtime_cursor(context=context, preparation=preparation)
    entry = context.entry
    warnings.extend(_cursor_safety_warnings(entry.cursor_bounds))

    pre_hook_exit: ModelExecutionResult | None = run_pre_hook_phase(
        context=context,
        warnings=warnings,
        audit_results=audit_results,
        hook_results=hook_results,
        statement_recorder=statement_recorder,
    )
    if pre_hook_exit is not None:
        return pre_hook_exit

    try:
        _prepare_delta_relation(
            context=context,
            target_database=target_database,
            target_schema=target_schema,
            delta_qualified=delta_qualified,
            preparation=preparation,
            statement_recorder=statement_recorder,
        )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.STAGING,
            error=exc,
            staging_relation=delta_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    try:
        with diagnostics_context(
            sqlbuild_phase="type_enforcement", sqlbuild_action_name="rebuild_delta"
        ):
            initial_delta_columns: tuple[ColumnInfo, ...] = inspect_runtime_relation_schema(
                adapter=adapter,
                connection=connection,
                database=target_database,
                schema=target_schema,
                name=delta_table,
            )
            target_columns: tuple[ColumnInfo, ...] = adapter.widen_target_nullability(
                connection=connection,
                destination=target_qualified,
                target_columns=inspect_runtime_relation_schema(
                    adapter=adapter,
                    connection=connection,
                    database=target_database,
                    schema=target_schema,
                    name=target_table,
                ),
                incoming_columns=_widenable_columns(
                    on_schema_change=entry.on_schema_change or _DEFAULT_ON_SCHEMA_CHANGE,
                    incoming_columns=initial_delta_columns,
                ),
                statement_recorder=statement_recorder,
            )
            delta_columns, type_warnings = _prepare_incremental_delta_types(
                adapter=adapter,
                connection=connection,
                entry=entry,
                inputs=_DeltaTypeInputs(
                    target_qualified=target_qualified,
                    delta_qualified=delta_qualified,
                    target_database=target_database,
                    target_schema=target_schema,
                    delta_table=delta_table,
                    target_columns=target_columns,
                    delta_columns=initial_delta_columns,
                    declared_columns=declared_columns,
                ),
                statement_recorder=statement_recorder,
            )
            warnings.extend(type_warnings)
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.TYPE_ENFORCEMENT,
            error=exc,
            staging_relation=delta_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    try:
        with diagnostics_context(sqlbuild_phase="schema_change", sqlbuild_action_name="inspect"):
            warnings.extend(
                _apply_schema_change(
                    adapter=adapter,
                    connection=connection,
                    target_qualified=target_qualified,
                    target_columns=target_columns,
                    delta_columns=delta_columns,
                    on_schema_change=entry.on_schema_change or _DEFAULT_ON_SCHEMA_CHANGE,
                    statement_recorder=statement_recorder,
                    bound_views=context.bound_views,
                )
            )
            target_columns = inspect_runtime_relation_schema(
                adapter=adapter,
                connection=connection,
                database=target_database,
                schema=target_schema,
                name=target_table,
            )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.SCHEMA_CHANGE,
            error=exc,
            staging_relation=delta_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )

    try:
        with diagnostics_context(sqlbuild_phase="contract", sqlbuild_action_name="validate_delta"):
            delta_columns: tuple[ColumnInfo, ...] = inspect_runtime_relation_schema(
                adapter=adapter,
                connection=connection,
                database=target_database,
                schema=target_schema,
                name=delta_table,
            )
            validate_runtime_contract(
                entry=entry,
                actual_columns=delta_columns,
                dialect=adapter.sql_analysis_dialect_name,
            )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.CONTRACT,
            error=exc,
            staging_relation=delta_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )

    delta_audit_run: FinalAuditRun = run_delta_scope_audits(
        context=context, delta_qualified=delta_qualified
    )
    audit_results.extend(delta_audit_run.results)

    if delta_audit_run.has_error:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.AUDIT,
            error=(
                f"delta audit for '{entry.name}' failed before target update "
                "with severity level: error"
            ),
            staging_relation=delta_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )

    effective_bounds: CursorBounds | None = preparation.runtime_cursor_bounds or entry.cursor_bounds
    cursor_start: str | None = (
        sentinel_to_token(sentinel=effective_bounds.start) if effective_bounds else None
    )
    cursor_end: str | None = (
        sentinel_to_token(sentinel=effective_bounds.end) if effective_bounds else None
    )

    try:
        with diagnostics_context(sqlbuild_phase="dml", sqlbuild_action_name="apply"):
            _execute_dml(
                adapter=adapter,
                connection=connection,
                target_qualified=target_qualified,
                delta_qualified=delta_qualified,
                target_columns=target_columns,
                delta_columns=delta_columns,
                entry=entry,
                cursor_start=cursor_start,
                cursor_end=cursor_end,
                statement_recorder=statement_recorder,
            )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.DML,
            error=exc,
            staging_relation=delta_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )

    final_audit_run: FinalAuditRun = run_final_scope_audits(context=context)
    audit_results.extend(final_audit_run.results)

    if final_audit_run.has_error:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.AUDIT,
            error=(
                f"final audit for '{entry.name}' failed after target update "
                "with severity level: error"
            ),
            staging_relation=delta_qualified,
            promoted_relation=target_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )

    post_hook_outcome: PostHookPhaseOutcome = run_post_hook_phase(
        context=context,
        warnings=warnings,
        audit_results=audit_results,
        hook_results=hook_results,
        statement_recorder=statement_recorder,
        staging_relation=delta_qualified,
        promoted_relation=target_qualified,
    )
    if post_hook_outcome.failure is not None:
        return post_hook_outcome.failure
    if post_hook_outcome.skipped:
        with diagnostics_context(sqlbuild_phase="cleanup", sqlbuild_action_name="drop_delta"):
            adapter.drop_table(
                connection=connection,
                destination=delta_qualified,
                statement_recorder=statement_recorder,
            )
        return build_skipped_result(
            entry=entry,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
            promoted_relation=target_qualified,
        )

    warnings.extend(
        try_write_fingerprint(
            entry=entry,
            adapter=adapter,
            connection=connection,
            run_id=context.run_id,
            query_change_tracking=context.query_change_tracking,
            fingerprint_writes=context.fingerprint_writes,
            model_audits=context.model_audits,
            audit_results=tuple(audit_results),
        )
    )

    with diagnostics_context(sqlbuild_phase="cleanup", sqlbuild_action_name="drop_delta"):
        adapter.drop_table(
            connection=connection,
            destination=delta_qualified,
            statement_recorder=statement_recorder,
        )

    return ModelExecutionResult(
        model_name=entry.name,
        status=ExecutionStatus.SUCCESS,
        promoted_relation=target_qualified,
        cursor_range_start=cursor_start,
        cursor_range_end=cursor_end,
        cursor_type=entry.cursor_type,
        cursor_grain=entry.cursor_grain,
        future_cursor_safety=(
            effective_bounds.future_safety if effective_bounds is not None else None
        ),
        maximum_start_safety=(
            effective_bounds.maximum_start_safety if effective_bounds is not None else None
        ),
        audit_results=tuple(audit_results),
        warning_messages=tuple(warnings),
        lifecycle_events=statement_recorder.snapshot(),
        hook_results=tuple(hook_results),
    )


def _no_input_rows_result(
    *,
    context: ModelMaterializationContext,
    targets: _IncrementalTarget,
    input_names: tuple[str, ...],
    waiting_on_empty_inputs: bool,
    statement_recorder: StatementRecorder,
) -> ModelExecutionResult:
    """Leave the destination unchanged when the cursor inputs have no rows, then audit it."""

    entry: ModelPlanEntry = context.entry
    final_audit_run: FinalAuditRun = run_final_scope_audits(context=context)
    if final_audit_run.has_error:
        return replace(
            build_failed_result(
                entry=entry,
                phase=ExecutionPhase.AUDIT,
                error=f"final audit for '{entry.name}' failed with severity level: error",
                promoted_relation=targets.qualified,
                warnings=[],
                audit_results=list(final_audit_run.results),
                statement_recorder=statement_recorder,
            ),
            empty_cursor_inputs=input_names,
            waiting_on_empty_inputs=waiting_on_empty_inputs,
        )
    return ModelExecutionResult(
        model_name=entry.name,
        status=ExecutionStatus.SUCCESS,
        promoted_relation=targets.qualified,
        cursor_type=entry.cursor_type,
        cursor_grain=entry.cursor_grain,
        empty_cursor_inputs=input_names,
        waiting_on_empty_inputs=waiting_on_empty_inputs,
        audit_results=final_audit_run.results,
        lifecycle_events=statement_recorder.snapshot(),
    )


def _prepare_delta_relation(
    *,
    context: ModelMaterializationContext,
    target_database: str | None,
    target_schema: str | None,
    delta_qualified: str,
    preparation: _DeltaPreparation,
    statement_recorder: StatementRecorder,
) -> None:
    """Create the delta relation after cursor safety has resolved."""

    adapter: BaseAdapter = context.adapter
    connection: Any = context.connection
    if not context.schema_prepared:
        adapter.ensure_schema(
            connection=connection,
            database=target_database,
            schema=target_schema,
            statement_recorder=statement_recorder,
        )
    with diagnostics_context(sqlbuild_phase="materialize", sqlbuild_action_name="create_delta"):
        with OperationLifecycle(
            operation_kind="warehouse",
            operation_name="staging_creation",
            attributes=OperationAttributes(
                phase="create",
                adapter=canonicalize_operation_adapter(adapter.adapter_name),
                target_kind="staging_relation",
            ),
        ) as lifecycle:
            create_intermediate_table(
                adapter=adapter,
                connection=connection,
                destination=delta_qualified,
                sql=preparation.resolved_sql,
                config=model_output_config(context.entry),
                statement_recorder=statement_recorder,
            )
            lifecycle.completed(metadata={"changed_count": 1})


def _resolve_incremental_cursor_bounds(
    *,
    context: ModelMaterializationContext,
    target_database: str | None,
    target_schema: str | None,
    target_table: str,
    target_qualified: str,
) -> _DeltaPreparation:
    """Resolve runtime cursor policy before pre-hook execution."""

    entry: ModelPlanEntry = context.entry
    if not has_runtime_owned_cursor_watermarks(
        entry.cursor_input_relations
    ) or has_authoritative_cursor_override(entry=entry):
        return _DeltaPreparation(resolved_sql=entry.resolved_sql, runtime_cursor_bounds=None)
    if entry.cursor_column is None:
        raise ExecutorInputError("runtime-owned cursor resolution requires cursor_column")
    runtime_cursor_bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=context.adapter,
        connection=context.connection,
        target_relation=target_qualified,
        target_database=target_database,
        target_schema=target_schema,
        target_name=target_table,
        spec=build_runtime_cursor_spec(entry=entry),
        watermark_resolver=context.watermark_resolver,
    )
    if runtime_cursor_bounds is None:
        raise cursor_window_unavailable_error(entry=entry)
    return _DeltaPreparation(
        resolved_sql=substitute_cursor_sentinels(
            sql=entry.resolved_sql, bounds=runtime_cursor_bounds
        ),
        runtime_cursor_bounds=runtime_cursor_bounds,
    )


def _prepare_cursor_window(
    *,
    context: ModelMaterializationContext,
    targets: _IncrementalTarget,
    statement_recorder: StatementRecorder,
) -> _DeltaPreparation | ModelExecutionResult:
    """Settle the cursor window before hooks or staging, or return the run's final result."""

    entry: ModelPlanEntry = context.entry
    if entry.empty_cursor_inputs:
        return _no_input_rows_result(
            context=context,
            targets=targets,
            input_names=entry.empty_cursor_inputs,
            waiting_on_empty_inputs=entry.waiting_on_empty_inputs,
            statement_recorder=statement_recorder,
        )
    try:
        preparation: _DeltaPreparation = _resolve_incremental_cursor_bounds(
            context=context,
            target_database=targets.database,
            target_schema=targets.schema,
            target_table=targets.table,
            target_qualified=targets.qualified,
        )
        if _lacks_required_cursor_window(
            entry=_context_with_runtime_cursor(context=context, preparation=preparation).entry
        ):
            raise cursor_window_unavailable_error(entry=entry)
    except EmptyCursorInputsError as exc:
        return _no_input_rows_result(
            context=context,
            targets=targets,
            input_names=exc.input_names,
            waiting_on_empty_inputs=exc.waiting_on_empty_inputs,
            statement_recorder=statement_recorder,
        )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.STAGING,
            error=exc,
            warnings=[],
            audit_results=[],
            statement_recorder=statement_recorder,
            hook_results=[],
        )
    return preparation


def _lacks_required_cursor_window(*, entry: ModelPlanEntry) -> bool:
    """Return whether a cursor delete would have no window, before any staging work."""

    return (
        entry.incremental_strategy == IncrementalStrategy.DELETE_INSERT
        and entry.cursor_column is not None
        and entry.cursor_bounds is None
    )


def _context_with_runtime_cursor(
    *, context: ModelMaterializationContext, preparation: _DeltaPreparation
) -> ModelMaterializationContext:
    if preparation.runtime_cursor_bounds is None:
        return context
    return replace(
        context,
        entry=replace(context.entry, cursor_bounds=preparation.runtime_cursor_bounds),
    )


def _cursor_safety_warnings(bounds: CursorBounds | None) -> list[str]:
    return [
        warning
        for warning in (future_cursor_cap_warning(bounds), maximum_start_cap_warning(bounds))
        if warning is not None
    ]


def _prepare_incremental_delta_types(
    *,
    adapter: BaseAdapter,
    connection: Any,
    entry: ModelPlanEntry,
    inputs: _DeltaTypeInputs,
    statement_recorder: StatementRecorder,
) -> tuple[tuple[ColumnInfo, ...], tuple[str, ...]]:
    """Apply declared types and preserve undeclared target types before schema comparison."""

    authoritative_columns: tuple[ColumnInfo, ...] = (
        inputs.declared_columns if entry.type_enforcement else ()
    )
    authoritative_names: frozenset[str] = frozenset(
        column_name_key(column.name) for column in authoritative_columns
    )
    delta_map: dict[str, ColumnInfo] = {
        column_name_key(column.name): column for column in inputs.delta_columns
    }
    effective_policy: OnSchemaChange = entry.on_schema_change or _DEFAULT_ON_SCHEMA_CHANGE
    preserved_columns: list[tuple[ColumnInfo, ColumnInfo]] = []
    desired_columns: list[ColumnInfo] = list(authoritative_columns)
    if effective_policy in (OnSchemaChange.APPEND_NEW_COLUMNS, OnSchemaChange.IGNORE):
        target_column: ColumnInfo
        for target_column in inputs.target_columns:
            incoming_column: ColumnInfo | None = delta_map.get(column_name_key(target_column.name))
            if (
                incoming_column is not None
                and column_name_key(target_column.name) not in authoritative_names
                and not types_equal(
                    left=target_column.type,
                    right=incoming_column.type,
                    dialect=adapter.sql_analysis_dialect_name,
                )
            ):
                desired_columns.append(target_column)
                preserved_columns.append((incoming_column, target_column))

    if desired_columns:
        enforce_types_staged(
            adapter=adapter,
            connection=connection,
            staging_qualified=inputs.delta_qualified,
            staging_database=inputs.target_database,
            staging_schema=inputs.target_schema,
            staging_table=inputs.delta_table,
            declared_columns=tuple(desired_columns),
            table_type=TableType.TRANSIENT,
            statement_recorder=statement_recorder,
        )

    prepared_columns: tuple[ColumnInfo, ...] = inspect_runtime_relation_schema(
        adapter=adapter,
        connection=connection,
        database=inputs.target_database,
        schema=inputs.target_schema,
        name=inputs.delta_table,
    )
    warnings: tuple[str, ...] = tuple(
        f"model '{entry.name}' preserved existing target type for "
        f"'{inputs.target_qualified}.{target.name}': incoming {incoming.type} -> "
        f"target {target.type} (on_schema_change {effective_policy.value})"
        for incoming, target in preserved_columns
    )
    return prepared_columns, warnings


def _widenable_columns(
    *, on_schema_change: OnSchemaChange, incoming_columns: tuple[ColumnInfo, ...]
) -> tuple[ColumnInfo, ...]:
    """The incoming columns target nullability may widen to, under a policy that changes schema."""

    if on_schema_change in _SCHEMA_CHANGING_POLICIES:
        return incoming_columns
    return ()


def _apply_schema_change(
    *,
    adapter: BaseAdapter,
    connection: Any,
    target_qualified: str,
    target_columns: tuple[ColumnInfo, ...],
    delta_columns: tuple[ColumnInfo, ...],
    on_schema_change: OnSchemaChange,
    statement_recorder: StatementRecorder,
    bound_views: BoundViewGuard | None = None,
) -> tuple[str, ...]:
    """Inspect runtime schema diff, apply on_schema_change policy, return new warnings."""

    guard: BoundViewGuard = bound_views or BoundViewGuard()

    target_map: dict[str, str] = {column_name_key(col.name): col.type for col in target_columns}
    delta_map: dict[str, str] = {column_name_key(col.name): col.type for col in delta_columns}

    added: list[ColumnInfo] = []
    removed: list[str] = []
    type_changed: list[ColumnInfo] = []

    col: ColumnInfo
    for col in delta_columns:
        col_lower: str = column_name_key(col.name)
        if col_lower not in target_map:
            added.append(col)
        elif not types_equal(
            left=target_map[col_lower], right=col.type, dialect=adapter.sql_analysis_dialect_name
        ):
            type_changed.append(col)

    for col in target_columns:
        col_lower = column_name_key(col.name)
        if col_lower not in delta_map:
            removed.append(col.name)

    has_diff: bool = bool(added or removed or type_changed)

    if not has_diff:
        return ()

    if on_schema_change == OnSchemaChange.FAIL:
        diff_parts: list[str] = []
        if added:
            diff_parts.append(f"added columns: {', '.join(c.name for c in added)}")
        if removed:
            diff_parts.append(f"removed columns: {', '.join(removed)}")
        if type_changed:
            diff_parts.append(f"type changes: {', '.join(c.name for c in type_changed)}")
        raise ExecutorInputError(
            f"schema change detected and on_schema_change is set to fail: {'; '.join(diff_parts)}"
        )

    if on_schema_change == OnSchemaChange.IGNORE:
        ignored_warnings: list[str] = []
        if added:
            ignored_warnings.append(
                f"schema change ignored: new columns in delta not added to target: "
                f"{', '.join(c.name for c in added)}"
            )
        if removed:
            ignored_warnings.append(
                f"schema change ignored: target columns not in delta: {', '.join(removed)}"
            )
        if type_changed:
            ignored_warnings.append(
                f"schema change ignored: type changes detected: "
                f"{', '.join(c.name for c in type_changed)}"
            )
        return tuple(ignored_warnings)

    if on_schema_change == OnSchemaChange.APPEND_NEW_COLUMNS:
        if type_changed:
            raise ExecutorInputError(
                f"append_new_columns does not support type changes: "
                f"{', '.join(c.name for c in type_changed)}"
            )
        if not added:
            return ()
        with OperationLifecycle(
            operation_kind="warehouse",
            operation_name="schema_synchronization",
            attributes=OperationAttributes(
                phase="apply",
                strategy="append_new_columns",
                adapter=canonicalize_operation_adapter(adapter.adapter_name),
                target_kind="relation",
            ),
        ) as lifecycle:
            adapter.add_columns(
                connection=connection,
                destination=target_qualified,
                columns=tuple(added),
                statement_recorder=statement_recorder,
            )
            lifecycle.completed(metadata={"changed_count": len(added), "added_count": len(added)})
        return ()

    if on_schema_change == OnSchemaChange.SYNC_ALL_COLUMNS:
        with (
            OperationLifecycle(
                operation_kind="warehouse",
                operation_name="schema_synchronization",
                attributes=OperationAttributes(
                    phase="apply",
                    strategy="sync_all_columns",
                    adapter=canonicalize_operation_adapter(adapter.adapter_name),
                    target_kind="relation",
                ),
            ) as lifecycle,
            guard.transaction(),
        ):
            if removed or type_changed:
                _ = guard.release()
            if added:
                adapter.add_columns(
                    connection=connection,
                    destination=target_qualified,
                    columns=tuple(added),
                    statement_recorder=statement_recorder,
                )
            if removed:
                adapter.drop_columns(
                    connection=connection,
                    destination=target_qualified,
                    column_names=tuple(removed),
                    statement_recorder=statement_recorder,
                )
            if type_changed:
                adapter.alter_column_types(
                    connection=connection,
                    destination=target_qualified,
                    columns=tuple(type_changed),
                    statement_recorder=statement_recorder,
                )
            _ = guard.rebind()
            lifecycle.completed(
                metadata={
                    "changed_count": len(added) + len(removed) + len(type_changed),
                    "added_count": len(added),
                    "removed_count": len(removed),
                    "altered_count": len(type_changed),
                }
            )
        return ()

    return ()


def _execute_dml(
    *,
    adapter: BaseAdapter,
    connection: Any,
    target_qualified: str,
    delta_qualified: str,
    target_columns: tuple[ColumnInfo, ...],
    delta_columns: tuple[ColumnInfo, ...],
    entry: ModelPlanEntry,
    cursor_start: str | None = None,
    cursor_end: str | None = None,
    statement_recorder: StatementRecorder,
) -> int | None:
    """Execute the incremental DML strategy from delta into target."""

    strategy: str | None = entry.incremental_strategy
    unique_key: tuple[str, ...] = entry.unique_key

    target_col_set: frozenset[str] = frozenset(column_name_key(col.name) for col in target_columns)
    intersection_names: tuple[str, ...] = tuple(
        col.name for col in delta_columns if column_name_key(col.name) in target_col_set
    )
    dml_columns: tuple[str, ...] = intersection_names
    projection: str = ", ".join(intersection_names)
    dml_sql: str = f"SELECT {projection} FROM {delta_qualified}"

    if strategy == IncrementalStrategy.APPEND:
        incremental_adapter: BaseAdapter = adapter
        return incremental_adapter.append(
            connection=connection,
            destination=target_qualified,
            sql=dml_sql,
            columns=dml_columns,
            statement_recorder=statement_recorder,
        )

    if strategy == IncrementalStrategy.DELETE_INSERT:
        cursor_column: str | None = entry.cursor_column
        if cursor_column is not None:
            if cursor_start is None or cursor_end is None:
                raise cursor_window_unavailable_error(entry=entry)
            return adapter.delete_insert_cursor(
                connection=connection,
                destination=target_qualified,
                sql=dml_sql,
                cursor_column=cursor_column,
                cursor_start=cursor_start,
                cursor_end=cursor_end,
                columns=dml_columns,
                statement_recorder=statement_recorder,
                cursor_type=entry.cursor_type,
            )
        return adapter.delete_insert(
            connection=connection,
            destination=target_qualified,
            sql=dml_sql,
            unique_key=unique_key,
            columns=dml_columns,
            statement_recorder=statement_recorder,
        )

    if strategy == IncrementalStrategy.MERGE:
        merge_projection: str = ", ".join(intersection_names)
        merge_sql: str = f"SELECT {merge_projection} FROM {delta_qualified}"
        return adapter.merge(
            connection=connection,
            destination=target_qualified,
            sql=merge_sql,
            unique_key=unique_key,
            statement_recorder=statement_recorder,
            exclude_columns=entry.merge_exclude_columns,
        )

    raise ExecutorInputError(f"unsupported incremental strategy: {strategy}")
