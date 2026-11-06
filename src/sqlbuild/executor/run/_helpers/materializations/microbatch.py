"""Microbatch incremental execution lifecycle."""

from __future__ import annotations

import hashlib
import logging
import time
from collections.abc import Callable
from dataclasses import dataclass, field, replace
from datetime import UTC, date, datetime
from decimal import Decimal, InvalidOperation
from functools import partial
from typing import Any

from sqlbuild._native import (
    AlignedInterval,
    BackfillAction,
    CursorBounds,
    CursorGrain,
    CursorType,
    DateValue,
    Duration,
    IncrementalStrategy,
    IntegerValue,
    MicrobatchLimitAction,
    MicrobatchStrategy,
    ModelPlanEntry,
    OnSchemaChange,
    TableType,
    TimestampValue,
    aligned_timestamp_bounds,
    compute_query_hash,
    cursor_bound_display,
    deterministic_microbatch_event_id,
    empty_cursor_window,
    fit_auxiliary_relation_name,
    future_cursor_cap_warning,
    inclusive_cursor_end,
    latest_active_replay_requirement,
    maximum_start_cap_warning,
    merge_microbatch_intervals,
    microbatch_limit_warning,
    project_microbatch_coverage,
    project_replay_requirement,
    resolve_effective_microbatch_batch_size,
)
from sqlbuild._native import cursor_cap_from_end as cap_from_end
from sqlbuild._native import cursor_cap_from_start as cap_from_start
from sqlbuild._native import cursor_compare as compare
from sqlbuild._native import (
    cursor_exclusive_end_from_observed_max as exclusive_end_from_observed_max,
)
from sqlbuild._native import cursor_floor_to_grain as floor_to_grain
from sqlbuild._native import cursor_next_boundary as next_boundary
from sqlbuild._native import cursor_parse as parse
from sqlbuild._native import cursor_render as render
from sqlbuild._native import cursor_sentinel_to_token as sentinel_to_token
from sqlbuild._native import cursor_split_into_batches as split_into_batches
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.relations.main.cached_relation_exists import cached_relation_exists
from sqlbuild.adapter.relations.main.resolve_qualified_name_parts import (
    resolve_qualified_name_parts,
)
from sqlbuild.adapter.relations.main.resolve_relation_location_qualified_name import (
    resolve_relation_location_qualified_name,
)
from sqlbuild.cursor_algebra.types import Bound, BoundSentinel, CursorScalar
from sqlbuild.diagnostics.main.diagnostics_context import diagnostics_context
from sqlbuild.diagnostics.main.log_debug_event import log_debug_event
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
from sqlbuild.executor.run._helpers.materializations.full_refresh import (
    promote_full_refresh_rebuild,
    relation_exists,
    resolve_full_refresh_relations,
)
from sqlbuild.executor.run._helpers.materializations.incremental import (
    _apply_schema_change,
    _execute_dml,
    _widenable_columns,
)
from sqlbuild.executor.run._helpers.reuse.fingerprinting import try_write_fingerprint
from sqlbuild.executor.run._helpers.validation.configured_columns import require_quoted_spelling
from sqlbuild.executor.run._helpers.validation.cursor_bounds import (
    build_runtime_cursor_spec,
    empty_input_rebuild_error,
    has_authoritative_cursor_override,
    has_runtime_owned_cursor_watermarks,
    resolve_effective_timestamp_grain,
    resolve_runtime_cursor_bounds,
    substitute_cursor_sentinels,
)
from sqlbuild.executor.run._helpers.validation.type_enforcement import enforce_types_staged
from sqlbuild.executor.run.constants import DELTA_RELATION_SUFFIX
from sqlbuild.executor.run.exceptions import EmptyCursorInputsError
from sqlbuild.executor.run.models import (
    BatchWindow,
    FinalAuditRun,
    FullRefreshRelations,
    MicrobatchAccountingInterval,
    MicrobatchBatchProgress,
    MicrobatchLifecycleState,
    MicrobatchPhaseOutcome,
    MicrobatchSchemaPhaseOutcome,
    MicrobatchTargets,
    ModelExecutionResult,
    ModelMaterializationContext,
    PostHookPhaseOutcome,
)
from sqlbuild.executor.run.types import ExecutionPhase, MicrobatchBatchRunner
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.microbatches.classes.direct_store import (
    DirectMicrobatchEventStore,
    direct_microbatch_scope,
)
from sqlbuild.microbatches.constants import (
    DIRECT_MICROBATCH_SCOPE_KIND,
    MICROBATCH_GENERATION_WILDCARD,
    MICROBATCH_REPLAY_GENERATION_PREFIX,
)
from sqlbuild.microbatches.exceptions import MicrobatchStateError
from sqlbuild.microbatches.models import (
    MicrobatchCoverageProjection,
    MicrobatchEvent,
    MicrobatchInterval,
    MicrobatchScope,
    MicrobatchWriteResult,
    ProjectedMicrobatchInterval,
    ReplayRequirementProjection,
)
from sqlbuild.microbatches.types import (
    MicrobatchCompletionType,
    MicrobatchEventStore,
    MicrobatchFingerprintStatus,
    MicrobatchRecordType,
    MicrobatchRunType,
    ReplayRequirementState,
    UnaccountedPartitionPolicy,
)
from sqlbuild.runtime.observability.classes.microbatch_lifecycle import (
    MicrobatchLifecycle,
)
from sqlbuild.runtime.observability.classes.operation_lifecycle import (
    OperationAttributes,
    OperationLifecycle,
)
from sqlbuild.runtime.observability.main.canonicalize_operation_adapter import (
    canonicalize_operation_adapter,
)
from sqlbuild.runtime.observability.models import MicrobatchLifecycleContext

_DEFAULT_ON_SCHEMA_CHANGE: OnSchemaChange = OnSchemaChange.APPEND_NEW_COLUMNS
_DEBUG_LOGGER: logging.Logger = logging.getLogger("sqlbuild.execution")
_UNACCOUNTED_COUNT_CHUNK_SIZE: int = 100


@dataclass(frozen=True)
class _MicrobatchPlan:
    """Planned batch windows or the early-exit result when none can run."""

    batches: tuple[BatchWindow, ...] = ()
    effective_batch_size: str | None = None
    resolved_range: CursorBounds | None = None
    early_exit: ModelExecutionResult | None = None
    runtime_discovery: bool = False
    resolved_intervals: tuple[MicrobatchInterval, ...] = ()
    empty_cursor_inputs: tuple[str, ...] = ()
    waiting_on_empty_inputs: bool = False


@dataclass(frozen=True)
class _MicrobatchHistoryContext:
    store: MicrobatchEventStore
    scope: MicrobatchScope
    history: tuple[MicrobatchEvent, ...]
    run_type: MicrobatchRunType
    run_start: CursorScalar
    run_end: CursorScalar
    batch_size: str
    replay_requirement_id: str | None = None
    origin_run_id: str | None = None
    origin_run_started_at: datetime | None = None
    execution_run_started_at: datetime | None = None
    recovery_intervals: frozenset[tuple[str, str]] = frozenset()
    known_missing_intervals: tuple[MicrobatchInterval, ...] = ()
    unaccounted_intervals: tuple[MicrobatchInterval, ...] = ()
    synthetic_intervals: tuple[MicrobatchInterval, ...] = ()
    accounting_intervals: tuple[MicrobatchAccountingInterval, ...] = ()
    replay_requirement_state: ReplayRequirementState | None = None
    replay_unknown_fingerprint_count: int = 0
    contiguous_frontier: str | None = None
    unaccounted_partition_policy: str = "synthesize"
    required_model_version_hash: str | None = None
    concurrent_enabled: bool = False
    batch_concurrency: int = 1
    global_concurrency: int = 1
    recovery_origins: dict[tuple[str, str], _RecoveryOrigin] = field(default_factory=dict)
    pending_events: tuple[MicrobatchEvent, ...] = ()
    resolution_reason: str = "normal"


@dataclass(frozen=True)
class _RecoveryOrigin:
    origin_run_id: str
    origin_run_started_at: datetime | None
    run_start: CursorScalar
    run_end: CursorScalar
    run_type: MicrobatchRunType
    replay_requirement_id: str | None


@dataclass(frozen=True)
class _PreparedMicrobatchExecution:
    targets: MicrobatchTargets
    full_refresh_relations: FullRefreshRelations | None
    state: MicrobatchLifecycleState
    batch_plan: _MicrobatchPlan
    history: _MicrobatchHistoryContext


@dataclass(frozen=True)
class _MicrobatchLifecyclePlan:
    """Stable planned context repeated on each active microbatch fact."""

    batch_count: int
    cursor_start: str
    cursor_end_exclusive: str


class _BatchColumns:
    """The delta columns of a model's first batch; later batches stage the same query's output."""

    def __init__(self) -> None:
        self._delta: tuple[ColumnInfo, ...] | None = None

    def delta(self, read: Callable[[], tuple[ColumnInfo, ...]]) -> tuple[ColumnInfo, ...]:
        """The first batch's delta columns, read once."""

        if self._delta is None:
            self._delta = read()
        return self._delta


@dataclass(frozen=True)
class _MicrobatchBatchPreparation:
    """Prepared batch state or the failure that prevents target DML."""

    state: MicrobatchLifecycleState
    schema_checked: bool
    failure: ModelExecutionResult | None = None


class _SerialMicrobatchEventStore:
    """Reject accidental durable-state use from the serial execution path."""

    def write(self, event: MicrobatchEvent) -> None:
        raise MicrobatchStateError("serial microbatch execution does not use durable event state")

    def write_many(self, events: tuple[MicrobatchEvent, ...]) -> MicrobatchWriteResult:
        raise MicrobatchStateError("serial microbatch execution does not use durable event state")

    def read_scope_history(self, scope: MicrobatchScope) -> tuple[MicrobatchEvent, ...]:
        raise MicrobatchStateError("serial microbatch execution does not use durable event state")

    def read_model_history(self, scope: MicrobatchScope) -> tuple[MicrobatchEvent, ...]:
        raise MicrobatchStateError("serial microbatch execution does not use durable event state")


def _prepare_microbatch_execution(
    *,
    context: ModelMaterializationContext,
    is_full_refresh: bool,
    on_progress: Callable[[str], None] | None,
) -> _PreparedMicrobatchExecution | ModelExecutionResult:
    targets: MicrobatchTargets = _resolve_microbatch_targets(context=context)
    full_refresh_relations: FullRefreshRelations | None = None
    if is_full_refresh:
        full_refresh_relations = resolve_full_refresh_relations(
            adapter=context.adapter,
            database=context.entry.destination.database,
            schema=context.entry.destination.schema,
            target_name=context.entry.destination.name,
        )
        targets = replace(
            targets,
            target_table=full_refresh_relations.rebuild_name,
            target_qualified=full_refresh_relations.rebuild_qualified,
        )
    state: MicrobatchLifecycleState = MicrobatchLifecycleState(
        warnings=[],
        audit_results=[],
        hook_results=[],
        statement_recorder=StatementRecorder(),
    )
    batch_plan: _MicrobatchPlan = _plan_microbatch_windows(
        context=context,
        is_full_refresh=is_full_refresh,
        target_qualified=targets.target_qualified,
        warnings=state.warnings,
        audit_results=state.audit_results,
        statement_recorder=state.statement_recorder,
        on_progress=on_progress,
    )
    if batch_plan.early_exit is not None:
        return batch_plan.early_exit
    state = _with_future_cursor_warning(state=state, bounds=batch_plan.resolved_range)
    history_read: (
        tuple[
            MicrobatchEventStore,
            MicrobatchScope,
            tuple[MicrobatchEvent, ...],
            tuple[MicrobatchEvent, ...],
        ]
        | ModelExecutionResult
    )
    if context.microbatch_event_store is None:
        history_read = _serial_microbatch_history(context=context, batch_plan=batch_plan)
    else:
        if on_progress is not None:
            on_progress("microbatch state reconciliation: reading history")
        with diagnostics_context(
            sqlbuild_phase="microbatch_state", sqlbuild_action_name="read_history"
        ):
            history_read = _read_microbatch_history(
                context=context, state=state, is_full_refresh=is_full_refresh
            )
    if isinstance(history_read, ModelExecutionResult):
        return history_read
    event_store, event_scope, event_history, model_history = history_read
    history: _MicrobatchHistoryContext | ModelExecutionResult = (
        _serial_microbatch_context(
            context=context,
            batch_plan=batch_plan,
            store=event_store,
            scope=event_scope,
        )
        if context.microbatch_event_store is None
        else _prepare_microbatch_history(
            context=context,
            state=state,
            batch_plan=batch_plan,
            store=event_store,
            scope=event_scope,
            history=event_history,
            transition_history=model_history,
        )
    )
    if isinstance(history, ModelExecutionResult):
        return history
    if history.run_type == MicrobatchRunType.REPLAY_ON_CHANGE:
        replay_batches: tuple[BatchWindow, ...] = compute_batch_windows(
            start=history.run_start,
            end=history.run_end,
            batch_size=history.batch_size,
            cursor_type=context.entry.cursor_type or "",
        )
        batch_plan = replace(
            batch_plan,
            batches=replay_batches,
            resolved_range=CursorBounds(start=history.run_start, end=history.run_end),
        )
    return _PreparedMicrobatchExecution(
        targets=targets,
        full_refresh_relations=full_refresh_relations,
        state=state,
        batch_plan=batch_plan,
        history=history,
    )


