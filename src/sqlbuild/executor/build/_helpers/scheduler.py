"""Concurrent build scheduler dispatch helpers."""

from __future__ import annotations

from collections.abc import Callable, Mapping

from sqlbuild._native import (
    MICROBATCH_END_SENTINEL,
    MICROBATCH_START_SENTINEL,
    CompiledObjectKey,
    IncrementalMode,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
    SnapshotsConfig,
    assert_no_unresolved_sql_markers,
    replaces_incremental_relation,
    resolve_cursor_intrinsics,
)
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.build.constants import (
    BUILD_CUSTOM_MATERIALIZATION_MISSING_CODE,
    BUILD_SOURCE_FRESHNESS_BLOCKED_CODE,
    DYNAMIC_ACTIONS,
    INCREMENTAL_ACTIONS,
)
from sqlbuild.executor.build.models import (
    FunctionExecutionResult,
    NodeCompletion,
    SeedExecutionResult,
)
from sqlbuild.executor.custom.models import MaterializationResult
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.run.main._execute import (
    execute_custom_entry,
    execute_dynamic_entry,
    execute_incremental_entry,
    execute_microbatch_entry,
    execute_snapshot_entry,
    execute_table_entry,
    execute_view_entry,
    with_waiting_on_empty_inputs_warning,
)
from sqlbuild.executor.run.models import ModelExecutionResult, ModelMaterializationContext
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.executor.testing.models import SqlTestExecutionResult
from sqlbuild.runtime.contracts.types import ExecutionResourceKind
from sqlbuild.runtime.observability.classes.statement_lifecycle import StatementLifecycle
from sqlbuild.runtime.observability.constants import MODEL_BUILD_PURPOSE

type _BuildWorkerResult = (
    ModelExecutionResult
    | SeedExecutionResult
    | FunctionExecutionResult
    | SqlTestExecutionResult
    | LoadExecutionResult
)


def _build_worker_success_completion(
    *, key: CompiledObjectKey, result: _BuildWorkerResult
) -> NodeCompletion:
    return NodeCompletion(key=key, result=result)


def _dispatch_model(
    *,
    context: ModelMaterializationContext,
    promotion_mode: TablePromotionMode,
    snapshots: SnapshotsConfig,
    allow_snapshot_schema_change: bool,
    custom_materializations: Mapping[str, Callable[..., MaterializationResult]] | None = None,
    target: str = "",
    effective_vars: dict[str, object] | None = None,
    warehouse_relations: dict[str, RelationInfo] | None = None,
    on_progress: Callable[[str], None] | None = None,
) -> ModelExecutionResult:
    """Route a model to the correct executor, then report a stalled cursor window as a warning."""

    with StatementLifecycle.purpose_scope(MODEL_BUILD_PURPOSE):
        return with_waiting_on_empty_inputs_warning(
            _dispatch_model_entry(
                context=context,
                promotion_mode=promotion_mode,
                snapshots=snapshots,
                allow_snapshot_schema_change=allow_snapshot_schema_change,
                custom_materializations=custom_materializations,
                target=target,
                effective_vars=effective_vars,
                warehouse_relations=warehouse_relations,
                on_progress=on_progress,
            )
        )


