"""Snapshot model execution lifecycle."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import (
    SNAPSHOT_DEFAULT_VALID_FROM_COLUMN,
    SNAPSHOT_DEFAULT_VALID_TO_COLUMN,
    HistoricalInput,
    InitialValidFrom,
    ModelPlanEntry,
    PlanReason,
    SnapshotSchemaChangePolicy,
    SnapshotsConfig,
    SnapshotStrategy,
    fit_auxiliary_relation_name,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    NormalizedType,
    SnapshotChangeTarget,
)
from sqlbuild.adapter.contract.types import TypeFamily
from sqlbuild.adapter.relations.main.cached_relation_exists import cached_relation_exists
from sqlbuild.adapter.relations.main.resolve_qualified_name_parts import (
    resolve_qualified_name_parts,
)
from sqlbuild.adapter.relations.main.resolve_relation_location_qualified_name import (
    resolve_relation_location_qualified_name,
)
from sqlbuild.adapter.type_system.main.normalize_type import normalize_type
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
from sqlbuild.executor.run._helpers.execution.table_config import model_output_config
from sqlbuild.executor.run._helpers.materializations.full_refresh import (
    promote_full_refresh_rebuild,
    relation_exists,
    resolve_full_refresh_relations,
)
from sqlbuild.executor.run._helpers.reuse.fingerprinting import try_write_fingerprint
from sqlbuild.executor.run._helpers.validation.configured_columns import require_quoted_spelling
from sqlbuild.executor.run._helpers.validation.contracts import validate_runtime_contract
from sqlbuild.executor.run.constants import (
    SNAPSHOT_ALL_CHECK_COLUMNS,
    SNAPSHOT_DELTA_RELATION_SUFFIX,
)
from sqlbuild.executor.run.models import (
    FinalAuditRun,
    FullRefreshRelations,
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

_SCHEMA_CHANGE_STRICTNESS: dict[SnapshotSchemaChangePolicy, int] = {
    SnapshotSchemaChangePolicy.APPEND_NEW_COLUMNS: 0,
    SnapshotSchemaChangePolicy.REQUIRE_CONFIRMATION: 1,
    SnapshotSchemaChangePolicy.DENY: 2,
}


def execute_snapshot_entry(  # noqa: PLR0915
    *,
    context: ModelMaterializationContext,
    snapshots: SnapshotsConfig | None = None,
    allow_snapshot_schema_change: bool = False,
) -> ModelExecutionResult:
    """Execute one current-state snapshot model."""

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
        suffix=SNAPSHOT_DELTA_RELATION_SUFFIX,
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
    full_refresh_relations: FullRefreshRelations | None = _resolve_snapshot_full_refresh_relations(
        context=context
    )

    try:
        _validate_supported_snapshot(entry)
        if not context.schema_prepared:
            adapter.ensure_schema(
                connection=connection,
                database=target_database,
                schema=target_schema,
                statement_recorder=statement_recorder,
            )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.PRE_HOOK,
            error=exc,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    pre_hook_exit: ModelExecutionResult | None = run_pre_hook_phase(
        context=context,
        warnings=warnings,
        audit_results=audit_results,
        hook_results=hook_results,
        statement_recorder=statement_recorder,
    )
    if pre_hook_exit is not None:
        return pre_hook_exit

    if full_refresh_relations is not None:
        reconciliation: ModelExecutionResult | bool = _reconcile_snapshot_full_refresh_result(
            context=context,
            relations=full_refresh_relations,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )
        reconciliation_exit: ModelExecutionResult | None = _snapshot_reconciliation_exit(
            result=reconciliation,
            entry=entry,
            target_qualified=target_qualified,
            warnings=warnings,
            hook_results=hook_results,
            statement_recorder=statement_recorder,
        )
        if reconciliation_exit is not None:
            return reconciliation_exit

    try:
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
                adapter.drop(
                    connection=connection,
                    destination=delta_qualified,
                    if_exists=True,
                    statement_recorder=statement_recorder,
                )
                adapter.create_table_as(
                    connection=connection,
                    destination=delta_qualified,
                    sql=entry.resolved_sql,
                    config=model_output_config(entry),
                    statement_recorder=statement_recorder,
                )
                lifecycle.completed(metadata={"changed_count": 1})
            delta_columns: tuple[ColumnInfo, ...] = inspect_runtime_relation_schema(
                adapter=adapter,
                connection=connection,
                database=target_database,
                schema=target_schema,
                name=delta_table,
            )
            check_columns: tuple[str, ...] = _expanded_check_columns(
                entry=entry, delta_columns=delta_columns
            )
            _validate_delta_columns(
                entry=entry, delta_columns=delta_columns, check_columns=check_columns
            )
            _validate_unique_snapshot_keys(
                adapter=adapter,
                connection=connection,
                entry=entry,
                delta_qualified=delta_qualified,
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
        )

    try:
        with diagnostics_context(sqlbuild_phase="contract", sqlbuild_action_name="validate_delta"):
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

    try:
        target_exists: bool = _apply_snapshot_phase(
            context=context,
            snapshots=snapshots or SnapshotsConfig(),
            allow_snapshot_schema_change=allow_snapshot_schema_change,
            target_qualified=target_qualified,
            delta_qualified=delta_qualified,
            delta_columns=delta_columns,
            check_columns=check_columns,
            full_refresh_relations=full_refresh_relations,
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

    final_audit_run: FinalAuditRun = run_final_scope_audits(
        context=context,
        relation_override=(
            None if full_refresh_relations is None else full_refresh_relations.rebuild_qualified
        ),
    )
    audit_results.extend(final_audit_run.results)

    if final_audit_run.has_error:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.AUDIT,
            error=(
                f"final audit for '{entry.name}' failed before full-refresh promotion "
                if full_refresh_relations is not None
                else f"final audit for '{entry.name}' failed after target update "
            )
            + ("with severity level: error"),
            staging_relation=delta_qualified,
            promoted_relation=target_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )

    if full_refresh_relations is not None:
        promotion_failure: ModelExecutionResult | None = _snapshot_promotion_failure(
            context=context,
            relations=full_refresh_relations,
            target_exists=target_exists,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )
        if promotion_failure is not None:
            return promotion_failure

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
            adapter.drop(
                connection=connection,
                destination=delta_qualified,
                if_exists=True,
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
        adapter.drop(
            connection=connection,
            destination=delta_qualified,
            if_exists=True,
            statement_recorder=statement_recorder,
        )

    return ModelExecutionResult(
        model_name=entry.name,
        status=ExecutionStatus.SUCCESS,
        promoted_relation=target_qualified,
        audit_results=tuple(audit_results),
        warning_messages=tuple(warnings),
        lifecycle_events=statement_recorder.snapshot(),
        hook_results=tuple(hook_results),
    )


def _reconcile_snapshot_full_refresh(
    *,
    adapter: BaseAdapter,
    connection: Any,
    database: str | None,
    schema: str | None,
    relations: FullRefreshRelations,
    statement_recorder: StatementRecorder,
) -> bool:
    """Finish a target-absent rename before beginning a new replacement build."""

    target_exists: bool = relation_exists(
        adapter=adapter,
        connection=connection,
        database=database,
        schema=schema,
        name=relations.target_name,
    )
    rebuild_exists: bool = relation_exists(
        adapter=adapter,
        connection=connection,
        database=database,
        schema=schema,
        name=relations.rebuild_name,
    )
    if not target_exists and rebuild_exists:
        promote_full_refresh_rebuild(
            adapter=adapter,
            connection=connection,
            relations=relations,
            target_exists=False,
            statement_recorder=statement_recorder,
        )
        return True
    return False


def _apply_snapshot_phase(
    *,
    context: ModelMaterializationContext,
    snapshots: SnapshotsConfig,
    allow_snapshot_schema_change: bool,
    target_qualified: str,
    delta_qualified: str,
    delta_columns: tuple[ColumnInfo, ...],
    check_columns: tuple[str, ...],
    full_refresh_relations: FullRefreshRelations | None,
    statement_recorder: StatementRecorder,
) -> bool:
    """Apply snapshot DML to a rebuild or live destination."""

    entry: ModelPlanEntry = context.entry
    with diagnostics_context(sqlbuild_phase="dml", sqlbuild_action_name="apply_snapshot"):
        if full_refresh_relations is not None:
            context.adapter.drop(
                connection=context.connection,
                destination=full_refresh_relations.rebuild_qualified,
                if_exists=True,
                statement_recorder=statement_recorder,
            )
            _create_initial_snapshot_target(
                adapter=context.adapter,
                connection=context.connection,
                entry=entry,
                target_qualified=full_refresh_relations.rebuild_qualified,
                delta_qualified=delta_qualified,
                delta_columns=delta_columns,
                check_columns=check_columns,
                statement_recorder=statement_recorder,
            )
        target_exists: bool = cached_relation_exists(
            adapter=context.adapter,
            connection=context.connection,
            database=entry.destination.database,
            schema=entry.destination.schema,
            name=entry.destination.name,
        )
        if full_refresh_relations is None and not target_exists:
            _create_initial_snapshot_target(
                adapter=context.adapter,
                connection=context.connection,
                entry=entry,
                target_qualified=target_qualified,
                delta_qualified=delta_qualified,
                delta_columns=delta_columns,
                check_columns=check_columns,
                statement_recorder=statement_recorder,
            )
        elif full_refresh_relations is None:
            target_columns: tuple[ColumnInfo, ...] = inspect_runtime_relation_schema(
                adapter=context.adapter,
                connection=context.connection,
                database=entry.destination.database,
                schema=entry.destination.schema,
                name=entry.destination.name,
            )
            _apply_snapshot_schema_change(
                adapter=context.adapter,
                connection=context.connection,
                entry=entry,
                snapshots=snapshots,
                target_qualified=target_qualified,
                target_columns=target_columns,
                delta_columns=delta_columns,
                allow_snapshot_schema_change=allow_snapshot_schema_change,
                statement_recorder=statement_recorder,
            )
            _apply_snapshot_changes(
                adapter=context.adapter,
                connection=context.connection,
                entry=entry,
                target_qualified=target_qualified,
                delta_qualified=delta_qualified,
                delta_columns=delta_columns,
                check_columns=check_columns,
                statement_recorder=statement_recorder,
            )
    return target_exists


def _reconcile_snapshot_full_refresh_result(
    *,
    context: ModelMaterializationContext,
    relations: FullRefreshRelations,
    warnings: list[str],
    audit_results: list[AuditExecutionResult],
    statement_recorder: StatementRecorder,
) -> ModelExecutionResult | bool:
    """Reconcile a snapshot rename window or return its execution failure."""

    try:
        return _reconcile_snapshot_full_refresh(
            adapter=context.adapter,
            connection=context.connection,
            database=context.entry.destination.database,
            schema=context.entry.destination.schema,
            relations=relations,
            statement_recorder=statement_recorder,
        )
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.DML,
            error=exc,
            error_prefix="failed to reconcile snapshot full-refresh rebuild",
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )


def _resolve_snapshot_full_refresh_relations(
    *, context: ModelMaterializationContext
) -> FullRefreshRelations | None:
    """Resolve build-aside names only for a snapshot full refresh."""

    if context.entry.reason != PlanReason.FULL_REFRESH:
        return None
    return resolve_full_refresh_relations(
        adapter=context.adapter,
        database=context.entry.destination.database,
        schema=context.entry.destination.schema,
        target_name=context.entry.destination.name,
    )


def _snapshot_promotion_failure(
    *,
    context: ModelMaterializationContext,
    relations: FullRefreshRelations,
    target_exists: bool,
    warnings: list[str],
    audit_results: list[AuditExecutionResult],
    statement_recorder: StatementRecorder,
) -> ModelExecutionResult | None:
    """Promote a snapshot rebuild or return its execution failure."""

    try:
        promote_full_refresh_rebuild(
            adapter=context.adapter,
            connection=context.connection,
            relations=relations,
            target_exists=target_exists,
            statement_recorder=statement_recorder,
            bound_views=context.bound_views,
        )
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.DML,
            error=exc,
            error_prefix="failed to promote snapshot full-refresh rebuild",
            staging_relation=relations.rebuild_qualified,
            promoted_relation=relations.target_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )
    return None


def _snapshot_reconciliation_exit(
    *,
    result: ModelExecutionResult | bool,
    entry: ModelPlanEntry,
    target_qualified: str,
    warnings: list[str],
    hook_results: list[HookExecutionResult],
    statement_recorder: StatementRecorder,
) -> ModelExecutionResult | None:
    """Return the terminal result for snapshot rename recovery when applicable."""

    if isinstance(result, ModelExecutionResult):
        return result
    if not result:
        return None
    return ModelExecutionResult(
        model_name=entry.name,
        status=ExecutionStatus.SUCCESS,
        promoted_relation=target_qualified,
        warning_messages=tuple(warnings),
        lifecycle_events=statement_recorder.snapshot(),
        hook_results=tuple(hook_results),
    )


def _validate_supported_snapshot(entry: ModelPlanEntry) -> None:
    if entry.snapshot_strategy not in (SnapshotStrategy.TIMESTAMP, SnapshotStrategy.CHECK):
        raise ExecutorInputError(
            "snapshot execution currently supports snapshot_strategy=timestamp or check"
        )
    if (
        entry.observed_at_column is not None
        and entry.invalidate_hard_deletes
        and entry.historical_input == HistoricalInput.CHANGES
    ):
        raise ExecutorInputError("snapshot execution does not support historical hard deletes yet")
    if entry.snapshot_strategy == SnapshotStrategy.TIMESTAMP and entry.updated_at_column is None:
        raise ExecutorInputError("timestamp snapshot execution requires updated_at")
    if entry.snapshot_strategy == SnapshotStrategy.CHECK and not entry.check_columns:
        raise ExecutorInputError("check snapshot execution requires check_columns")
    if not entry.unique_key:
        raise ExecutorInputError("snapshot execution requires unique_key")


def _validate_delta_columns(
    *,
    entry: ModelPlanEntry,
    delta_columns: tuple[ColumnInfo, ...],
    check_columns: tuple[str, ...],
) -> None:
    column_names: frozenset[str] = frozenset(
        column_name_key(column.name) for column in delta_columns
    )
    required_columns: tuple[str, ...]
    if entry.snapshot_strategy == SnapshotStrategy.TIMESTAMP:
        required_columns = (*entry.unique_key, _require_updated_at(entry))
        if entry.observed_at_column is not None:
            required_columns = (*required_columns, entry.observed_at_column)
    elif entry.snapshot_strategy == SnapshotStrategy.CHECK:
        required_columns = (*entry.unique_key, *check_columns)
        if entry.observed_at_column is not None:
            required_columns = (*required_columns, entry.observed_at_column)
    else:
        required_columns = entry.unique_key
    if entry.initial_valid_from == InitialValidFrom.UPDATED_AT:
        required_columns = (*required_columns, _require_updated_at(entry))
    if (
        entry.initial_valid_from == InitialValidFrom.OBSERVED_AT
        and entry.observed_at_column is not None
    ):
        required_columns = (*required_columns, entry.observed_at_column)
    missing_columns: list[str] = [
        column for column in required_columns if column_name_key(column) not in column_names
    ]
    if missing_columns:
        raise ExecutorInputError(
            f"snapshot model '{entry.name}' query output is missing required columns: "
            f"{', '.join(missing_columns)}"
        )
    setting: str
    configured: tuple[str | None, ...]
    for setting, configured in (
        ("unique_key", entry.unique_key),
        ("updated_at", (entry.updated_at_column,)),
        ("observed_at", (entry.observed_at_column,)),
        ("check_columns", check_columns),
    ):
        require_quoted_spelling(
            model_name=entry.name,
            setting=setting,
            configured=tuple(
                name for name in configured if name is not None and name in required_columns
            ),
            listed_columns=delta_columns,
        )
    valid_from_column: str = _valid_from_column(entry)
    valid_to_column: str = _valid_to_column(entry)
    generated_collisions: list[str] = [
        column
        for column in (valid_from_column, valid_to_column)
        if column_name_key(column) in column_names
    ]
    if generated_collisions:
        raise ExecutorInputError(
            f"snapshot model '{entry.name}' query output includes generated validity columns: "
            f"{', '.join(generated_collisions)}"
        )


def _validate_unique_snapshot_keys(
    *,
    adapter: BaseAdapter,
    connection: Any,
    entry: ModelPlanEntry,
    delta_qualified: str,
) -> None:
    identity_columns: tuple[str, ...] = entry.unique_key
    if (
        entry.snapshot_strategy == SnapshotStrategy.TIMESTAMP
        and entry.observed_at_column is not None
        and entry.historical_input == HistoricalInput.CHANGES
    ):
        identity_columns = (*identity_columns, _require_updated_at(entry))
    elif entry.observed_at_column is not None:
        identity_columns = (*identity_columns, entry.observed_at_column)
    key_list: str = ", ".join(identity_columns)
    duplicate_sql: str = (
        f"SELECT COUNT(*) FROM (SELECT {key_list} FROM {delta_qualified} "
        f"GROUP BY {key_list} HAVING COUNT(*) > 1) AS __snapshot_duplicate_keys"
    )
    cursor: Any = adapter.execute(connection=connection, sql=duplicate_sql)
    row: tuple[Any, ...] | None = cursor.fetchone()
    if row is not None and int(row[0]) > 0:
        identity_label: str = (
            "snapshot identity" if entry.observed_at_column is not None else "unique_key"
        )
        raise ExecutorInputError(
            f"snapshot model '{entry.name}' source query returned multiple rows for the same "
            f"{identity_label} ({key_list})"
        )


def _create_initial_snapshot_target(
    *,
    adapter: BaseAdapter,
    connection: Any,
    entry: ModelPlanEntry,
    target_qualified: str,
    delta_qualified: str,
    delta_columns: tuple[ColumnInfo, ...],
    check_columns: tuple[str, ...],
    statement_recorder: StatementRecorder,
) -> None:
    valid_from_column: str = _valid_from_column(entry)
    valid_to_column: str = _valid_to_column(entry)
    if (
        entry.snapshot_strategy == SnapshotStrategy.TIMESTAMP
        and entry.observed_at_column is not None
    ):
        updated_at_column: str = _require_updated_at(entry)
        output_columns: tuple[str, ...] = tuple(column.name for column in delta_columns)
        if entry.historical_input == HistoricalInput.CHANGES:
            statements: tuple[str, ...] = (
                adapter.render_create_initial_historical_timestamp_changes_destination(
                    table_type=entry.table_type,
                    destination=target_qualified,
                    origin=delta_qualified,
                    unique_key=entry.unique_key,
                    updated_at_column=updated_at_column,
                    valid_from_column=valid_from_column,
                    valid_to_column=valid_to_column,
                    output_columns=output_columns,
                )
            )
        else:
            statements = adapter.render_create_initial_historical_timestamp_snapshot_destination(
                table_type=entry.table_type,
                destination=target_qualified,
                origin=delta_qualified,
                unique_key=entry.unique_key,
                updated_at_column=updated_at_column,
                observed_at_column=entry.observed_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                output_columns=output_columns,
                invalidate_hard_deletes=entry.invalidate_hard_deletes,
            )
    elif entry.snapshot_strategy == SnapshotStrategy.CHECK and entry.observed_at_column is not None:
        output_columns: tuple[str, ...] = tuple(column.name for column in delta_columns)
        statements = adapter.render_create_initial_historical_check_snapshot_destination(
            table_type=entry.table_type,
            destination=target_qualified,
            origin=delta_qualified,
            unique_key=entry.unique_key,
            check_columns=check_columns,
            observed_at_column=entry.observed_at_column,
            valid_from_column=valid_from_column,
            valid_to_column=valid_to_column,
            output_columns=output_columns,
            invalidate_hard_deletes=entry.invalidate_hard_deletes,
        )
    else:
        statements = adapter.render_create_initial_snapshot_destination(
            table_type=entry.table_type,
            destination=target_qualified,
            origin=delta_qualified,
            snapshot_strategy=entry.snapshot_strategy,
            updated_at_column=entry.updated_at_column,
            observed_at_column=entry.observed_at_column,
            valid_from_column=valid_from_column,
            valid_to_column=valid_to_column,
            initial_valid_from=entry.initial_valid_from,
        )
    statement_recorder.record_many(statements)
    statement: str
    for statement in statements:
        adapter.execute(connection=connection, sql=statement)


def _apply_snapshot_changes(
    *,
    adapter: BaseAdapter,
    connection: Any,
    entry: ModelPlanEntry,
    target_qualified: str,
    delta_qualified: str,
    delta_columns: tuple[ColumnInfo, ...],
    check_columns: tuple[str, ...],
    statement_recorder: StatementRecorder,
) -> None:
    if entry.snapshot_strategy == SnapshotStrategy.TIMESTAMP:
        _apply_timestamp_snapshot_changes(
            adapter=adapter,
            connection=connection,
            entry=entry,
            target_qualified=target_qualified,
            delta_qualified=delta_qualified,
            delta_columns=delta_columns,
            statement_recorder=statement_recorder,
        )
        return
    if entry.snapshot_strategy == SnapshotStrategy.CHECK:
        _apply_check_snapshot_changes(
            adapter=adapter,
            connection=connection,
            entry=entry,
            target_qualified=target_qualified,
            delta_qualified=delta_qualified,
            delta_columns=delta_columns,
            check_columns=check_columns,
            statement_recorder=statement_recorder,
        )
        return
    raise ExecutorInputError(f"unsupported snapshot strategy: {entry.snapshot_strategy}")


def _apply_timestamp_snapshot_changes(
    *,
    adapter: BaseAdapter,
    connection: Any,
    entry: ModelPlanEntry,
    target_qualified: str,
    delta_qualified: str,
    delta_columns: tuple[ColumnInfo, ...],
    statement_recorder: StatementRecorder,
) -> None:
    updated_at_column: str = _require_updated_at(entry)
    valid_from_column: str = _valid_from_column(entry)
    valid_to_column: str = _valid_to_column(entry)
    output_columns: tuple[str, ...] = tuple(column.name for column in delta_columns)
    if entry.observed_at_column is not None:
        statements: tuple[str, ...]
        if entry.historical_input == HistoricalInput.CHANGES:
            statements = adapter.render_apply_historical_timestamp_changes(
                destination=target_qualified,
                origin=delta_qualified,
                unique_key=entry.unique_key,
                updated_at_column=updated_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                output_columns=output_columns,
            )
        else:
            statements = adapter.render_apply_historical_timestamp_snapshot_changes(
                destination=target_qualified,
                origin=delta_qualified,
                unique_key=entry.unique_key,
                updated_at_column=updated_at_column,
                observed_at_column=entry.observed_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                output_columns=output_columns,
                invalidate_hard_deletes=entry.invalidate_hard_deletes,
            )
        statement_recorder.record_many(statements)
        with adapter.transaction(connection):
            statement: str
            for statement in statements:
                adapter.execute(connection=connection, sql=statement)
        return
    statements: tuple[str, ...] = adapter.render_apply_timestamp_snapshot_changes(
        destination=target_qualified,
        origin=delta_qualified,
        unique_key=entry.unique_key,
        updated_at_column=updated_at_column,
        observed_at_column=entry.observed_at_column,
        valid_from_column=valid_from_column,
        valid_to_column=valid_to_column,
        initial_valid_from=entry.initial_valid_from,
        output_columns=output_columns,
        invalidate_hard_deletes=entry.invalidate_hard_deletes,
    )
    statement_recorder.record_many(statements)
    with adapter.transaction(connection):
        statement: str
        for statement in statements:
            adapter.execute(connection=connection, sql=statement)


def _apply_check_snapshot_changes(
    *,
    adapter: BaseAdapter,
    connection: Any,
    entry: ModelPlanEntry,
    target_qualified: str,
    delta_qualified: str,
    delta_columns: tuple[ColumnInfo, ...],
    check_columns: tuple[str, ...],
    statement_recorder: StatementRecorder,
) -> None:
    valid_from_column: str = _valid_from_column(entry)
    valid_to_column: str = _valid_to_column(entry)
    output_columns: tuple[str, ...] = tuple(column.name for column in delta_columns)
    if entry.observed_at_column is not None:
        statements: tuple[str, ...] = adapter.render_apply_historical_check_snapshot_changes(
            destination=target_qualified,
            origin=delta_qualified,
            unique_key=entry.unique_key,
            check_columns=check_columns,
            observed_at_column=entry.observed_at_column,
            valid_from_column=valid_from_column,
            valid_to_column=valid_to_column,
            output_columns=output_columns,
            invalidate_hard_deletes=entry.invalidate_hard_deletes,
        )
    else:
        statements = adapter.render_apply_check_snapshot_changes(
            target=SnapshotChangeTarget(
                destination=target_qualified,
                origin=delta_qualified,
                unique_key=entry.unique_key,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                output_columns=output_columns,
            ),
            check_columns=check_columns,
            updated_at_column=entry.updated_at_column,
            observed_at_column=entry.observed_at_column,
            initial_valid_from=entry.initial_valid_from,
            invalidate_hard_deletes=entry.invalidate_hard_deletes,
        )
    statement_recorder.record_many(statements)
    with adapter.transaction(connection):
        statement: str
        for statement in statements:
            adapter.execute(connection=connection, sql=statement)


def _expanded_check_columns(
    *, entry: ModelPlanEntry, delta_columns: tuple[ColumnInfo, ...]
) -> tuple[str, ...]:
    if (
        entry.snapshot_strategy != SnapshotStrategy.CHECK
        or entry.check_columns != SNAPSHOT_ALL_CHECK_COLUMNS
    ):
        return entry.check_columns

    excluded_names: set[str] = {column_name_key(column) for column in entry.unique_key}
    excluded_names.add(column_name_key(_valid_from_column(entry)))
    excluded_names.add(column_name_key(_valid_to_column(entry)))
    if entry.observed_at_column is not None:
        excluded_names.add(column_name_key(entry.observed_at_column))
    if entry.updated_at_column is not None:
        excluded_names.add(column_name_key(entry.updated_at_column))

    expanded_columns: tuple[str, ...] = tuple(
        column.name
        for column in delta_columns
        if column_name_key(column.name) not in excluded_names
    )
    if not expanded_columns:
        raise ExecutorInputError(
            f"snapshot model '{entry.name}' check_columns [*] did not match any data columns"
        )
    return expanded_columns


def _apply_snapshot_schema_change(
    *,
    adapter: BaseAdapter,
    connection: Any,
    entry: ModelPlanEntry,
    snapshots: SnapshotsConfig,
    target_qualified: str,
    target_columns: tuple[ColumnInfo, ...],
    delta_columns: tuple[ColumnInfo, ...],
    allow_snapshot_schema_change: bool,
    statement_recorder: StatementRecorder,
) -> None:
    target_map: dict[str, ColumnInfo] = {column.name.lower(): column for column in target_columns}

    added: tuple[ColumnInfo, ...] = tuple(
        column for column in delta_columns if column.name.lower() not in target_map
    )
    type_changed: tuple[str, ...] = tuple(
        column.name
        for column in delta_columns
        if column.name.lower() in target_map
        and not _snapshot_types_compatible(
            target_type=target_map[column.name.lower()].type,
            delta_type=column.type,
            dialect=adapter.sql_analysis_dialect(),
        )
    )

    if type_changed:
        raise ExecutorInputError(
            f"snapshot schema change does not support type changes: {', '.join(type_changed)}"
        )
    if not added:
        return

    policy: SnapshotSchemaChangePolicy = _effective_snapshot_schema_change_policy(
        entry=entry, snapshots=snapshots
    )
    added_names: str = ", ".join(column.name for column in added)
    if policy == SnapshotSchemaChangePolicy.DENY:
        raise ExecutorInputError(
            f"snapshot model '{entry.name}' detected new output columns: {added_names}; "
            "snapshot_schema_change is set to deny"
        )
    if (
        policy == SnapshotSchemaChangePolicy.REQUIRE_CONFIRMATION
        and not allow_snapshot_schema_change
    ):
        if (
            entry.snapshot_strategy == SnapshotStrategy.CHECK
            and entry.check_columns == SNAPSHOT_ALL_CHECK_COLUMNS
        ):
            raise ExecutorInputError(
                f"snapshot model '{entry.name}' check_columns [*] detected new data columns "
                f"on existing target: {added_names}. These columns would become part of "
                "change detection. Re-run with --allow-snapshot-schema-change to accept, "
                "or use explicit check_columns."
            )
        raise ExecutorInputError(
            f"snapshot model '{entry.name}' detected new output columns: {added_names}; "
            "re-run with --allow-snapshot-schema-change to accept"
        )

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
            columns=added,
            statement_recorder=statement_recorder,
        )
        lifecycle.completed(metadata={"changed_count": len(added), "added_count": len(added)})


def _snapshot_types_compatible(*, target_type: str, delta_type: str, dialect: str | None) -> bool:
    if types_equal(left=target_type, right=delta_type, dialect=dialect):
        return True
    target: NormalizedType = normalize_type(type_sql=target_type, dialect=dialect)
    delta: NormalizedType = normalize_type(type_sql=delta_type, dialect=dialect)
    if target.family != TypeFamily.STRING or delta.family != TypeFamily.STRING:
        return False
    if target.length is None:
        return True
    if delta.length is None:
        return False
    return delta.length <= target.length


def _effective_snapshot_schema_change_policy(
    *, entry: ModelPlanEntry, snapshots: SnapshotsConfig
) -> SnapshotSchemaChangePolicy:
    project_policy: SnapshotSchemaChangePolicy = SnapshotSchemaChangePolicy(
        snapshots.wildcard_check_schema_change
        if entry.snapshot_strategy == SnapshotStrategy.CHECK
        and entry.check_columns == SNAPSHOT_ALL_CHECK_COLUMNS
        else snapshots.schema_change
    )
    if entry.snapshot_schema_change is None:
        return project_policy
    model_policy: SnapshotSchemaChangePolicy = SnapshotSchemaChangePolicy(
        entry.snapshot_schema_change
    )
    return max((project_policy, model_policy), key=lambda policy: _SCHEMA_CHANGE_STRICTNESS[policy])


def _require_updated_at(entry: ModelPlanEntry) -> str:
    if entry.updated_at_column is None:
        raise ExecutorInputError("timestamp snapshot execution requires updated_at")
    return entry.updated_at_column


def _valid_from_column(entry: ModelPlanEntry) -> str:
    return entry.valid_from_column or SNAPSHOT_DEFAULT_VALID_FROM_COLUMN


def _valid_to_column(entry: ModelPlanEntry) -> str:
    return entry.valid_to_column or SNAPSHOT_DEFAULT_VALID_TO_COLUMN