def execute_microbatch_entry(
    *,
    context: ModelMaterializationContext,
    declared_columns: tuple[ColumnInfo, ...],
    is_full_refresh: bool = False,
    on_progress: Callable[[str], None] | None = None,
) -> ModelExecutionResult:
    """Execute one microbatch incremental model through batched delta/DML."""

    if context.entry.batch_concurrency <= 1:
        context = replace(
            context,
            microbatch_event_store=None,
            microbatch_event_store_resolver=None,
            microbatch_scope=None,
            microbatch_model_version_hash=None,
            microbatch_lease_check=None,
        )
    prepared: _PreparedMicrobatchExecution | ModelExecutionResult = _prepare_microbatch_execution(
        context=context,
        is_full_refresh=is_full_refresh,
        on_progress=on_progress,
    )
    if isinstance(prepared, ModelExecutionResult):
        return prepared
    targets: MicrobatchTargets = prepared.targets
    full_refresh_relations: FullRefreshRelations | None = prepared.full_refresh_relations
    state: MicrobatchLifecycleState = prepared.state
    batch_plan: _MicrobatchPlan = prepared.batch_plan
    history_context: _MicrobatchHistoryContext = prepared.history
    reconciliation: (
        tuple[MicrobatchLifecycleState, _MicrobatchHistoryContext, tuple[BatchWindow, ...]]
        | ModelExecutionResult
    )
    reconciliation = _run_microbatch_reconciliation(
        context=context,
        state=state,
        targets=targets,
        history=history_context,
        normal_batches=batch_plan.batches,
        is_full_refresh=is_full_refresh,
        on_progress=on_progress,
    )
    if isinstance(reconciliation, ModelExecutionResult):
        return reconciliation
    state, history_context, reconciled_batches = reconciliation
    resolved_intervals: tuple[MicrobatchInterval, ...] = merge_microbatch_intervals(
        intervals=tuple(
            MicrobatchInterval(start=render(value=batch.start), end=render(value=batch.end))
            for batch in reconciled_batches
        ),
        cursor_type=context.entry.cursor_type or "",
    )
    resolved_range: CursorBounds | None = (
        CursorBounds(start=resolved_intervals[0].start, end=resolved_intervals[-1].end)
        if resolved_intervals
        else batch_plan.resolved_range
    )
    batch_plan = replace(
        batch_plan,
        batches=reconciled_batches,
        resolved_intervals=resolved_intervals,
        resolved_range=resolved_range,
    )
    resolution_reason: str = _runtime_resolution_reason(
        context=context, history=history_context, batches=reconciled_batches
    )
    history_context = replace(history_context, resolution_reason=resolution_reason)
    state = _with_reconciliation_warnings(context=context, state=state, history=history_context)
    if not batch_plan.batches and not batch_plan.empty_cursor_inputs:
        state = replace(state, warnings=[*state.warnings, "no batches to process"])
    state, safety_failure = _enforce_microbatch_safety_limit(
        context=context, state=state, batch_plan=batch_plan
    )
    if safety_failure is not None:
        return safety_failure
    context, state, batch_plan, limit_failure = _enforce_microbatch_limit(
        context=context, state=state, batch_plan=batch_plan
    )
    if limit_failure is not None:
        return replace(
            limit_failure,
            **_microbatch_result_fields(history=history_context, succeeded=False),
        )
    if (
        on_progress is not None
        and batch_plan.resolved_range is not None
        and batch_plan.effective_batch_size is not None
    ):
        on_progress(
            _format_resolved_microbatch_progress(
                bounds=batch_plan.resolved_range,
                batch_count=len(batch_plan.batches),
                batch_size=batch_plan.effective_batch_size,
                cursor_type=context.entry.cursor_type,
                cursor_grain=context.entry.cursor_grain,
                strategy=context.entry.microbatch_strategy or "unknown",
                reason=resolution_reason,
                limit=context.entry.microbatch_limit,
                intervals=resolved_intervals,
            )
        )
    publication_failure: ModelExecutionResult | None = _publish_pending_reconciliation_events(
        context=context, state=state, history=history_context
    )
    if publication_failure is not None:
        return replace(
            publication_failure,
            **_microbatch_result_fields(history=history_context, succeeded=False),
        )
    no_work_result: ModelExecutionResult | None = _no_work_microbatch_result(
        context=context,
        state=state,
        history=history_context,
        batch_plan=batch_plan,
        is_full_refresh=is_full_refresh,
    )
    if no_work_result is not None:
        return no_work_result
    pre_hook_exit: ModelExecutionResult | None = run_pre_hook_phase(
        context=context,
        warnings=state.warnings,
        audit_results=state.audit_results,
        hook_results=state.hook_results,
        statement_recorder=state.statement_recorder,
    )
    if pre_hook_exit is not None:
        return replace(
            pre_hook_exit,
            **_microbatch_result_fields(history=history_context, succeeded=False),
        )
    if full_refresh_relations is not None:
        preparation_failure: ModelExecutionResult | None = _prepare_full_refresh_rebuild(
            context=context,
            state=state,
            relations=full_refresh_relations,
        )
        if preparation_failure is not None:
            return replace(
                preparation_failure,
                **_microbatch_result_fields(history=history_context, succeeded=False),
            )
    batch_outcome: MicrobatchPhaseOutcome = _execute_microbatch_batches(
        context=context,
        declared_columns=declared_columns,
        is_full_refresh=is_full_refresh,
        batches=batch_plan.batches,
        targets=targets,
        state=state,
        history_context=history_context,
        on_progress=on_progress,
        lifecycle_plan=_microbatch_lifecycle_plan(
            batch_plan=batch_plan,
            cursor_type=context.entry.cursor_type or "",
        ),
    )
    state = batch_outcome.state
    if context.microbatch_event_store is not None:
        history_context = _refresh_microbatch_result_history(
            context=context,
            history=history_context,
            batches=batch_plan.batches,
            physical_target_name=targets.target_table,
        )
    if batch_outcome.failure is not None:
        return replace(
            batch_outcome.failure,
            batch_count=batch_outcome.completed_batches,
            batch_size=batch_plan.effective_batch_size,
            rows_affected=batch_outcome.rows_affected,
            **_microbatch_result_fields(history=history_context, succeeded=False),
        )
    final_audit_run: FinalAuditRun = run_final_scope_audits(
        context=context,
        relation_override=(
            None if full_refresh_relations is None else full_refresh_relations.rebuild_qualified
        ),
    )
    state.audit_results.extend(final_audit_run.results)
    if final_audit_run.has_error:
        return replace(
            build_failed_result(
                entry=context.entry,
                phase=ExecutionPhase.AUDIT,
                error=(
                    f"final audit for '{context.entry.name}' failed before full-refresh promotion "
                    if full_refresh_relations is not None
                    else f"final audit for '{context.entry.name}' failed after target update "
                )
                + ("with severity level: error"),
                promoted_relation=targets.target_qualified,
                warnings=state.warnings,
                audit_results=state.audit_results,
                statement_recorder=state.statement_recorder,
            ),
            **_microbatch_result_fields(history=history_context, succeeded=True),
        )
    if full_refresh_relations is not None:
        promotion_failure: ModelExecutionResult | None = _promote_microbatch_full_refresh(
            context=context,
            state=state,
            relations=full_refresh_relations,
        )
        if promotion_failure is not None:
            return replace(
                promotion_failure,
                **_microbatch_result_fields(history=history_context, succeeded=True),
            )
    post_hook_outcome: PostHookPhaseOutcome = run_post_hook_phase(
        context=context,
        warnings=state.warnings,
        audit_results=state.audit_results,
        hook_results=state.hook_results,
        statement_recorder=state.statement_recorder,
        promoted_relation=targets.target_qualified,
    )
    if post_hook_outcome.failure is not None:
        return replace(
            post_hook_outcome.failure,
            **_microbatch_result_fields(history=history_context, succeeded=True),
        )
    if post_hook_outcome.skipped:
        return replace(
            build_skipped_result(
                entry=context.entry,
                warnings=state.warnings,
                audit_results=state.audit_results,
                statement_recorder=state.statement_recorder,
                hook_results=state.hook_results,
                promoted_relation=targets.target_qualified,
            ),
            **_microbatch_result_fields(history=history_context, succeeded=True),
        )
    state.warnings.extend(
        try_write_fingerprint(
            entry=context.entry,
            adapter=context.adapter,
            connection=context.connection,
            run_id=context.run_id,
            query_change_tracking=context.query_change_tracking,
            fingerprint_writes=context.fingerprint_writes,
            model_audits=context.model_audits,
            audit_results=tuple(state.audit_results),
        )
    )
    resolved_range: CursorBounds | None = batch_plan.resolved_range
    return ModelExecutionResult(
        model_name=context.entry.name,
        status=ExecutionStatus.SUCCESS,
        promoted_relation=targets.target_qualified,
        batch_count=batch_outcome.completed_batches,
        batch_size=batch_plan.effective_batch_size,
        rows_affected=batch_outcome.rows_affected,
        cursor_range_start=(
            None if resolved_range is None else sentinel_to_token(sentinel=resolved_range.start)
        ),
        cursor_range_end=(
            None if resolved_range is None else sentinel_to_token(sentinel=resolved_range.end)
        ),
        cursor_type=context.entry.cursor_type,
        cursor_grain=context.entry.cursor_grain,
        empty_cursor_inputs=batch_plan.empty_cursor_inputs,
        waiting_on_empty_inputs=batch_plan.waiting_on_empty_inputs,
        future_cursor_safety=(resolved_range.future_safety if resolved_range is not None else None),
        maximum_start_safety=(
            resolved_range.maximum_start_safety if resolved_range is not None else None
        ),
        audit_results=tuple(state.audit_results),
        warning_messages=tuple(state.warnings),
        lifecycle_events=state.statement_recorder.snapshot(),
        hook_results=tuple(state.hook_results),
        microbatch_limit=context.entry.microbatch_limit,
        microbatch_limit_count=context.entry.microbatch_limit_count,
        microbatch_limit_action=context.entry.microbatch_limit_action,
        microbatch_limit_warning=context.entry.microbatch_limit_warning,
        microbatch_strategy=context.entry.microbatch_strategy,
        **_microbatch_result_fields(history=history_context, succeeded=True),
    )


def _no_work_microbatch_result(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    history: _MicrobatchHistoryContext,
    batch_plan: _MicrobatchPlan,
    is_full_refresh: bool,
) -> ModelExecutionResult | None:
    if batch_plan.batches:
        return None
    destination_exists: bool = cached_relation_exists(
        adapter=context.adapter,
        connection=context.connection,
        database=context.entry.destination.database,
        schema=context.entry.destination.schema,
        name=context.entry.destination.name,
    )
    if destination_exists and not is_full_refresh:
        return None
    resolved_range: CursorBounds | None = batch_plan.resolved_range
    promoted_relation: str | None = (
        context.entry.destination.qualified_name if destination_exists else None
    )
    return ModelExecutionResult(
        model_name=context.entry.name,
        status=ExecutionStatus.SUCCESS if destination_exists else ExecutionStatus.SKIPPED,
        promoted_relation=promoted_relation,
        batch_count=0,
        batch_size=batch_plan.effective_batch_size,
        cursor_range_start=(
            None if resolved_range is None else sentinel_to_token(sentinel=resolved_range.start)
        ),
        cursor_range_end=(
            None if resolved_range is None else sentinel_to_token(sentinel=resolved_range.end)
        ),
        cursor_type=context.entry.cursor_type,
        cursor_grain=context.entry.cursor_grain,
        empty_cursor_inputs=batch_plan.empty_cursor_inputs,
        waiting_on_empty_inputs=batch_plan.waiting_on_empty_inputs,
        warning_messages=tuple(state.warnings),
        lifecycle_events=state.statement_recorder.snapshot(),
        skip_reason=(
            None
            if destination_exists
            else "no microbatch work and destination relation does not exist"
        ),
        microbatch_limit=context.entry.microbatch_limit,
        microbatch_limit_count=context.entry.microbatch_limit_count,
        microbatch_limit_action=context.entry.microbatch_limit_action,
        microbatch_limit_warning=context.entry.microbatch_limit_warning,
        microbatch_strategy=context.entry.microbatch_strategy,
        **_microbatch_result_fields(history=history, succeeded=True),
    )


def _enforce_microbatch_safety_limit(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    batch_plan: _MicrobatchPlan,
) -> tuple[MicrobatchLifecycleState, ModelExecutionResult | None]:
    max_batches: int | None = context.entry.microbatch_safety_limit
    action: MicrobatchLimitAction | None = context.entry.microbatch_safety_limit_action
    warning: str | None = (
        None
        if action is None
        else microbatch_limit_warning(
            model_name=context.entry.name,
            max_batches=max_batches,
            batch_count=len(batch_plan.batches),
            action=action,
        )
    )
    if warning is None:
        return state, None
    if action == MicrobatchLimitAction.WARN:
        return replace(state, warnings=[*state.warnings, warning]), None
    safety_entry: ModelPlanEntry = replace(
        context.entry,
        microbatch_limit=max_batches,
        microbatch_limit_count=len(batch_plan.batches),
        microbatch_limit_action=action,
        microbatch_limit_warning=warning,
    )
    safety_context: ModelMaterializationContext = replace(context, entry=safety_entry)
    failure: ModelExecutionResult = build_failed_result(
        entry=safety_entry,
        phase=ExecutionPhase.STAGING,
        error=warning,
        warnings=state.warnings,
        audit_results=state.audit_results,
        statement_recorder=state.statement_recorder,
    )
    return state, replace(
        failure,
        microbatch_run_type=_microbatch_run_type(context=safety_context).value,
    )


def _enforce_microbatch_limit(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    batch_plan: _MicrobatchPlan,
) -> tuple[
    ModelMaterializationContext,
    MicrobatchLifecycleState,
    _MicrobatchPlan,
    ModelExecutionResult | None,
]:
    limit_count: int = len(batch_plan.batches)
    action: MicrobatchLimitAction | None = context.entry.microbatch_limit_action
    warning: str | None = (
        None
        if action is None
        else microbatch_limit_warning(
            model_name=context.entry.name,
            max_batches=context.entry.microbatch_limit,
            batch_count=limit_count,
            action=action,
        )
    )
    limited_plan: _MicrobatchPlan = batch_plan
    if (
        warning is not None
        and context.entry.microbatch_limit is not None
        and action
        in {
            MicrobatchLimitAction.CAP_FROM_END,
            MicrobatchLimitAction.CAP_FROM_START,
        }
    ):
        selected_batches: tuple[BatchWindow, ...] = (
            cap_from_end(intervals=batch_plan.batches, count=context.entry.microbatch_limit)
            if action == MicrobatchLimitAction.CAP_FROM_END
            else cap_from_start(intervals=batch_plan.batches, count=context.entry.microbatch_limit)
        )
        selected_batches = tuple(
            replace(batch, index=index) for index, batch in enumerate(selected_batches)
        )
        selected_intervals: tuple[MicrobatchInterval, ...] = tuple(
            MicrobatchInterval(start=render(value=batch.start), end=render(value=batch.end))
            for batch in selected_batches
        )
        limited_plan = replace(
            batch_plan,
            batches=selected_batches,
            resolved_intervals=selected_intervals,
            resolved_range=(
                CursorBounds(start=selected_batches[0].start, end=selected_batches[-1].end)
                if selected_batches
                else None
            ),
        )
    effective_warning: str | None = warning or context.entry.microbatch_limit_warning
    effective_count: int = (
        context.entry.microbatch_limit_count
        if context.entry.microbatch_limit_count is not None
        and context.entry.microbatch_limit_warning is not None
        else limit_count
    )
    limited_context: ModelMaterializationContext = replace(
        context,
        entry=replace(
            context.entry,
            microbatch_limit_count=effective_count,
            microbatch_limit_warning=effective_warning,
        ),
    )
    limited_context = _context_with_microbatch_range(
        context=limited_context, batch_plan=limited_plan
    )
    if warning is None:
        return limited_context, state, limited_plan, None
    if action == MicrobatchLimitAction.WARN:
        warned_state: MicrobatchLifecycleState = replace(state, warnings=[*state.warnings, warning])
        return limited_context, warned_state, limited_plan, None
    if action in {
        MicrobatchLimitAction.CAP_FROM_END,
        MicrobatchLimitAction.CAP_FROM_START,
    }:
        warned_state = replace(state, warnings=[*state.warnings, warning])
        return limited_context, warned_state, limited_plan, None
    failure: ModelExecutionResult = build_failed_result(
        entry=limited_context.entry,
        phase=ExecutionPhase.STAGING,
        error=warning,
        warnings=state.warnings,
        audit_results=state.audit_results,
        statement_recorder=state.statement_recorder,
    )
    return (
        limited_context,
        state,
        limited_plan,
        replace(
            failure,
            microbatch_run_type=_microbatch_run_type(context=limited_context).value,
        ),
    )