def _dispatch_model_entry(
    *,
    context: ModelMaterializationContext,
    promotion_mode: TablePromotionMode,
    snapshots: SnapshotsConfig,
    allow_snapshot_schema_change: bool,
    custom_materializations: Mapping[str, Callable[..., MaterializationResult]] | None = None,
    target: str = "",
    effective_vars: dict[str, object] | None = None,
    warehouse_relations: dict[str, RelationInfo] | None = None,
    on_progress: Callable[[str], None] | None = None,
) -> ModelExecutionResult:
    """Route a model to the correct executor based on action and mode."""

    entry: ModelPlanEntry = context.entry
    if entry.action == PlanAction.SKIP:
        return ModelExecutionResult(
            model_name=entry.name,
            status=ExecutionStatus.SKIPPED,
            skip_reason=_plan_skip_reason(entry),
            error_code=_plan_skip_error_code(entry),
        )

    _assert_model_sql_ready_for_execution(entry)

    if entry.action == PlanAction.CUSTOM:
        mat_name: str | None = entry.custom_materialization_name
        registry: Mapping[str, Callable[..., MaterializationResult]] = custom_materializations or {}
        if mat_name is None or mat_name not in registry:
            return ModelExecutionResult(
                model_name=entry.name,
                status=ExecutionStatus.FAILED,
                error_code=BUILD_CUSTOM_MATERIALIZATION_MISSING_CODE,
                error_message=f"custom materialization '{mat_name}' not found in registry",
            )
        existing: RelationInfo | None = (warehouse_relations or {}).get(entry.name)
        return execute_custom_entry(
            context=context,
            declared_columns=entry.declared_columns,
            materialize_fn=registry[mat_name],
            target=target,
            effective_vars=effective_vars or {},
            existing_relation=existing,
            on_progress=on_progress,
        )

    is_microbatch: bool = entry.incremental_mode == IncrementalMode.MICROBATCH
    is_full_refresh_microbatch: bool = is_microbatch and replaces_incremental_relation(
        materialization_type=entry.materialization_type, action=entry.action
    )

    if is_microbatch and entry.action in INCREMENTAL_ACTIONS:
        return execute_microbatch_entry(
            context=context,
            declared_columns=entry.declared_columns,
            on_progress=on_progress,
        )
    if is_full_refresh_microbatch:
        return execute_microbatch_entry(
            context=context,
            declared_columns=entry.declared_columns,
            is_full_refresh=True,
            on_progress=on_progress,
        )
    if entry.action in INCREMENTAL_ACTIONS:
        return execute_incremental_entry(
            context=context,
            declared_columns=entry.declared_columns,
        )
    if entry.action in DYNAMIC_ACTIONS:
        return execute_dynamic_entry(context=context)
    if entry.action == PlanAction.CREATE_VIEW:
        return execute_view_entry(context=context)
    if entry.action == PlanAction.SNAPSHOT:
        return execute_snapshot_entry(
            context=context,
            snapshots=snapshots,
            allow_snapshot_schema_change=allow_snapshot_schema_change,
        )
    return execute_table_entry(
        context=context,
        declared_columns=entry.declared_columns,
        promotion_mode=promotion_mode,
        is_full_refresh=entry.reason == PlanReason.FULL_REFRESH,
    )


def _assert_model_sql_ready_for_execution(entry: ModelPlanEntry) -> None:
    """Reject compiler markers immediately before materialization dispatch."""

    context: str = f"Model '{entry.name}' executable SQL"
    assert_no_unresolved_sql_markers(sql=entry.resolved_sql, context=context)
    _, has_intrinsics = resolve_cursor_intrinsics(sql=entry.resolved_sql)
    if has_intrinsics:
        raise ExecutorInputError(f"{context} contains unresolved cursor intrinsics")
    has_cursor_markers: bool = (
        MICROBATCH_START_SENTINEL in entry.resolved_sql
        or MICROBATCH_END_SENTINEL in entry.resolved_sql
    )
    runtime_resolved: bool = entry.incremental_mode == IncrementalMode.MICROBATCH or any(
        relation.is_runtime_owned for relation in entry.cursor_input_relations
    )
    if has_cursor_markers and not runtime_resolved:
        raise ExecutorInputError(f"{context} contains unresolved cursor markers")


def _plan_skip_reason(entry: ModelPlanEntry) -> str:
    if entry.reason == PlanReason.SOURCE_FRESHNESS_ERROR:
        return "Blocked by source freshness error"
    return entry.reason.value


def _plan_skip_error_code(entry: ModelPlanEntry) -> str | None:
    if entry.reason == PlanReason.SOURCE_FRESHNESS_ERROR:
        return BUILD_SOURCE_FRESHNESS_BLOCKED_CODE
    return None


def _model_execution_resource_kind(materialization_type: str) -> ExecutionResourceKind:
    if materialization_type == MaterializationType.VIEW:
        return ExecutionResourceKind.VIEW
    if materialization_type == MaterializationType.CUSTOM:
        return ExecutionResourceKind.CUSTOM
    if materialization_type == MaterializationType.SNAPSHOT:
        return ExecutionResourceKind.SNAPSHOT
    return ExecutionResourceKind.TABLE