def _with_future_cursor_warning(
    *, state: MicrobatchLifecycleState, bounds: CursorBounds | None
) -> MicrobatchLifecycleState:
    warnings: list[str] = [
        warning
        for warning in (future_cursor_cap_warning(bounds), maximum_start_cap_warning(bounds))
        if warning is not None
    ]
    if not warnings:
        return state
    return replace(state, warnings=[*state.warnings, *warnings])


def _run_microbatch_reconciliation(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    targets: MicrobatchTargets,
    history: _MicrobatchHistoryContext,
    normal_batches: tuple[BatchWindow, ...],
    is_full_refresh: bool,
    on_progress: Callable[[str], None] | None,
) -> (
    tuple[MicrobatchLifecycleState, _MicrobatchHistoryContext, tuple[BatchWindow, ...]]
    | ModelExecutionResult
):
    if context.microbatch_event_store is None:
        return state, history, normal_batches
    started: float = time.monotonic()
    with diagnostics_context(sqlbuild_phase="microbatch_state", sqlbuild_action_name="reconcile"):
        reconciliation: (
            tuple[MicrobatchLifecycleState, _MicrobatchHistoryContext, tuple[BatchWindow, ...]]
            | ModelExecutionResult
        ) = _reconcile_microbatch_history(
            context=context,
            state=state,
            targets=targets,
            history=history,
            normal_batches=normal_batches,
            is_full_refresh=is_full_refresh,
        )
    if isinstance(reconciliation, ModelExecutionResult):
        return reconciliation
    state, reconciled_history, batches = reconciliation
    duration: float = time.monotonic() - started
    covered_through: str = reconciled_history.contiguous_frontier or "none"
    if on_progress is not None:
        on_progress(
            "microbatch state reconciliation: "
            f"{len(reconciled_history.history)} events, "
            f"covered through {covered_through}, {duration:.1f}s"
        )
    log_debug_event(
        logger=_DEBUG_LOGGER,
        message="",
        sqlbuild_subject="model",
        sqlbuild_name=context.entry.name,
        sqlbuild_event="reconciliation_complete",
        sqlbuild_phase="microbatch_state",
        sqlbuild_event_count=len(reconciled_history.history),
        sqlbuild_synthetic_count=len(reconciled_history.synthetic_intervals),
        sqlbuild_covered_through=covered_through,
        sqlbuild_duration_ms=int(duration * 1000),
    )
    return state, reconciled_history, batches


def _with_reconciliation_warnings(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    history: _MicrobatchHistoryContext,
) -> MicrobatchLifecycleState:
    warnings: list[str] = []
    if history.synthetic_intervals:
        warnings.append(
            f"microbatch history did not account for "
            f"{len(history.synthetic_intervals)} intervals in '{context.entry.name}'; "
            "SQLBuild accepted synthetic physical coverage, but their model fingerprints "
            "are unknown"
        )
    if history.replay_requirement_id is not None and history.replay_unknown_fingerprint_count:
        warnings.append(
            f"replay-on-change for '{context.entry.name}' has "
            f"{history.replay_unknown_fingerprint_count} intervals with "
            "synthetic coverage and unknown fingerprints"
        )
    elif history.replay_unknown_fingerprint_count and not history.synthetic_intervals:
        warnings.append(
            f"microbatch history for '{context.entry.name}' retains "
            f"{history.replay_unknown_fingerprint_count} intervals with "
            "synthetic coverage and unknown fingerprints"
        )
    return replace(state, warnings=[*state.warnings, *warnings])


def _runtime_resolution_reason(
    *,
    context: ModelMaterializationContext,
    history: _MicrobatchHistoryContext,
    batches: tuple[BatchWindow, ...],
) -> str:
    if history.run_type == MicrobatchRunType.REPLAY_ON_CHANGE:
        return "definition recovery"
    if history.recovery_intervals:
        return "partition recovery"
    if batches and context.entry.lookback is not None:
        return "normal lookback"
    return "normal"


def _publish_pending_reconciliation_events(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    history: _MicrobatchHistoryContext,
) -> ModelExecutionResult | None:
    if not history.pending_events:
        return None
    try:
        result: MicrobatchWriteResult = history.store.write_many(history.pending_events)
        log_debug_event(
            logger=_DEBUG_LOGGER,
            message="",
            sqlbuild_subject="model",
            sqlbuild_name=context.entry.name,
            sqlbuild_event="reconciliation_events_published",
            sqlbuild_phase="microbatch_state",
            sqlbuild_event_count=result.total,
            sqlbuild_inserted_count=result.inserted,
            sqlbuild_already_existing_count=result.already_existing,
        )
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.MICROBATCH_STATE,
            error=exc,
            error_prefix="failed to append microbatch reconciliation state",
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    return None


def _resolve_microbatch_targets(*, context: ModelMaterializationContext) -> MicrobatchTargets:
    entry: ModelPlanEntry = context.entry
    target_table: str = entry.destination.name
    delta_table: str = fit_auxiliary_relation_name(
        base_name=target_table,
        suffix=DELTA_RELATION_SUFFIX,
        identifier_limit=context.adapter.maximum_identifier_length(),
    )
    return MicrobatchTargets(
        target_database=entry.destination.database,
        target_schema=entry.destination.schema,
        target_table=target_table,
        target_qualified=resolve_relation_location_qualified_name(
            adapter=context.adapter,
            location=entry.destination,
        ),
        delta_table=delta_table,
        delta_qualified=resolve_qualified_name_parts(
            adapter=context.adapter,
            database=entry.destination.database,
            schema=entry.destination.schema,
            name=delta_table,
        ),
    )


def _execute_microbatch_batches(
    *,
    context: ModelMaterializationContext,
    declared_columns: tuple[ColumnInfo, ...],
    is_full_refresh: bool,
    batches: tuple[BatchWindow, ...],
    targets: MicrobatchTargets,
    state: MicrobatchLifecycleState,
    history_context: _MicrobatchHistoryContext,
    on_progress: Callable[[str], None] | None = None,
    lifecycle_plan: _MicrobatchLifecyclePlan | None = None,
) -> MicrobatchPhaseOutcome:
    plan: _MicrobatchLifecyclePlan = lifecycle_plan or _MicrobatchLifecyclePlan(
        batch_count=len(batches),
        cursor_start=render(value=batches[0].start if batches else history_context.run_start),
        cursor_end_exclusive=render(value=batches[-1].end if batches else history_context.run_end),
    )
    if context.microbatch_batch_runner is not None and not is_full_refresh and len(batches) > 1:
        return _execute_microbatch_batches_concurrently(
            context=context,
            declared_columns=declared_columns,
            batches=batches,
            targets=targets,
            state=state,
            history_context=history_context,
            on_progress=on_progress,
            lifecycle_plan=plan,
        )
    schema_checked: bool = False
    completed_batches: int = 0
    total_rows: int = 0
    row_count_known: bool = False
    known_columns: _BatchColumns = _BatchColumns()
    batch: BatchWindow
    for batch in batches:
        lifecycle: MicrobatchLifecycle = _microbatch_lifecycle(
            context=context,
            batch=batch,
            history=history_context,
            plan=plan,
        )
        lifecycle.start()
        lease_failure: ModelExecutionResult | None = _microbatch_lease_failure(
            context=context,
            state=state,
            batch=batch,
            boundary="batch staging",
        )
        if lease_failure is not None:
            lifecycle.failed(error_code=lease_failure.error_code)
            return MicrobatchPhaseOutcome(
                state=state,
                failure=lease_failure,
                completed_batches=completed_batches,
            )
        batch_targets: MicrobatchTargets = _targets_for_batch(
            context=context, targets=targets, batch=batch
        )
        batch_start_time: float = time.monotonic()
        window_text: str = f"{render(value=batch.start)}..{render(value=batch.end)}"
        display_window: str = _format_batch_window_for_display(batch=batch, entry=context.entry)
        if on_progress is not None:
            on_progress(f"batch {batch.index + 1}/{plan.batch_count} {display_window}")
        preparation: _MicrobatchBatchPreparation = _prepare_microbatch_batch(
            context=context,
            declared_columns=declared_columns,
            is_full_refresh=is_full_refresh,
            batch=batch,
            schema_checked=schema_checked,
            window_text=window_text,
            targets=batch_targets,
            state=state,
        )
        state = preparation.state
        if preparation.failure is not None:
            lifecycle.failed(error_code=preparation.failure.error_code)
            return MicrobatchPhaseOutcome(
                state=state,
                failure=preparation.failure,
                completed_batches=completed_batches,
            )
        schema_checked = preparation.schema_checked
        lease_failure = _microbatch_lease_failure(
            context=context,
            state=state,
            batch=batch,
            boundary="target DML",
        )
        if lease_failure is not None:
            lifecycle.failed(error_code=lease_failure.error_code)
            return MicrobatchPhaseOutcome(
                state=state,
                failure=lease_failure,
                completed_batches=completed_batches,
            )
        dml_result: ModelExecutionResult | int | None = _apply_microbatch_dml(
            context=context,
            batch=batch,
            completed_batches=completed_batches,
            is_full_refresh=is_full_refresh,
            window_text=window_text,
            targets=batch_targets,
            state=state,
            known_columns=known_columns,
        )
        if isinstance(dml_result, ModelExecutionResult):
            lifecycle.failed(error_code=dml_result.error_code)
            return MicrobatchPhaseOutcome(
                state=state,
                failure=dml_result,
                completed_batches=completed_batches,
                rows_affected=_reported_rows_affected(
                    total_rows=total_rows,
                    row_count_known=row_count_known,
                ),
            )
        if isinstance(dml_result, int):
            total_rows += dml_result
            row_count_known = True
        lease_failure = _microbatch_lease_failure(
            context=context,
            state=state,
            batch=batch,
            boundary="completion publication",
        )
        if lease_failure is not None:
            lifecycle.failed(error_code=lease_failure.error_code)
            return MicrobatchPhaseOutcome(
                state=state,
                failure=lease_failure,
                completed_batches=completed_batches,
                rows_affected=_reported_rows_affected(
                    total_rows=total_rows,
                    row_count_known=row_count_known,
                ),
            )
        completion_failure: ModelExecutionResult | None = (
            None
            if context.microbatch_event_store is None
            else _record_microbatch_completion(
                context=context,
                batch=batch,
                is_full_refresh=is_full_refresh,
                rows_affected=dml_result if isinstance(dml_result, int) else None,
                history=history_context,
                state=state,
                targets=batch_targets,
            )
        )
        if completion_failure is not None:
            lifecycle.failed(error_code=completion_failure.error_code)
            return MicrobatchPhaseOutcome(
                state=state,
                failure=completion_failure,
                completed_batches=completed_batches,
                rows_affected=_reported_rows_affected(
                    total_rows=total_rows, row_count_known=row_count_known
                ),
            )
        try:
            _complete_microbatch_batch(
                context=context,
                window_text=window_text,
                targets=batch_targets,
                state=state,
                replaced_next=batch is not batches[-1]
                and batch_targets.delta_qualified == targets.delta_qualified,
            )
        except BaseException as error:
            lifecycle.failed(error=error)
            raise
        lifecycle.completed(affected_rows=dml_result if isinstance(dml_result, int) else None)
        completed_batches += 1
        batch_elapsed: float = time.monotonic() - batch_start_time
        if on_progress is not None:
            on_progress(
                f"batch {batch.index + 1}/{plan.batch_count} {display_window} {batch_elapsed:.1f}s"
            )
        if context.on_batch_complete is not None:
            context.on_batch_complete(
                MicrobatchBatchProgress(
                    model_name=context.entry.name,
                    batch_number=batch.index + 1,
                    batch_count=plan.batch_count,
                    window=display_window,
                    rows_affected=dml_result if isinstance(dml_result, int) else None,
                    elapsed_seconds=batch_elapsed,
                )
            )
    return MicrobatchPhaseOutcome(
        state=state,
        completed_batches=completed_batches,
        rows_affected=_reported_rows_affected(
            total_rows=total_rows,
            row_count_known=row_count_known,
        ),
    )


def _microbatch_lifecycle(
    *,
    context: ModelMaterializationContext,
    batch: BatchWindow,
    history: _MicrobatchHistoryContext,
    plan: _MicrobatchLifecyclePlan,
) -> MicrobatchLifecycle:
    start: str = render(value=batch.start)
    end: str = render(value=batch.end)
    return MicrobatchLifecycle(
        context=MicrobatchLifecycleContext(
            cursor_start=start,
            cursor_end_exclusive=end,
            planned_cursor_start=plan.cursor_start,
            planned_cursor_end_exclusive=plan.cursor_end_exclusive,
            batch_index=batch.index + 1,
            batch_count=plan.batch_count,
            configured_batch_size=context.entry.batch_size or history.batch_size,
            effective_batch_size=history.batch_size,
            microbatch_strategy=context.entry.microbatch_strategy or "watermark",
            microbatch_run_type=history.run_type.value,
            batch_kind=("recovery" if (start, end) in history.recovery_intervals else "ordinary"),
        )
    )


def _microbatch_lifecycle_plan(
    *, batch_plan: _MicrobatchPlan, cursor_type: str
) -> _MicrobatchLifecyclePlan:
    selected_intervals: tuple[MicrobatchInterval, ...] = merge_microbatch_intervals(
        intervals=tuple(
            MicrobatchInterval(start=render(value=batch.start), end=render(value=batch.end))
            for batch in batch_plan.batches
        ),
        cursor_type=cursor_type,
    )
    if selected_intervals:
        return _MicrobatchLifecyclePlan(
            batch_count=len(batch_plan.batches),
            cursor_start=selected_intervals[0].start,
            cursor_end_exclusive=selected_intervals[-1].end,
        )
    resolved_range: CursorBounds | None = batch_plan.resolved_range
    if resolved_range is None:
        raise ExecutorInputError("microbatch lifecycle requires resolved cursor bounds")
    return _MicrobatchLifecyclePlan(
        batch_count=len(batch_plan.batches),
        cursor_start=render(value=_concrete_bound(bound=resolved_range.start)),
        cursor_end_exclusive=render(value=_concrete_bound(bound=resolved_range.end)),
    )


def _prepare_microbatch_batch(
    *,
    context: ModelMaterializationContext,
    declared_columns: tuple[ColumnInfo, ...],
    is_full_refresh: bool,
    batch: BatchWindow,
    schema_checked: bool,
    window_text: str,
    targets: MicrobatchTargets,
    state: MicrobatchLifecycleState,
) -> _MicrobatchBatchPreparation:
    stage_failure: ModelExecutionResult | None = _stage_microbatch_delta(
        context=context,
        batch=batch,
        window_text=window_text,
        targets=targets,
        state=state,
    )
    if stage_failure is not None:
        return _MicrobatchBatchPreparation(state, schema_checked, stage_failure)
    schema_outcome: MicrobatchSchemaPhaseOutcome = _apply_microbatch_schema_change(
        context=context,
        is_full_refresh=is_full_refresh,
        schema_checked=schema_checked,
        window_text=window_text,
        targets=targets,
        state=state,
    )
    if schema_outcome.failure is not None:
        return _MicrobatchBatchPreparation(
            schema_outcome.state,
            schema_outcome.schema_checked,
            schema_outcome.failure,
        )
    type_failure: ModelExecutionResult | None = _enforce_microbatch_types(
        context=context,
        declared_columns=declared_columns,
        batch=batch,
        window_text=window_text,
        targets=targets,
        state=schema_outcome.state,
    )
    if type_failure is not None:
        return _MicrobatchBatchPreparation(
            schema_outcome.state,
            schema_outcome.schema_checked,
            type_failure,
        )
    audit_outcome: MicrobatchPhaseOutcome = _run_microbatch_delta_audits(
        context=context,
        batch=batch,
        targets=targets,
        state=schema_outcome.state,
    )
    return _MicrobatchBatchPreparation(
        audit_outcome.state,
        schema_outcome.schema_checked,
        audit_outcome.failure,
    )


def _microbatch_lease_failure(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    batch: BatchWindow,
    boundary: str,
) -> ModelExecutionResult | None:
    if context.microbatch_lease_check is None:
        return None
    try:
        context.microbatch_lease_check()
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.MICROBATCH_STATE,
            error=(
                f"microbatch lease check failed before {boundary} for batch {batch.index}: {exc}"
            ),
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    return None


def _execute_microbatch_batches_concurrently(
    *,
    context: ModelMaterializationContext,
    declared_columns: tuple[ColumnInfo, ...],
    batches: tuple[BatchWindow, ...],
    targets: MicrobatchTargets,
    state: MicrobatchLifecycleState,
    history_context: _MicrobatchHistoryContext,
    on_progress: Callable[[str], None] | None,
    lifecycle_plan: _MicrobatchLifecyclePlan,
) -> MicrobatchPhaseOutcome:
    recovery_batches: tuple[BatchWindow, ...] = tuple(
        batch
        for batch in batches
        if (render(value=batch.start), render(value=batch.end))
        in history_context.recovery_intervals
    )
    ordinary_batches: tuple[BatchWindow, ...] = tuple(
        batch
        for batch in batches
        if (render(value=batch.start), render(value=batch.end))
        not in history_context.recovery_intervals
    )
    completed_batches: int = 0
    total_rows: int = 0
    row_count_known: bool = False
    for phase_batches in (recovery_batches, ordinary_batches):
        if not phase_batches:
            continue
        phase_outcome: MicrobatchPhaseOutcome = _execute_concurrent_microbatch_phase(
            context=context,
            declared_columns=declared_columns,
            batches=phase_batches,
            targets=targets,
            aggregate_state=state,
            history_context=history_context,
            on_progress=on_progress,
            lifecycle_plan=lifecycle_plan,
        )
        state = phase_outcome.state
        completed_batches += phase_outcome.completed_batches
        if phase_outcome.rows_affected is not None:
            total_rows += phase_outcome.rows_affected
            row_count_known = True
        if phase_outcome.failure is not None:
            return MicrobatchPhaseOutcome(
                state=state,
                failure=phase_outcome.failure,
                completed_batches=completed_batches,
                rows_affected=_reported_rows_affected(
                    total_rows=total_rows, row_count_known=row_count_known
                ),
            )
    return MicrobatchPhaseOutcome(
        state=state,
        completed_batches=completed_batches,
        rows_affected=_reported_rows_affected(
            total_rows=total_rows, row_count_known=row_count_known
        ),
    )


def _execute_concurrent_microbatch_phase(
    *,
    context: ModelMaterializationContext,
    declared_columns: tuple[ColumnInfo, ...],
    batches: tuple[BatchWindow, ...],
    targets: MicrobatchTargets,
    aggregate_state: MicrobatchLifecycleState,
    history_context: _MicrobatchHistoryContext,
    on_progress: Callable[[str], None] | None,
    lifecycle_plan: _MicrobatchLifecyclePlan,
) -> MicrobatchPhaseOutcome:
    first: BatchWindow = batches[0]
    serial_context: ModelMaterializationContext = replace(context, microbatch_batch_runner=None)
    first_outcome: MicrobatchPhaseOutcome = _execute_microbatch_batches(
        context=serial_context,
        declared_columns=declared_columns,
        is_full_refresh=False,
        batches=(first,),
        targets=targets,
        state=aggregate_state,
        history_context=history_context,
        on_progress=on_progress,
        lifecycle_plan=lifecycle_plan,
    )
    if first_outcome.failure is not None or len(batches) == 1:
        return first_outcome

    def execute(*, batch: BatchWindow, connection: Any) -> MicrobatchPhaseOutcome:
        worker_state: MicrobatchLifecycleState = MicrobatchLifecycleState(
            warnings=[],
            audit_results=[],
            hook_results=[],
            statement_recorder=StatementRecorder(),
        )
        worker_store: MicrobatchEventStore = (
            context.microbatch_event_store_resolver(connection)
            if context.microbatch_event_store_resolver is not None
            else history_context.store
        )
        return _execute_microbatch_batches(
            context=replace(
                context,
                connection=connection,
                microbatch_batch_runner=None,
                microbatch_event_store=worker_store,
            ),
            declared_columns=declared_columns,
            is_full_refresh=False,
            batches=(batch,),
            targets=targets,
            state=worker_state,
            history_context=replace(history_context, store=worker_store),
            on_progress=on_progress,
            lifecycle_plan=lifecycle_plan,
        )

    runner: MicrobatchBatchRunner | None = context.microbatch_batch_runner
    if runner is None:
        return first_outcome
    outcomes: tuple[MicrobatchPhaseOutcome, ...] = runner(
        batches[1:],
        context.entry.batch_concurrency,
        lambda batch, connection: execute(batch=batch, connection=connection),
    )
    completed_batches: int = first_outcome.completed_batches
    total_rows: int = first_outcome.rows_affected or 0
    row_count_known: bool = first_outcome.rows_affected is not None
    failure: ModelExecutionResult | None = None
    for outcome in outcomes:
        aggregate_state = _merge_microbatch_states(target=aggregate_state, source=outcome.state)
        completed_batches += outcome.completed_batches
        if outcome.rows_affected is not None:
            total_rows += outcome.rows_affected
            row_count_known = True
        if failure is None and outcome.failure is not None:
            failure = outcome.failure
    return MicrobatchPhaseOutcome(
        state=aggregate_state,
        failure=failure,
        completed_batches=completed_batches,
        rows_affected=_reported_rows_affected(
            total_rows=total_rows, row_count_known=row_count_known
        ),
    )


def _merge_microbatch_states(
    *, target: MicrobatchLifecycleState, source: MicrobatchLifecycleState
) -> MicrobatchLifecycleState:
    recorder: StatementRecorder = StatementRecorder()
    recorder.events.extend(target.statement_recorder.snapshot())
    recorder.events.extend(source.statement_recorder.snapshot())
    return MicrobatchLifecycleState(
        warnings=[*target.warnings, *source.warnings],
        audit_results=[*target.audit_results, *source.audit_results],
        hook_results=[*target.hook_results, *source.hook_results],
        statement_recorder=recorder,
    )


def _targets_for_batch(
    *, context: ModelMaterializationContext, targets: MicrobatchTargets, batch: BatchWindow
) -> MicrobatchTargets:
    if context.entry.batch_concurrency <= 1:
        return targets
    identity: str = hashlib.sha256(
        f"{context.run_id}:{batch.index}:{render(value=batch.start)}:"
        f"{render(value=batch.end)}".encode()
    ).hexdigest()[:12]
    delta_table: str = fit_auxiliary_relation_name(
        base_name=targets.target_table,
        suffix=f"{DELTA_RELATION_SUFFIX}_{identity}",
        identifier_limit=context.adapter.maximum_identifier_length(),
    )
    return replace(
        targets,
        delta_table=delta_table,
        delta_qualified=resolve_qualified_name_parts(
            adapter=context.adapter,
            database=targets.target_database,
            schema=targets.target_schema,
            name=delta_table,
        ),
    )


def _read_microbatch_history(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    is_full_refresh: bool,
) -> (
    tuple[
        MicrobatchEventStore,
        MicrobatchScope,
        tuple[MicrobatchEvent, ...],
        tuple[MicrobatchEvent, ...],
    ]
    | ModelExecutionResult
):
    store: MicrobatchEventStore = context.microbatch_event_store or DirectMicrobatchEventStore(
        adapter=context.adapter, connection=context.connection
    )
    scope: MicrobatchScope = context.microbatch_scope or direct_microbatch_scope(
        adapter=context.adapter, connection=context.connection, entry=context.entry
    )
    try:
        all_history: tuple[MicrobatchEvent, ...] = store.read_model_history(scope)
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.MICROBATCH_STATE,
            error=exc,
            error_prefix="failed to read microbatch history",
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    if is_full_refresh and scope.scope_kind == DIRECT_MICROBATCH_SCOPE_KIND:
        if _microbatch_run_type(context=context) == MicrobatchRunType.REPLAY_ON_CHANGE:
            replay_generation: str = (
                MICROBATCH_REPLAY_GENERATION_PREFIX
                + _expected_model_version_hash(context=context)
                + ":"
                + scope.physical_generation_id
            )
            scope = replace(scope, physical_generation_id=replay_generation)
            return (
                store,
                scope,
                tuple(
                    event
                    for event in all_history
                    if event.scope.physical_generation_id == replay_generation
                ),
                all_history,
            )
        scope = replace(scope, physical_generation_id=context.run_id)
        return store, scope, (), all_history
    if scope.physical_generation_id != MICROBATCH_GENERATION_WILDCARD:
        return (
            store,
            scope,
            tuple(
                event
                for event in all_history
                if event.scope.physical_generation_id == scope.physical_generation_id
            ),
            all_history,
        )
    if not all_history:
        scope = replace(scope, physical_generation_id=scope.scope_key)
        return store, scope, (), all_history
    latest_generation: str = max(
        all_history, key=lambda event: (event.created_at, event.event_id)
    ).scope.physical_generation_id
    scope = replace(scope, physical_generation_id=latest_generation)
    history: tuple[MicrobatchEvent, ...] = tuple(
        event for event in all_history if event.scope.physical_generation_id == latest_generation
    )
    return store, scope, history, all_history


def _serial_microbatch_history(
    *, context: ModelMaterializationContext, batch_plan: _MicrobatchPlan
) -> tuple[
    MicrobatchEventStore,
    MicrobatchScope,
    tuple[MicrobatchEvent, ...],
    tuple[MicrobatchEvent, ...],
]:
    if batch_plan.resolved_range is None or batch_plan.effective_batch_size is None:
        raise ExecutorInputError(
            "serial microbatch execution requires resolved bounds and batch size"
        )
    return (
        _SerialMicrobatchEventStore(),
        MicrobatchScope(
            scope_kind="serial",
            scope_key=context.entry.name,
            model_name=context.entry.name,
            target_database=None,
            target_schema=None,
            target_name=context.entry.destination.name,
            physical_generation_id="",
        ),
        (),
        (),
    )


def _serial_microbatch_context(
    *,
    context: ModelMaterializationContext,
    batch_plan: _MicrobatchPlan,
    store: MicrobatchEventStore,
    scope: MicrobatchScope,
) -> _MicrobatchHistoryContext:
    resolved_range: CursorBounds | None = batch_plan.resolved_range
    batch_size: str | None = batch_plan.effective_batch_size
    if resolved_range is None or batch_size is None:
        raise ExecutorInputError(
            "serial microbatch execution requires resolved bounds and batch size"
        )
    return _MicrobatchHistoryContext(
        store=store,
        scope=scope,
        history=(),
        run_type=_microbatch_run_type(context=context),
        run_start=_concrete_bound(bound=resolved_range.start),
        run_end=_concrete_bound(bound=resolved_range.end),
        batch_size=batch_size,
        origin_run_id=context.run_id,
        execution_run_started_at=datetime.now(tz=UTC),
        unaccounted_partition_policy=context.microbatch_unaccounted_partition_policy,
        batch_concurrency=1,
        global_concurrency=context.microbatch_global_concurrency,
    )


def _prepare_microbatch_history(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    batch_plan: _MicrobatchPlan,
    store: MicrobatchEventStore,
    scope: MicrobatchScope,
    history: tuple[MicrobatchEvent, ...],
    transition_history: tuple[MicrobatchEvent, ...],
) -> _MicrobatchHistoryContext | ModelExecutionResult:
    resolved_range: CursorBounds | None = batch_plan.resolved_range
    batch_size: str | None = batch_plan.effective_batch_size
    if resolved_range is None or batch_size is None:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.MICROBATCH_STATE,
            error="microbatch history requires resolved bounds and batch size",
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    run_type: MicrobatchRunType = _microbatch_run_type(context=context)
    expected_version_hash: str = _expected_model_version_hash(context=context)
    execution_run_started_at: datetime = datetime.now(tz=UTC)
    requirement: MicrobatchEvent | None = None
    if run_type == MicrobatchRunType.REPLAY_ON_CHANGE:
        requirement = latest_active_replay_requirement(
            events=tuple(
                event
                for event in history
                if event.previous_model_version_hash == context.entry.previous_version_hash
            ),
            current_model_version_hash=expected_version_hash,
        )
        if requirement is not None and not _replay_requirement_still_active(
            requirement=requirement,
            history=transition_history,
            cursor_type=context.entry.cursor_type or "",
        ):
            requirement = None
        if requirement is None:
            transition_anchor: str = (
                max(
                    transition_history,
                    key=lambda event: (event.created_at, event.event_id),
                ).event_id
                if transition_history
                else ""
            )
            requirement_reason: str = ":".join(
                (
                    expected_version_hash,
                    context.entry.previous_version_hash or "",
                    context.entry.backfill.duration or context.entry.backfill.action.value,
                    transition_anchor,
                )
            )
            requirement_id: str = deterministic_microbatch_event_id(
                scope=scope,
                record_type=MicrobatchRecordType.REPLAY_REQUIREMENT,
                partition_start=sentinel_to_token(sentinel=resolved_range.start),
                partition_end=sentinel_to_token(sentinel=resolved_range.end),
                completion_reason=requirement_reason,
            )
            requirement = MicrobatchEvent(
                event_id=requirement_id,
                record_type=MicrobatchRecordType.REPLAY_REQUIREMENT,
                scope=scope,
                origin_run_id=context.run_id,
                execution_run_id=context.run_id,
                run_type=run_type,
                run_start=sentinel_to_token(sentinel=resolved_range.start),
                run_end=sentinel_to_token(sentinel=resolved_range.end),
                batch_size=batch_size,
                cursor_column=context.entry.cursor_column or "",
                cursor_type=context.entry.cursor_type or "",
                cursor_grain=context.entry.cursor_grain,
                model_version_hash=expected_version_hash,
                definition_hash=_definition_hash(context=context),
                fingerprint_status=MicrobatchFingerprintStatus.KNOWN,
                replay_requirement_id=requirement_id,
                required_model_version_hash=expected_version_hash,
                previous_model_version_hash=context.entry.previous_version_hash,
                replay_policy=(
                    context.entry.backfill.duration
                    if context.entry.backfill.action == BackfillAction.BOUNDED
                    else context.entry.backfill.action.value
                ),
                origin_run_started_at=execution_run_started_at,
                execution_run_started_at=execution_run_started_at,
            )
    effective_history: tuple[MicrobatchEvent, ...] = (
        (*history, requirement)
        if requirement is not None and requirement not in history
        else history
    )
    return _MicrobatchHistoryContext(
        store=store,
        scope=scope,
        history=effective_history,
        run_type=run_type,
        run_start=(
            parse(raw=requirement.run_start, cursor_type=context.entry.cursor_type or "")
            if requirement is not None
            else _concrete_bound(bound=resolved_range.start)
        ),
        run_end=(
            parse(raw=requirement.run_end, cursor_type=context.entry.cursor_type or "")
            if requirement is not None
            else _concrete_bound(bound=resolved_range.end)
        ),
        batch_size=requirement.batch_size if requirement is not None else batch_size,
        replay_requirement_id=(
            requirement.replay_requirement_id if requirement is not None else None
        ),
        origin_run_id=requirement.origin_run_id if requirement is not None else context.run_id,
        origin_run_started_at=(
            requirement.origin_run_started_at
            if requirement is not None
            else execution_run_started_at
        ),
        execution_run_started_at=execution_run_started_at,
        replay_requirement_state=(
            ReplayRequirementState.INCOMPLETE if requirement is not None else None
        ),
        unaccounted_partition_policy=context.microbatch_unaccounted_partition_policy,
        required_model_version_hash=(
            requirement.required_model_version_hash if requirement is not None else None
        ),
        concurrent_enabled=context.microbatch_batch_runner is not None,
        batch_concurrency=context.entry.batch_concurrency,
        global_concurrency=context.microbatch_global_concurrency,
        pending_events=(
            (requirement,) if requirement is not None and requirement not in history else ()
        ),
    )


def _replay_requirement_still_active(
    *, requirement: MicrobatchEvent, history: tuple[MicrobatchEvent, ...], cursor_type: str
) -> bool:
    requirement_order: tuple[float, str] = (
        requirement.created_at.timestamp(),
        requirement.event_id,
    )
    if any(
        event.record_type
        in {
            MicrobatchRecordType.PARTITION_COMPLETION,
            MicrobatchRecordType.REPLAY_REQUIREMENT,
        }
        and (
            event.record_type == MicrobatchRecordType.REPLAY_REQUIREMENT
            or event.model_version_hash
            not in {
                None,
                requirement.required_model_version_hash,
            }
        )
        and (event.created_at.timestamp(), event.event_id) > requirement_order
        for event in history
    ):
        return False
    expected_intervals: tuple[MicrobatchInterval, ...] = tuple(
        MicrobatchInterval(start=render(value=batch.start), end=render(value=batch.end))
        for batch in compute_batch_windows(
            start=requirement.run_start,
            end=requirement.run_end,
            batch_size=requirement.batch_size,
            cursor_type=cursor_type,
        )
    )
    projection: ReplayRequirementProjection = project_replay_requirement(
        requirement=requirement,
        current_model_version_hash=requirement.required_model_version_hash or "",
        expected_intervals=expected_intervals,
        coverage=project_microbatch_coverage(
            events=history,
            expected_intervals=expected_intervals,
            cursor_type=cursor_type,
        ),
        cursor_type=cursor_type,
    )
    return projection.state == ReplayRequirementState.INCOMPLETE


def _record_microbatch_completion(
    *,
    context: ModelMaterializationContext,
    batch: BatchWindow,
    is_full_refresh: bool,
    rows_affected: int | None,
    history: _MicrobatchHistoryContext,
    state: MicrobatchLifecycleState,
    targets: MicrobatchTargets,
) -> ModelExecutionResult | None:
    now: datetime = datetime.now(tz=UTC)
    batch_key: tuple[str, str] = (render(value=batch.start), render(value=batch.end))
    recovery_origin: _RecoveryOrigin | None = history.recovery_origins.get(batch_key)
    completion_scope: MicrobatchScope = _current_completion_scope(
        context=context,
        scope=history.scope,
        physical_target_name=(
            targets.target_table if is_full_refresh else context.entry.destination.name
        ),
    )
    event: MicrobatchEvent = MicrobatchEvent(
        event_id=deterministic_microbatch_event_id(
            scope=completion_scope,
            record_type=MicrobatchRecordType.PARTITION_COMPLETION,
            partition_start=batch_key[0],
            partition_end=batch_key[1],
            completion_reason=":".join(
                (
                    (
                        MicrobatchCompletionType.RECOVERY.value
                        if batch_key in history.recovery_intervals
                        else MicrobatchCompletionType.INITIAL.value
                    ),
                    _expected_model_version_hash(context=context),
                    history.replay_requirement_id or "",
                    context.run_id,
                )
            ),
        ),
        record_type=MicrobatchRecordType.PARTITION_COMPLETION,
        scope=completion_scope,
        origin_run_id=(
            recovery_origin.origin_run_id
            if recovery_origin is not None
            else history.origin_run_id or context.run_id
        ),
        execution_run_id=context.run_id,
        run_type=(recovery_origin.run_type if recovery_origin is not None else history.run_type),
        completion_type=(
            MicrobatchCompletionType.RECOVERY
            if batch_key in history.recovery_intervals
            else MicrobatchCompletionType.INITIAL
        ),
        run_start=render(
            value=recovery_origin.run_start if recovery_origin is not None else history.run_start
        ),
        run_end=render(
            value=recovery_origin.run_end if recovery_origin is not None else history.run_end
        ),
        partition_start=batch_key[0],
        partition_end=batch_key[1],
        batch_size=history.batch_size,
        cursor_column=context.entry.cursor_column or "",
        cursor_type=context.entry.cursor_type or "",
        cursor_grain=context.entry.cursor_grain,
        model_version_hash=_expected_model_version_hash(context=context),
        definition_hash=_definition_hash(context=context),
        fingerprint_status=MicrobatchFingerprintStatus.KNOWN,
        replay_requirement_id=(
            recovery_origin.replay_requirement_id
            if recovery_origin is not None
            else history.replay_requirement_id
        ),
        rows_affected=rows_affected,
        completed_at=now,
        origin_run_started_at=(
            recovery_origin.origin_run_started_at
            if recovery_origin is not None
            else history.origin_run_started_at
        ),
        execution_run_started_at=history.execution_run_started_at,
    )
    try:
        history.store.write_many((event,))
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.MICROBATCH_STATE,
            error=exc,
            error_prefix=f"batch {batch.index}: target DML succeeded but completion write failed",
            staging_relation=targets.delta_qualified,
            promoted_relation=targets.target_qualified,
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    return None


def _microbatch_run_type(*, context: ModelMaterializationContext) -> MicrobatchRunType:
    entry: ModelPlanEntry = context.entry
    if entry.start_cursor_override is not None or entry.end_cursor_override is not None:
        return MicrobatchRunType.BACKFILL
    if (
        entry.backfill.action in {BackfillAction.BOUNDED, BackfillAction.FULL}
        and entry.previous_version_hash is not None
        and _expected_model_version_hash(context=context) != entry.previous_version_hash
    ):
        return MicrobatchRunType.REPLAY_ON_CHANGE
    return MicrobatchRunType.NORMAL


def _definition_hash(*, context: ModelMaterializationContext) -> str:
    return compute_query_hash(
        query_sql=context.entry.fingerprint_query_sql,
        dialect=context.adapter.sql_analysis_dialect(),
    )


def _expected_model_version_hash(*, context: ModelMaterializationContext) -> str:
    return (
        context.microbatch_model_version_hash
        or context.entry.fingerprint_version_hash
        or _definition_hash(context=context)
    )


def _current_completion_scope(
    *,
    context: ModelMaterializationContext,
    scope: MicrobatchScope,
    physical_target_name: str | None = None,
    preserve_replay_generation: bool = True,
) -> MicrobatchScope:
    if preserve_replay_generation and scope.physical_generation_id.startswith(
        MICROBATCH_REPLAY_GENERATION_PREFIX
    ):
        return scope
    generation: str | None = context.adapter.physical_relation_generation(
        connection=context.connection,
        database=context.entry.destination.database,
        schema=context.entry.destination.schema,
        name=physical_target_name or context.entry.destination.name,
    )
    if generation is None:
        return scope
    if scope.scope_kind == DIRECT_MICROBATCH_SCOPE_KIND:
        return replace(scope, physical_generation_id=generation)
    generation_hash: str = hashlib.sha256(generation.encode()).hexdigest()
    if scope.physical_generation_id.rpartition(":")[2] == generation_hash:
        return scope
    return replace(
        scope,
        physical_generation_id=f"{scope.physical_generation_id}:{generation_hash}",
    )


def _reconcile_microbatch_history(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    targets: MicrobatchTargets,
    history: _MicrobatchHistoryContext,
    normal_batches: tuple[BatchWindow, ...],
    is_full_refresh: bool,
) -> (
    tuple[MicrobatchLifecycleState, _MicrobatchHistoryContext, tuple[BatchWindow, ...]]
    | ModelExecutionResult
):
    if is_full_refresh:
        return state, history, normal_batches
    cursor_type: str = context.entry.cursor_type or ""
    if cursor_type not in {CursorType.TIMESTAMP, CursorType.INTEGER}:
        return state, history, normal_batches
    envelope: CursorBounds | None | ModelExecutionResult = _physical_cursor_envelope(
        context=context, state=state, targets=targets
    )
    if isinstance(envelope, ModelExecutionResult):
        return envelope
    expected_intervals: tuple[MicrobatchInterval, ...] = _clamp_intervals_to_model_domain(
        context=context,
        intervals=_accounting_intervals(context=context, history=history, envelope=envelope),
    )
    coverage: MicrobatchCoverageProjection = project_microbatch_coverage(
        events=history.history,
        expected_intervals=expected_intervals,
        cursor_type=cursor_type,
    )
    policy_result: (
        tuple[
            tuple[MicrobatchInterval, ...],
            tuple[MicrobatchInterval, ...],
            tuple[MicrobatchEvent, ...],
        ]
        | ModelExecutionResult
    ) = _apply_unaccounted_policy(
        context=context,
        state=state,
        targets=targets,
        history=history,
        intervals=coverage.unaccounted,
    )
    if isinstance(policy_result, ModelExecutionResult):
        return policy_result
    policy_recovery, synthetic_intervals, synthetic_events = policy_result
    (
        replay_recovery,
        replay_requirement_state,
        replay_unknown_fingerprint_count,
    ) = _replay_recovery_intervals(
        context=context,
        history=history,
        synthetic_intervals=synthetic_intervals,
    )
    known_missing: tuple[MicrobatchInterval, ...] = coverage.known_missing
    if history.batch_concurrency == 1:
        known_missing, inherited_interior = _serial_trailing_recovery_only(
            history=history, intervals=known_missing, cursor_type=cursor_type
        )
        if inherited_interior:
            state = replace(
                state,
                warnings=[
                    *state.warnings,
                    f"serial microbatch history for '{context.entry.name}' contains "
                    f"{len(inherited_interior)} inherited interior gap(s); continuing from the "
                    "ordinary frontier and leaving older gaps unprocessed. Use an explicit "
                    "bounded backfill to repair them",
                ],
            )
    recovery: tuple[MicrobatchInterval, ...] = _clamp_intervals_to_model_domain(
        context=context,
        intervals=_deduplicate_intervals((*known_missing, *policy_recovery, *replay_recovery)),
    )
    recovery_keys: frozenset[tuple[str, str]] = frozenset(
        (interval.start, interval.end) for interval in recovery
    )
    recovery_batches: tuple[BatchWindow, ...] = tuple(
        BatchWindow(start=interval.start, end=interval.end, index=index)
        for index, interval in enumerate(recovery)
    )
    ordinary_batches: tuple[BatchWindow, ...] = (
        ()
        if history.run_type == MicrobatchRunType.REPLAY_ON_CHANGE
        else tuple(
            batch
            for batch in normal_batches
            if (render(value=batch.start), render(value=batch.end)) not in recovery_keys
        )
    )
    all_batches: tuple[BatchWindow, ...] = tuple(
        replace(batch, index=index)
        for index, batch in enumerate((*recovery_batches, *ordinary_batches))
    )
    accounting_intervals: tuple[MicrobatchAccountingInterval, ...] = (
        _build_microbatch_accounting_intervals(
            coverage=coverage.intervals,
            known_missing=coverage.known_missing,
            unaccounted=coverage.unaccounted,
            synthetic=synthetic_intervals,
            recovery=recovery,
        )
    )
    recovery_origins: dict[tuple[str, str], _RecoveryOrigin] = _known_gap_origins(
        events=history.history,
        known_missing=coverage.known_missing,
        cursor_type=cursor_type,
    )
    return (
        state,
        replace(
            history,
            recovery_intervals=recovery_keys,
            known_missing_intervals=coverage.known_missing,
            unaccounted_intervals=coverage.unaccounted,
            synthetic_intervals=synthetic_intervals,
            accounting_intervals=accounting_intervals,
            replay_requirement_state=replay_requirement_state,
            replay_unknown_fingerprint_count=max(
                replay_unknown_fingerprint_count,
                len(coverage.unknown_fingerprints) + len(synthetic_intervals),
            ),
            contiguous_frontier=coverage.contiguous_frontier,
            recovery_origins=recovery_origins,
            pending_events=(*history.pending_events, *synthetic_events),
        ),
        all_batches,
    )


def _serial_trailing_recovery_only(
    *,
    history: _MicrobatchHistoryContext,
    intervals: tuple[MicrobatchInterval, ...],
    cursor_type: str,
) -> tuple[tuple[MicrobatchInterval, ...], tuple[MicrobatchInterval, ...]]:
    completed_ends: tuple[str, ...] = tuple(
        event.partition_end
        for event in history.history
        if event.record_type == MicrobatchRecordType.PARTITION_COMPLETION
        and event.partition_end is not None
    )
    if not completed_ends:
        return intervals, ()
    latest_end: str = _cursor_bound(
        values=list(completed_ends), cursor_type=cursor_type, maximum=True
    )
    interior: tuple[MicrobatchInterval, ...] = tuple(
        interval
        for interval in intervals
        if not _cursor_lte(left=latest_end, right=interval.end, cursor_type=cursor_type)
    )
    interior_keys: frozenset[tuple[str, str]] = frozenset(
        (interval.start, interval.end) for interval in interior
    )
    trailing: tuple[MicrobatchInterval, ...] = tuple(
        interval for interval in intervals if (interval.start, interval.end) not in interior_keys
    )
    return trailing, interior


def _accounting_intervals(
    *,
    context: ModelMaterializationContext,
    history: _MicrobatchHistoryContext,
    envelope: CursorBounds | None,
) -> tuple[MicrobatchInterval, ...]:
    cursor_type: str = context.entry.cursor_type or ""
    continuity_events: tuple[MicrobatchEvent, ...] = tuple(
        event
        for event in history.history
        if event.run_type != MicrobatchRunType.BACKFILL
        and event.partition_start is not None
        and event.partition_end is not None
    )
    starts: list[str] = [
        event.partition_start for event in continuity_events if event.partition_start is not None
    ]
    ends: list[str] = [
        event.partition_end for event in history.history if event.partition_end is not None
    ]
    if envelope is not None:
        if not starts:
            starts.append(sentinel_to_token(sentinel=envelope.start))
        ends.append(sentinel_to_token(sentinel=envelope.end))
    if context.entry.cursor_start is not None and not starts:
        starts.append(context.entry.cursor_start)
    if not starts or not ends:
        return ()
    floor: str = _cursor_bound(values=starts, cursor_type=cursor_type, maximum=False)
    frontier: str = _cursor_bound(values=ends, cursor_type=cursor_type, maximum=True)
    return tuple(
        MicrobatchInterval(start=render(value=batch.start), end=render(value=batch.end))
        for batch in compute_batch_windows(
            start=floor,
            end=frontier,
            batch_size=history.batch_size,
            cursor_type=cursor_type,
        )
    )


def _apply_unaccounted_policy(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    targets: MicrobatchTargets,
    history: _MicrobatchHistoryContext,
    intervals: tuple[MicrobatchInterval, ...],
) -> (
    tuple[
        tuple[MicrobatchInterval, ...],
        tuple[MicrobatchInterval, ...],
        tuple[MicrobatchEvent, ...],
    ]
    | ModelExecutionResult
):
    if not intervals:
        return (), (), ()
    policy: UnaccountedPartitionPolicy = UnaccountedPartitionPolicy(
        context.microbatch_unaccounted_partition_policy
    )
    if policy == UnaccountedPartitionPolicy.RECOVER_ALL:
        return intervals, (), ()
    if policy == UnaccountedPartitionPolicy.SYNTHESIZE:
        events: tuple[MicrobatchEvent, ...] = _build_synthetic_completions(
            context=context,
            state=state,
            history=history,
            intervals=intervals,
            policy=policy,
            row_counts=None,
        )
        return (), intervals, events
    counts: dict[tuple[str, str], int] | ModelExecutionResult = _count_unaccounted_intervals(
        context=context,
        state=state,
        targets=targets,
        intervals=intervals,
    )
    if isinstance(counts, ModelExecutionResult):
        return counts
    empty: tuple[MicrobatchInterval, ...] = tuple(
        interval for interval in intervals if counts[(interval.start, interval.end)] == 0
    )
    non_empty: tuple[MicrobatchInterval, ...] = tuple(
        interval for interval in intervals if counts[(interval.start, interval.end)] > 0
    )
    events = _build_synthetic_completions(
        context=context,
        state=state,
        history=history,
        intervals=non_empty,
        policy=policy,
        row_counts=counts,
    )
    return empty, non_empty, events


def _replay_recovery_intervals(
    *,
    context: ModelMaterializationContext,
    history: _MicrobatchHistoryContext,
    synthetic_intervals: tuple[MicrobatchInterval, ...],
) -> tuple[tuple[MicrobatchInterval, ...], ReplayRequirementState | None, int]:
    if history.replay_requirement_id is None:
        return (), None, 0
    requirement: MicrobatchEvent | None = next(
        (
            event
            for event in history.history
            if event.record_type == MicrobatchRecordType.REPLAY_REQUIREMENT
            and event.replay_requirement_id == history.replay_requirement_id
        ),
        None,
    )
    if requirement is None:
        return (), None, 0
    replay_intervals: tuple[MicrobatchInterval, ...] = tuple(
        MicrobatchInterval(start=render(value=batch.start), end=render(value=batch.end))
        for batch in compute_batch_windows(
            start=requirement.run_start,
            end=requirement.run_end,
            batch_size=requirement.batch_size,
            cursor_type=context.entry.cursor_type or "",
        )
    )
    projection: ReplayRequirementProjection = project_replay_requirement(
        requirement=requirement,
        current_model_version_hash=_expected_model_version_hash(context=context),
        expected_intervals=replay_intervals,
        coverage=project_microbatch_coverage(
            events=history.history,
            expected_intervals=replay_intervals,
            cursor_type=context.entry.cursor_type or "",
        ),
        cursor_type=context.entry.cursor_type or "",
    )
    synthetic_keys: frozenset[tuple[str, str]] = frozenset(
        (interval.start, interval.end) for interval in synthetic_intervals
    )
    if projection.state == ReplayRequirementState.SUPERSEDED:
        return (), projection.state, len(projection.unknown_fingerprints)
    return (
        _clamp_intervals_to_model_domain(
            context=context,
            intervals=tuple(
                interval
                for interval in projection.missing
                if (interval.start, interval.end) not in synthetic_keys
            ),
        ),
        projection.state,
        len(projection.unknown_fingerprints) + len(synthetic_intervals),
    )


def _clamp_intervals_to_model_domain(
    *, context: ModelMaterializationContext, intervals: tuple[MicrobatchInterval, ...]
) -> tuple[MicrobatchInterval, ...]:
    cursor_type: str = context.entry.cursor_type or ""
    floor: CursorScalar | None = (
        parse(raw=context.entry.cursor_start, cursor_type=cursor_type)
        if context.entry.cursor_start is not None
        else None
    )
    ceiling: CursorScalar | None = (
        parse(raw=context.entry.cursor_end, cursor_type=cursor_type)
        if context.entry.cursor_end is not None
        else None
    )
    bounded: list[MicrobatchInterval] = []
    for interval in intervals:
        start: CursorScalar = parse(raw=interval.start, cursor_type=cursor_type)
        end: CursorScalar = parse(raw=interval.end, cursor_type=cursor_type)
        if floor is not None and compare(left=start, right=floor) <= 0:
            start = floor
        if ceiling is not None and compare(left=ceiling, right=end) <= 0:
            end = ceiling
        if ceiling is not None and compare(left=ceiling, right=start) <= 0:
            continue
        if compare(left=end, right=start) <= 0:
            continue
        bounded.append(MicrobatchInterval(start=render(value=start), end=render(value=end)))
    return _deduplicate_intervals(tuple(bounded))


def _physical_cursor_envelope(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    targets: MicrobatchTargets,
) -> CursorBounds | None | ModelExecutionResult:
    if not cached_relation_exists(
        adapter=context.adapter,
        connection=context.connection,
        database=targets.target_database,
        schema=targets.target_schema,
        name=targets.target_table,
    ):
        return None
    cursor_column: str | None = context.entry.cursor_column
    if cursor_column is None:
        return None
    quoted_cursor: str = context.adapter.render_cursor_operand(
        expression=context.adapter.render_identifier(cursor_column),
        cursor_type=context.entry.cursor_type,
    )
    try:
        cursor: Any = context.adapter.execute(
            connection=context.connection,
            sql=(
                f"SELECT MIN({quoted_cursor}), MAX({quoted_cursor}) FROM {targets.target_qualified}"
            ),
        )
        row: Any = cursor.fetchone()
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.MICROBATCH_STATE,
            error=exc,
            error_prefix="failed to inspect target cursor envelope",
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    if row is None or row[0] is None or row[1] is None:
        return None
    return _canonical_destination_envelope(
        raw_min=row[0],
        raw_max=row[1],
        cursor_type=context.entry.cursor_type,
        cursor_grain=context.entry.cursor_grain,
    )


def _canonical_destination_envelope(
    *, raw_min: object, raw_max: object, cursor_type: str | None, cursor_grain: str | None
) -> CursorBounds:
    if cursor_type == CursorType.INTEGER:
        minimum: CursorScalar = parse(raw=raw_min, cursor_type=CursorType.INTEGER)
        maximum: CursorScalar = exclusive_end_from_observed_max(
            value=parse(raw=raw_max, cursor_type=CursorType.INTEGER), grain=None
        )
        return CursorBounds(
            start=minimum,
            end=maximum,
        )
    if not isinstance(raw_min, datetime | date) or not isinstance(raw_max, datetime | date):
        raise ExecutorInputError("timestamp cursor envelope returned non-temporal bounds")
    grain: CursorGrain = CursorGrain(cursor_grain or CursorGrain.SECOND)
    minimum = parse(raw=raw_min, cursor_type=CursorType.TIMESTAMP)
    maximum = parse(raw=raw_max, cursor_type=CursorType.TIMESTAMP)
    start: CursorScalar = floor_to_grain(value=minimum, grain=grain)
    end_floor: CursorScalar = floor_to_grain(value=maximum, grain=grain)
    end: CursorScalar = next_boundary(value=end_floor, grain=grain)
    return CursorBounds(start=start, end=end)


def _build_synthetic_completions(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    history: _MicrobatchHistoryContext,
    intervals: tuple[MicrobatchInterval, ...],
    policy: UnaccountedPartitionPolicy,
    row_counts: dict[tuple[str, str], int] | None,
) -> tuple[MicrobatchEvent, ...]:
    if not intervals:
        return ()
    now: datetime = datetime.now(tz=UTC)
    return tuple(
        MicrobatchEvent(
            event_id=deterministic_microbatch_event_id(
                scope=history.scope,
                record_type=MicrobatchRecordType.SYNTHETIC_COMPLETION,
                partition_start=interval.start,
                partition_end=interval.end,
                completion_reason=f"completion_history_missing:{policy.value}",
            ),
            record_type=MicrobatchRecordType.SYNTHETIC_COMPLETION,
            scope=history.scope,
            origin_run_id=context.run_id,
            origin_run_started_at=history.execution_run_started_at,
            execution_run_id=context.run_id,
            execution_run_started_at=history.execution_run_started_at,
            run_type=history.run_type,
            completion_type=MicrobatchCompletionType.RECOVERY,
            run_start=render(value=history.run_start),
            run_end=render(value=history.run_end),
            partition_start=interval.start,
            partition_end=interval.end,
            batch_size=history.batch_size,
            cursor_column=context.entry.cursor_column or "",
            cursor_type=context.entry.cursor_type or "",
            cursor_grain=context.entry.cursor_grain,
            model_version_hash=None,
            definition_hash=None,
            fingerprint_status=MicrobatchFingerprintStatus.UNKNOWN,
            coverage_source="synthetic",
            observed_row_count=(
                None if row_counts is None else row_counts[(interval.start, interval.end)]
            ),
            observed_at=now,
            synthetic_reason="completion_history_missing",
            unaccounted_policy=policy,
        )
        for interval in intervals
    )


def _count_unaccounted_intervals(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    targets: MicrobatchTargets,
    intervals: tuple[MicrobatchInterval, ...],
) -> dict[tuple[str, str], int] | ModelExecutionResult:
    cursor_column: str = context.adapter.render_cursor_operand(
        expression=context.adapter.render_identifier(context.entry.cursor_column or ""),
        cursor_type=context.entry.cursor_type,
    )
    counts: dict[tuple[str, str], int] = {}
    chunk_start: int
    for chunk_start in range(0, len(intervals), _UNACCOUNTED_COUNT_CHUNK_SIZE):
        chunk: tuple[MicrobatchInterval, ...] = intervals[
            chunk_start : chunk_start + _UNACCOUNTED_COUNT_CHUNK_SIZE
        ]
        expressions: list[str] = []
        for index, interval in enumerate(chunk):
            start_literal: str = context.adapter.render_cursor_bound_literal(
                value=interval.start, cursor_type=context.entry.cursor_type
            )
            end_literal: str = context.adapter.render_cursor_bound_literal(
                value=interval.end, cursor_type=context.entry.cursor_type
            )
            expressions.append(
                f"SUM(CASE WHEN {cursor_column} >= {start_literal} AND "
                f"{cursor_column} < {end_literal} THEN 1 ELSE 0 END) "
                f"AS __sqb_count_{index}"
            )
        try:
            cursor: Any = context.adapter.execute(
                connection=context.connection,
                sql=f"SELECT {', '.join(expressions)} FROM {targets.target_qualified}",
            )
            row: Any = cursor.fetchone()
        except Exception as exc:
            return build_failed_result(
                entry=context.entry,
                phase=ExecutionPhase.MICROBATCH_STATE,
                error=exc,
                error_prefix="failed to count unaccounted microbatch intervals",
                warnings=state.warnings,
                audit_results=state.audit_results,
                statement_recorder=state.statement_recorder,
            )
        counts.update(
            {
                (interval.start, interval.end): int(row[index] or 0)
                for index, interval in enumerate(chunk)
            }
        )
    return counts


def _deduplicate_intervals(
    intervals: tuple[MicrobatchInterval, ...],
) -> tuple[MicrobatchInterval, ...]:
    return tuple({(interval.start, interval.end): interval for interval in intervals}.values())


def _build_microbatch_accounting_intervals(
    *,
    coverage: tuple[ProjectedMicrobatchInterval, ...],
    known_missing: tuple[MicrobatchInterval, ...],
    unaccounted: tuple[MicrobatchInterval, ...],
    synthetic: tuple[MicrobatchInterval, ...],
    recovery: tuple[MicrobatchInterval, ...],
) -> tuple[MicrobatchAccountingInterval, ...]:
    results: list[MicrobatchAccountingInterval] = [
        MicrobatchAccountingInterval(
            partition_start=interval.start,
            partition_end=interval.end,
            accounting_status=(
                "synthetic"
                if interval.record_type == MicrobatchRecordType.SYNTHETIC_COMPLETION
                else "ordinary_completion"
            ),
            fingerprint_status=interval.fingerprint_status.value,
            model_version_hash=interval.model_version_hash,
            completion_type=(
                None if interval.completion_type is None else interval.completion_type.value
            ),
            event_id=interval.event_id,
        )
        for interval in coverage
    ]
    recovery_keys: frozenset[tuple[str, str]] = frozenset(
        (interval.start, interval.end) for interval in recovery
    )
    synthetic_keys: frozenset[tuple[str, str]] = frozenset(
        (interval.start, interval.end) for interval in synthetic
    )
    for interval in known_missing:
        results.append(
            MicrobatchAccountingInterval(
                partition_start=interval.start,
                partition_end=interval.end,
                accounting_status="recovery_pending",
                fingerprint_status=MicrobatchFingerprintStatus.UNKNOWN.value,
            )
        )
    for interval in unaccounted:
        key: tuple[str, str] = (interval.start, interval.end)
        status: str = "unaccounted"
        if key in synthetic_keys:
            status = "synthetic"
        elif key in recovery_keys:
            status = "recovery_pending"
        results.append(
            MicrobatchAccountingInterval(
                partition_start=interval.start,
                partition_end=interval.end,
                accounting_status=status,
                fingerprint_status=MicrobatchFingerprintStatus.UNKNOWN.value,
            )
        )
    return tuple(results)


def _known_gap_origins(
    *,
    events: tuple[MicrobatchEvent, ...],
    known_missing: tuple[MicrobatchInterval, ...],
    cursor_type: str,
) -> dict[tuple[str, str], _RecoveryOrigin]:
    origins: dict[tuple[str, str], _RecoveryOrigin] = {}
    for interval in known_missing:
        candidates: tuple[MicrobatchEvent, ...] = tuple(
            event
            for event in events
            if event.record_type == MicrobatchRecordType.PARTITION_COMPLETION
            and event.partition_start is not None
            and _cursor_lte(
                left=interval.end,
                right=event.partition_start,
                cursor_type=cursor_type,
            )
            and _cursor_lte(left=event.run_start, right=interval.start, cursor_type=cursor_type)
            and _cursor_lte(left=interval.end, right=event.run_end, cursor_type=cursor_type)
        )
        if not candidates:
            continue
        witness: MicrobatchEvent = min(
            candidates, key=lambda event: (event.created_at, event.event_id)
        )
        origins[(interval.start, interval.end)] = _RecoveryOrigin(
            origin_run_id=witness.origin_run_id,
            origin_run_started_at=witness.origin_run_started_at,
            run_start=parse(raw=witness.run_start, cursor_type=cursor_type),
            run_end=parse(raw=witness.run_end, cursor_type=cursor_type),
            run_type=witness.run_type,
            replay_requirement_id=witness.replay_requirement_id,
        )
    return origins


def _cursor_lte(*, left: str, right: str, cursor_type: str) -> bool:
    if cursor_type == CursorType.TIMESTAMP:
        return datetime.fromisoformat(left) <= datetime.fromisoformat(right)
    return Decimal(left) <= Decimal(right)


def _microbatch_result_fields(
    *, history: _MicrobatchHistoryContext, succeeded: bool
) -> dict[str, Any]:
    replay_state: ReplayRequirementState | None = history.replay_requirement_state
    if succeeded and history.run_type == MicrobatchRunType.REPLAY_ON_CHANGE:
        if (
            replay_state == ReplayRequirementState.COMPLETE_WITH_UNKNOWN_FINGERPRINTS
            or history.synthetic_intervals
        ):
            replay_state = ReplayRequirementState.COMPLETE_WITH_UNKNOWN_FINGERPRINTS
        else:
            replay_state = ReplayRequirementState.VERIFIED_COMPLETE
    return {
        "microbatch_run_type": history.run_type.value,
        "microbatch_recovery_batch_count": len(history.recovery_intervals),
        "microbatch_known_gap_count": len(history.known_missing_intervals),
        "microbatch_unaccounted_interval_count": len(history.unaccounted_intervals),
        "microbatch_synthetic_completion_count": len(history.synthetic_intervals),
        "microbatch_unknown_fingerprint_count": history.replay_unknown_fingerprint_count,
        "microbatch_contiguous_frontier": history.contiguous_frontier,
        "microbatch_unaccounted_partition_policy": (
            history.unaccounted_partition_policy if history.concurrent_enabled else None
        ),
        "microbatch_replay_requirement_id": history.replay_requirement_id,
        "microbatch_required_model_version_hash": history.required_model_version_hash,
        "microbatch_physical_generation_id": (history.scope.physical_generation_id),
        "microbatch_concurrent_enabled": history.concurrent_enabled,
        "microbatch_batch_concurrency": history.batch_concurrency,
        "microbatch_global_concurrency": history.global_concurrency,
        "microbatch_replay_requirement_state": (
            None if replay_state is None else replay_state.value
        ),
        "microbatch_accounting_intervals": history.accounting_intervals,
        "microbatch_plan_reason": history.resolution_reason,
    }


def _refresh_microbatch_result_history(
    *,
    context: ModelMaterializationContext,
    history: _MicrobatchHistoryContext,
    batches: tuple[BatchWindow, ...],
    physical_target_name: str,
) -> _MicrobatchHistoryContext:
    if not batches:
        return history
    expected: tuple[MicrobatchInterval, ...] = tuple(
        MicrobatchInterval(start=render(value=batch.start), end=render(value=batch.end))
        for batch in batches
    )
    current_scope: MicrobatchScope = _current_completion_scope(
        context=context, scope=history.scope, physical_target_name=physical_target_name
    )
    try:
        events: tuple[MicrobatchEvent, ...] = history.store.read_scope_history(current_scope)
    except Exception:
        return history
    coverage: MicrobatchCoverageProjection = project_microbatch_coverage(
        events=events,
        expected_intervals=expected,
        cursor_type=context.entry.cursor_type or "",
    )
    synthetic: tuple[MicrobatchInterval, ...] = tuple(
        MicrobatchInterval(start=interval.start, end=interval.end)
        for interval in coverage.intervals
        if interval.record_type == MicrobatchRecordType.SYNTHETIC_COMPLETION
    )
    replay_state: ReplayRequirementState | None = history.replay_requirement_state
    if history.replay_requirement_id is not None:
        requirement: MicrobatchEvent | None = next(
            (
                event
                for event in events
                if event.record_type == MicrobatchRecordType.REPLAY_REQUIREMENT
                and event.replay_requirement_id == history.replay_requirement_id
            ),
            None,
        )
        if requirement is not None:
            replay_state = project_replay_requirement(
                requirement=requirement,
                current_model_version_hash=_expected_model_version_hash(context=context),
                expected_intervals=expected,
                coverage=coverage,
                cursor_type=context.entry.cursor_type or "",
            ).state
    return replace(
        history,
        scope=current_scope,
        history=events,
        synthetic_intervals=synthetic,
        replay_requirement_state=replay_state,
        contiguous_frontier=coverage.contiguous_frontier,
        accounting_intervals=_build_microbatch_accounting_intervals(
            coverage=coverage.intervals,
            known_missing=coverage.known_missing,
            unaccounted=coverage.unaccounted,
            synthetic=synthetic,
            recovery=(),
        ),
    )


def _cursor_bound(*, values: list[str], cursor_type: str, maximum: bool) -> str:
    if cursor_type == CursorType.TIMESTAMP:
        return (
            max(values, key=datetime.fromisoformat)
            if maximum
            else min(values, key=datetime.fromisoformat)
        )
    return (
        max(values, key=lambda value: Decimal(value))
        if maximum
        else min(values, key=lambda value: Decimal(value))
    )


def _reported_rows_affected(*, total_rows: int, row_count_known: bool) -> int | None:
    """Preserve a known zero row count while keeping unavailable counts as None."""

    return total_rows if row_count_known else None


def _stage_microbatch_delta(
    *,
    context: ModelMaterializationContext,
    batch: BatchWindow,
    window_text: str,
    targets: MicrobatchTargets,
    state: MicrobatchLifecycleState,
) -> ModelExecutionResult | None:
    log_debug_event(
        logger=_DEBUG_LOGGER,
        message="",
        sqlbuild_subject="model",
        sqlbuild_name=context.entry.name,
        sqlbuild_event="batch_start",
        sqlbuild_phase="batch",
        sqlbuild_window=window_text,
    )
    batch_sql: str = _substitute_sentinels(
        sql=context.entry.resolved_sql,
        batch_start=batch.start,
        batch_end=batch.end,
    )
    try:
        with diagnostics_context(
            sqlbuild_phase="materialize",
            sqlbuild_action_name="create_delta",
            sqlbuild_window=window_text,
        ):
            with OperationLifecycle(
                operation_kind="warehouse",
                operation_name="staging_creation",
                attributes=OperationAttributes(
                    phase="create",
                    adapter=canonicalize_operation_adapter(context.adapter.adapter_name),
                    target_kind="staging_relation",
                ),
            ) as lifecycle:
                create_intermediate_table(
                    adapter=context.adapter,
                    connection=context.connection,
                    destination=targets.delta_qualified,
                    sql=batch_sql,
                    config=model_output_config(context.entry),
                    statement_recorder=state.statement_recorder,
                )
                lifecycle.completed(metadata={"changed_count": 1})
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.STAGING,
            error=exc,
            error_prefix=f"batch {batch.index}",
            staging_relation=targets.delta_qualified,
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    return None


def _apply_microbatch_schema_change(
    *,
    context: ModelMaterializationContext,
    is_full_refresh: bool,
    schema_checked: bool,
    window_text: str,
    targets: MicrobatchTargets,
    state: MicrobatchLifecycleState,
) -> MicrobatchSchemaPhaseOutcome:
    if schema_checked or is_full_refresh:
        return MicrobatchSchemaPhaseOutcome(state=state, schema_checked=schema_checked)
    try:
        with diagnostics_context(
            sqlbuild_phase="schema_change",
            sqlbuild_action_name="inspect",
            sqlbuild_window=window_text,
        ):
            delta_columns: tuple[ColumnInfo, ...] = inspect_runtime_relation_schema(
                adapter=context.adapter,
                connection=context.connection,
                database=targets.target_database,
                schema=targets.target_schema,
                name=targets.delta_table,
            )
            target_columns: tuple[ColumnInfo, ...] = inspect_runtime_relation_schema(
                adapter=context.adapter,
                connection=context.connection,
                database=targets.target_database,
                schema=targets.target_schema,
                name=targets.target_table,
            )
            target_columns = context.adapter.widen_target_nullability(
                connection=context.connection,
                destination=targets.target_qualified,
                target_columns=target_columns,
                incoming_columns=_widenable_columns(
                    on_schema_change=context.entry.on_schema_change or _DEFAULT_ON_SCHEMA_CHANGE,
                    incoming_columns=delta_columns,
                ),
                statement_recorder=state.statement_recorder,
            )
            schema_warnings: tuple[str, ...] = _apply_schema_change(
                adapter=context.adapter,
                connection=context.connection,
                target_qualified=targets.target_qualified,
                target_columns=target_columns,
                delta_columns=delta_columns,
                on_schema_change=context.entry.on_schema_change or _DEFAULT_ON_SCHEMA_CHANGE,
                statement_recorder=state.statement_recorder,
                bound_views=context.bound_views,
            )
    except Exception as exc:
        return MicrobatchSchemaPhaseOutcome(
            state=state,
            schema_checked=False,
            failure=build_failed_result(
                entry=context.entry,
                phase=ExecutionPhase.SCHEMA_CHANGE,
                error=exc,
                staging_relation=targets.delta_qualified,
                warnings=state.warnings,
                audit_results=state.audit_results,
                statement_recorder=state.statement_recorder,
            ),
        )
    return MicrobatchSchemaPhaseOutcome(
        state=replace(state, warnings=[*state.warnings, *schema_warnings]),
        schema_checked=True,
    )


def _enforce_microbatch_types(
    *,
    context: ModelMaterializationContext,
    declared_columns: tuple[ColumnInfo, ...],
    batch: BatchWindow,
    window_text: str,
    targets: MicrobatchTargets,
    state: MicrobatchLifecycleState,
) -> ModelExecutionResult | None:
    if not context.entry.type_enforcement or not declared_columns:
        return None
    try:
        with diagnostics_context(
            sqlbuild_phase="type_enforcement",
            sqlbuild_action_name="rebuild_delta",
            sqlbuild_window=window_text,
        ):
            enforce_types_staged(
                adapter=context.adapter,
                connection=context.connection,
                staging_qualified=targets.delta_qualified,
                staging_database=targets.target_database,
                staging_schema=targets.target_schema,
                staging_table=targets.delta_table,
                declared_columns=declared_columns,
                table_type=TableType.TRANSIENT,
                statement_recorder=state.statement_recorder,
            )
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.TYPE_ENFORCEMENT,
            error=exc,
            error_prefix=f"batch {batch.index}",
            staging_relation=targets.delta_qualified,
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    return None


def _run_microbatch_delta_audits(
    *,
    context: ModelMaterializationContext,
    batch: BatchWindow,
    targets: MicrobatchTargets,
    state: MicrobatchLifecycleState,
) -> MicrobatchPhaseOutcome:
    delta_audit_run: FinalAuditRun = run_delta_scope_audits(
        context=context,
        delta_qualified=targets.delta_qualified,
    )
    updated_state: MicrobatchLifecycleState = replace(
        state,
        audit_results=[*state.audit_results, *delta_audit_run.results],
    )
    if not delta_audit_run.has_error:
        return MicrobatchPhaseOutcome(state=updated_state)
    return MicrobatchPhaseOutcome(
        state=updated_state,
        failure=build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.AUDIT,
            error=(
                f"batch {batch.index}: delta audit for '{context.entry.name}' failed before "
                "target update with severity level: error"
            ),
            staging_relation=targets.delta_qualified,
            warnings=updated_state.warnings,
            audit_results=updated_state.audit_results,
            statement_recorder=updated_state.statement_recorder,
        ),
    )


def _apply_microbatch_dml(
    *,
    context: ModelMaterializationContext,
    batch: BatchWindow,
    completed_batches: int,
    is_full_refresh: bool,
    window_text: str,
    targets: MicrobatchTargets,
    state: MicrobatchLifecycleState,
    known_columns: _BatchColumns,
) -> ModelExecutionResult | int | None:
    try:
        with diagnostics_context(
            sqlbuild_phase="dml",
            sqlbuild_action_name="apply",
            sqlbuild_window=window_text,
        ):
            target_columns: tuple[ColumnInfo, ...] = (
                inspect_runtime_relation_schema(
                    adapter=context.adapter,
                    connection=context.connection,
                    database=targets.target_database,
                    schema=targets.target_schema,
                    name=targets.target_table,
                )
                if not is_full_refresh or completed_batches > 0
                else ()
            )
            delta_columns: tuple[ColumnInfo, ...] = known_columns.delta(
                partial(
                    inspect_runtime_relation_schema,
                    adapter=context.adapter,
                    connection=context.connection,
                    database=targets.target_database,
                    schema=targets.target_schema,
                    name=targets.delta_table,
                )
            )
            _validate_cursor_output_columns(entry=context.entry, delta_columns=delta_columns)
            batch_rows: int | None
            if is_full_refresh and completed_batches == 0:
                batch_rows = _count_microbatch_delta_rows(context=context, targets=targets)
                context.adapter.create_table_as(
                    connection=context.connection,
                    destination=targets.target_qualified,
                    sql=f"SELECT * FROM {targets.delta_qualified}",
                    config={
                        "table_type": context.entry.table_type,
                        "physical_storage": context.entry.physical_storage,
                    },
                    statement_recorder=state.statement_recorder,
                )
            else:
                reported_rows: int | None = _execute_dml(
                    adapter=context.adapter,
                    connection=context.connection,
                    target_qualified=targets.target_qualified,
                    delta_qualified=targets.delta_qualified,
                    target_columns=target_columns,
                    delta_columns=delta_columns,
                    entry=context.entry,
                    cursor_start=render(value=batch.start),
                    cursor_end=render(value=batch.end),
                    statement_recorder=state.statement_recorder,
                )
                batch_rows = (
                    reported_rows
                    if reported_rows is not None
                    else _count_microbatch_delta_rows(context=context, targets=targets)
                )
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.DML,
            error=exc,
            error_prefix=f"batch {batch.index}",
            staging_relation=targets.delta_qualified,
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    return batch_rows


def _count_microbatch_delta_rows(
    *, context: ModelMaterializationContext, targets: MicrobatchTargets
) -> int | None:
    """Return the staged row count used for completion provenance."""

    cursor: Any = context.adapter.execute(
        connection=context.connection,
        sql=f"SELECT COUNT(*) FROM {targets.delta_qualified}",
    )
    row: Any = cursor.fetchone()
    return None if row is None or row[0] is None else int(row[0])


def _complete_microbatch_batch(
    *,
    context: ModelMaterializationContext,
    window_text: str,
    targets: MicrobatchTargets,
    state: MicrobatchLifecycleState,
    replaced_next: bool,
) -> None:
    """Finish one batch; its delta stays only when the next batch's delta replaces it."""

    if not (replaced_next and context.adapter.create_table_as_replaces()):
        with diagnostics_context(
            sqlbuild_phase="cleanup",
            sqlbuild_action_name="drop_delta",
            sqlbuild_window=window_text,
        ):
            context.adapter.drop_table(
                connection=context.connection,
                destination=targets.delta_qualified,
                statement_recorder=state.statement_recorder,
            )
    log_debug_event(
        logger=_DEBUG_LOGGER,
        message="",
        sqlbuild_subject="model",
        sqlbuild_name=context.entry.name,
        sqlbuild_event="batch_complete",
        sqlbuild_phase="batch",
        sqlbuild_window=window_text,
        sqlbuild_status="ok",
    )


def _prepare_full_refresh_rebuild(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    relations: FullRefreshRelations,
) -> ModelExecutionResult | None:
    """Inspect or initialize the deterministic full-refresh rebuild relation."""

    try:
        live_exists: bool = relation_exists(
            adapter=context.adapter,
            connection=context.connection,
            database=context.entry.destination.database,
            schema=context.entry.destination.schema,
            name=relations.target_name,
        )
        rebuild_exists: bool = relation_exists(
            adapter=context.adapter,
            connection=context.connection,
            database=context.entry.destination.database,
            schema=context.entry.destination.schema,
            name=relations.rebuild_name,
        )
        if not live_exists and rebuild_exists:
            context.adapter.rename(
                connection=context.connection,
                origin=relations.rebuild_qualified,
                destination=relations.target_qualified,
                statement_recorder=state.statement_recorder,
            )
        elif rebuild_exists:
            context.adapter.drop_table(
                connection=context.connection,
                destination=relations.rebuild_qualified,
                statement_recorder=state.statement_recorder,
            )
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.STAGING,
            error=exc,
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    return None


def _promote_microbatch_full_refresh(
    *,
    context: ModelMaterializationContext,
    state: MicrobatchLifecycleState,
    relations: FullRefreshRelations,
) -> ModelExecutionResult | None:
    """Promote a complete rebuild while retaining the outgoing generation."""

    try:
        live_exists: bool = relation_exists(
            adapter=context.adapter,
            connection=context.connection,
            database=context.entry.destination.database,
            schema=context.entry.destination.schema,
            name=relations.target_name,
        )
        promote_full_refresh_rebuild(
            adapter=context.adapter,
            connection=context.connection,
            relations=relations,
            target_exists=live_exists,
            statement_recorder=state.statement_recorder,
            bound_views=context.bound_views,
        )
    except Exception as exc:
        return build_failed_result(
            entry=context.entry,
            phase=ExecutionPhase.DML,
            error=exc,
            error_prefix="failed to promote full-refresh rebuild",
            staging_relation=relations.rebuild_qualified,
            promoted_relation=relations.target_qualified,
            warnings=state.warnings,
            audit_results=state.audit_results,
            statement_recorder=state.statement_recorder,
        )
    return None


def _context_with_microbatch_range(
    *, context: ModelMaterializationContext, batch_plan: _MicrobatchPlan
) -> ModelMaterializationContext:
    if batch_plan.resolved_range is None:
        return context
    return replace(
        context,
        entry=replace(context.entry, microbatch_range=batch_plan.resolved_range),
    )


def _plan_microbatch_windows(
    *,
    context: ModelMaterializationContext,
    is_full_refresh: bool,
    target_qualified: str,
    warnings: list[str],
    audit_results: list[AuditExecutionResult],
    statement_recorder: StatementRecorder,
    on_progress: Callable[[str], None] | None,
) -> _MicrobatchPlan:
    """Resolve the microbatch cursor range and compute batch windows."""

    entry: ModelPlanEntry = context.entry
    adapter: BaseAdapter = context.adapter
    connection: Any = context.connection
    runtime_owned_cursor_bounds: bool = has_runtime_owned_cursor_watermarks(
        entry.cursor_input_relations
    )
    effective_grain: str | None = resolve_effective_timestamp_grain(
        cursor_type=entry.cursor_type,
        downstream_grain=entry.cursor_grain,
        cursor_input_relations=entry.cursor_input_relations,
        microbatch_strategy=entry.microbatch_strategy,
    )
    has_authoritative_override: bool = has_authoritative_cursor_override(entry=entry)
    runtime_discovery: bool = not has_authoritative_override and (
        runtime_owned_cursor_bounds
        or (is_full_refresh and entry.microbatch_strategy != MicrobatchStrategy.ROLLING_WINDOW)
    )
    microbatch_range: CursorBounds | None = entry.microbatch_range
    empty_cursor_inputs: tuple[str, ...] = entry.empty_cursor_inputs
    waiting_on_empty_inputs: bool = entry.waiting_on_empty_inputs
    if runtime_discovery:
        if entry.cursor_column is None:
            return _MicrobatchPlan(
                early_exit=build_failed_result(
                    entry=entry,
                    phase=ExecutionPhase.STAGING,
                    error="runtime-owned cursor resolution requires cursor_column",
                    warnings=warnings,
                    audit_results=audit_results,
                    statement_recorder=statement_recorder,
                )
            )
        try:
            microbatch_range = resolve_runtime_cursor_bounds(
                adapter=adapter,
                connection=connection,
                target_relation=target_qualified,
                target_database=entry.destination.database,
                target_schema=entry.destination.schema,
                target_name=entry.destination.name,
                spec=build_runtime_cursor_spec(
                    entry=entry,
                    read_destination_cursor=not is_full_refresh,
                ),
                on_progress=on_progress,
                watermark_resolver=context.watermark_resolver,
            )
        except EmptyCursorInputsError as exc:
            if (
                is_full_refresh
                and entry.microbatch_strategy == MicrobatchStrategy.WATERMARK
                and cached_relation_exists(
                    adapter=adapter,
                    connection=connection,
                    database=entry.destination.database,
                    schema=entry.destination.schema,
                    name=entry.destination.name,
                )
            ):
                return _MicrobatchPlan(
                    early_exit=build_failed_result(
                        entry=entry,
                        phase=ExecutionPhase.STAGING,
                        error=empty_input_rebuild_error(entry=entry, input_names=exc.input_names),
                        warnings=warnings,
                        audit_results=audit_results,
                        statement_recorder=statement_recorder,
                    )
                )
            microbatch_range = None
            empty_cursor_inputs = exc.input_names
            waiting_on_empty_inputs = exc.waiting_on_empty_inputs
        except Exception as exc:
            return _MicrobatchPlan(
                early_exit=build_failed_result(
                    entry=entry,
                    phase=ExecutionPhase.STAGING,
                    error=exc,
                    warnings=warnings,
                    audit_results=audit_results,
                    statement_recorder=statement_recorder,
                )
            )
    batch_size: str | None = entry.batch_size
    cursor_type: str | None = entry.cursor_type
    if batch_size is None or cursor_type is None:
        return _MicrobatchPlan(
            early_exit=build_failed_result(
                entry=entry,
                phase=ExecutionPhase.STAGING,
                error="microbatch requires batch_size and cursor_type",
                warnings=warnings,
                audit_results=audit_results,
                statement_recorder=statement_recorder,
            )
        )
    if microbatch_range is None:
        empty_bound: CursorScalar = _empty_microbatch_bound(entry=entry)
        microbatch_range = CursorBounds(start=empty_bound, end=empty_bound)

    effective_batch_size: str = batch_size
    if effective_grain is not None:
        effective_batch_size = resolve_effective_microbatch_batch_size(
            batch_size=batch_size,
            effective_grain=effective_grain,
        )
    work_intervals: tuple[MicrobatchInterval, ...] = merge_microbatch_intervals(
        intervals=(
            (
                MicrobatchInterval(
                    start=sentinel_to_token(sentinel=microbatch_range.start),
                    end=sentinel_to_token(sentinel=microbatch_range.end),
                ),
            )
            if microbatch_range.start != microbatch_range.end
            else ()
        ),
        cursor_type=cursor_type,
    )
    computed_batches: tuple[BatchWindow, ...] = _batches_for_intervals(
        intervals=work_intervals,
        batch_size=effective_batch_size,
        cursor_type=cursor_type,
    )
    batches: tuple[BatchWindow, ...] = (
        (
            BatchWindow(
                start=sentinel_to_token(sentinel=microbatch_range.start),
                end=sentinel_to_token(sentinel=microbatch_range.end),
                index=0,
            ),
        )
        if is_full_refresh
        and microbatch_range.start == microbatch_range.end
        and sentinel_to_token(sentinel=microbatch_range.start) != entry.cursor_end
        else tuple(replace(batch, index=index) for index, batch in enumerate(computed_batches))
    )
    resolved_range: CursorBounds = (
        replace(
            microbatch_range,
            start=work_intervals[0].start,
            end=work_intervals[-1].end,
        )
        if work_intervals
        else microbatch_range
    )

    return _MicrobatchPlan(
        batches=batches,
        effective_batch_size=effective_batch_size,
        resolved_range=resolved_range,
        runtime_discovery=runtime_discovery,
        empty_cursor_inputs=empty_cursor_inputs,
        waiting_on_empty_inputs=waiting_on_empty_inputs,
    )


def _batches_for_intervals(
    *, intervals: tuple[MicrobatchInterval, ...], batch_size: str, cursor_type: str
) -> tuple[BatchWindow, ...]:
    batches: list[BatchWindow] = []
    for interval in intervals:
        batches.extend(
            compute_batch_windows(
                start=parse(raw=interval.start, cursor_type=cursor_type),
                end=parse(raw=interval.end, cursor_type=cursor_type),
                batch_size=batch_size,
                cursor_type=cursor_type,
            )
        )
    return tuple(batches)


def _empty_microbatch_bound(*, entry: ModelPlanEntry) -> CursorScalar:
    return _concrete_bound(
        bound=empty_cursor_window(
            cursor_type=entry.cursor_type, cursor_start=entry.cursor_start
        ).start
    )


def _concrete_bound(*, bound: Bound) -> CursorScalar:
    if isinstance(bound, BoundSentinel):
        raise ExecutorInputError("microbatch execution requires concrete cursor bounds")
    return bound


def _format_resolved_microbatch_progress(
    *,
    bounds: CursorBounds,
    batch_count: int,
    batch_size: str,
    cursor_type: str | None,
    cursor_grain: str | None,
    strategy: str,
    reason: str,
    limit: int | None,
    intervals: tuple[MicrobatchInterval, ...],
) -> str:
    """Format the concrete runtime-owned range before its first batch starts."""

    start: str = cursor_bound_display(
        value=sentinel_to_token(sentinel=bounds.start),
        cursor_type=cursor_type,
        cursor_grain=cursor_grain,
    )
    end: str = inclusive_cursor_end(
        end=sentinel_to_token(sentinel=bounds.end),
        cursor_type=cursor_type,
        cursor_grain=cursor_grain,
    )
    batch_noun: str = "batch" if batch_count == 1 else "batches"
    limit_text: str = "none" if limit is None else str(limit)
    interval_text: str = ", ".join(f"[{item.start}, {item.end})" for item in intervals)
    return (
        f"runtime plan resolved: strategy {strategy}; range {start} -> {end}; "
        f"intervals {interval_text or 'none'}; reason {reason}; "
        f"{batch_count} {batch_noun} x {batch_size}; limit {limit_text}"
    )


def compute_batch_windows(
    *,
    start: CursorScalar | str,
    end: CursorScalar | str,
    batch_size: str,
    cursor_type: str,
) -> tuple[BatchWindow, ...]:
    """Split a cursor range into ordered batch windows."""

    start_value: CursorScalar = (
        parse(raw=start, cursor_type=cursor_type) if isinstance(start, str) else start
    )
    end_value: CursorScalar = (
        parse(raw=end, cursor_type=cursor_type) if isinstance(end, str) else end
    )
    if cursor_type == CursorType.TIMESTAMP:
        return _compute_timestamp_batches(start=start_value, end=end_value, batch_size=batch_size)
    if cursor_type == CursorType.INTEGER:
        return _compute_integer_batches(start=start_value, end=end_value, batch_size=batch_size)
    return ()


def _format_batch_window_for_display(*, batch: BatchWindow, entry: ModelPlanEntry) -> str:
    """Render a batch window for progress output, collapsing whole-day bounds to dates."""

    start: str = cursor_bound_display(
        value=render(value=batch.start),
        cursor_type=entry.cursor_type,
        cursor_grain=entry.cursor_grain,
    )
    end: str = cursor_bound_display(
        value=render(value=batch.end),
        cursor_type=entry.cursor_type,
        cursor_grain=entry.cursor_grain,
    )
    return f"{start}..{end}"


def _substitute_sentinels(
    *,
    sql: str,
    batch_start: CursorScalar,
    batch_end: CursorScalar,
) -> str:
    """Replace microbatch cursor sentinels with concrete batch bounds."""

    return substitute_cursor_sentinels(
        sql=sql,
        bounds=CursorBounds(start=batch_start, end=batch_end),
    )


def _validate_cursor_output_columns(
    *,
    entry: ModelPlanEntry,
    delta_columns: tuple[ColumnInfo, ...],
) -> None:
    """Validate cursor-based microbatch DML can address the model output cursor."""

    if entry.incremental_strategy != IncrementalStrategy.DELETE_INSERT:
        return
    cursor_column: str | None = entry.cursor_column
    if cursor_column is None:
        return
    delta_names: frozenset[str] = frozenset(
        column_name_key(column.name) for column in delta_columns
    )
    if column_name_key(cursor_column) in delta_names:
        require_quoted_spelling(
            model_name=entry.name,
            setting="cursor",
            configured=(cursor_column,),
            listed_columns=delta_columns,
        )
        return
    raise ExecutorInputError(
        f"microbatch cursor column '{cursor_column}' is not produced by model output; "
        "use cursor_inputs filter roles for upstream cursor columns and set cursor to the "
        "target output cursor column"
    )


def _compute_timestamp_batches(
    *,
    start: CursorScalar,
    end: CursorScalar,
    batch_size: str,
) -> tuple[BatchWindow, ...]:
    """Split a timestamp range into batch windows by duration."""

    duration: Duration | None = Duration.parse(batch_size)
    if duration is None:
        return ()

    if not isinstance(start, TimestampValue | DateValue) or not isinstance(
        end, TimestampValue | DateValue
    ):
        return ()
    start_dt, end_dt = aligned_timestamp_bounds(start=start, end=end)

    if start_dt >= end_dt:
        return ()

    windows: list[BatchWindow] = []
    index: int = 0
    current: datetime = start_dt
    while current < end_dt:
        batch_end: datetime = min(duration.add_to(current), end_dt)
        windows.append(
            BatchWindow(
                start=TimestampValue(value=current),
                end=TimestampValue(value=batch_end),
                index=index,
            )
        )
        current = batch_end
        index += 1

    return tuple(windows)


def _compute_integer_batches(
    *,
    start: CursorScalar,
    end: CursorScalar,
    batch_size: str,
) -> tuple[BatchWindow, ...]:
    """Split an integer range into batch windows."""

    if not isinstance(start, IntegerValue) or not isinstance(end, IntegerValue):
        return ()
    try:
        start_int: int = start.value
        end_int: int = end.value
        size_int: int = int(Decimal(batch_size))
    except (InvalidOperation, ValueError, OverflowError):
        return ()

    if size_int <= 0 or start_int >= end_int:
        return ()

    interval: AlignedInterval = AlignedInterval(
        start=IntegerValue(value=start_int), end=IntegerValue(value=end_int), grain=None
    )
    batches: tuple[AlignedInterval, ...] = split_into_batches(interval=interval, step=size_int)
    return tuple(
        BatchWindow(start=batch.start, end=batch.end, index=index)
        for index, batch in enumerate(batches)
    )
