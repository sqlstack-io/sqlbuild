"""Single-model table execution lifecycle."""

from __future__ import annotations

from dataclasses import replace
from typing import Any

from sqlbuild._native import (
    PROJECT_CONFIG_FILENAME,
    SETTINGS_SECTION,
    STAGED_PROMOTION_MODE,
    TABLE_PROMOTION_MODE_SETTING_KEY,
    Fingerprint,
    ModelPlanEntry,
    setting_help,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.diagnostics.main.diagnostics_context import diagnostics_context
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.run._helpers.execution.final_audits import run_final_model_audits
from sqlbuild.executor.run._helpers.execution.hook_phases import (
    run_post_hook_phase,
    run_pre_hook_phase,
)
from sqlbuild.executor.run._helpers.execution.promotion import promote_relation_to_destination
from sqlbuild.executor.run._helpers.execution.results import (
    build_failed_result,
    build_skipped_result,
)
from sqlbuild.executor.run._helpers.execution.results import (
    with_waiting_on_empty_inputs_warning as with_waiting_on_empty_inputs_warning,
)
from sqlbuild.executor.run._helpers.execution.schema import inspect_runtime_relation_schema
from sqlbuild.executor.run._helpers.execution.staging import create_staging_relation
from sqlbuild.executor.run._helpers.execution.table_config import model_table_config
from sqlbuild.executor.run._helpers.execution.table_targets import resolve_table_targets
from sqlbuild.executor.run._helpers.materializations.custom import (
    execute_custom_entry as execute_custom_entry,
)
from sqlbuild.executor.run._helpers.materializations.dynamic import (
    execute_dynamic_entry as execute_dynamic_entry,
)
from sqlbuild.executor.run._helpers.materializations.incremental import (
    execute_incremental_entry as execute_incremental_entry,
)
from sqlbuild.executor.run._helpers.materializations.microbatch import (
    execute_microbatch_entry as execute_microbatch_entry,
)
from sqlbuild.executor.run._helpers.materializations.snapshot import (
    execute_snapshot_entry as execute_snapshot_entry,
)
from sqlbuild.executor.run._helpers.materializations.table_cursor import (
    resolve_table_cursor,
    result_with_cursor_safety,
)
from sqlbuild.executor.run._helpers.materializations.view import (
    execute_view_entry as execute_view_entry,
)
from sqlbuild.executor.run._helpers.reuse.fingerprinting import try_write_fingerprint
from sqlbuild.executor.run._helpers.validation.contracts import validate_runtime_contract
from sqlbuild.executor.run._helpers.validation.type_enforcement import enforce_types_staged
from sqlbuild.executor.run.models import (
    FinalAuditRun,
    HookExecutionResult,
    ModelExecutionResult,
    ModelMaterializationContext,
    PostHookPhaseOutcome,
    TableCursorResolution,
    TableLifecycleState,
    TableTargets,
)
from sqlbuild.executor.run.types import ExecutionPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus


def execute_table_entry(
    *,
    context: ModelMaterializationContext,
    declared_columns: tuple[ColumnInfo, ...],
    promotion_mode: TablePromotionMode,
    is_full_refresh: bool = False,
) -> ModelExecutionResult:
    """Execute one table model through its full materialization lifecycle."""

    entry: ModelPlanEntry = context.entry
    adapter: BaseAdapter = context.adapter
    connection: Any = context.connection
    targets: TableTargets = resolve_table_targets(adapter=adapter, entry=entry)
    warnings: list[str] = []
    audit_results: list[AuditExecutionResult] = []
    hook_results: list[HookExecutionResult] = []
    statement_recorder: StatementRecorder = StatementRecorder()
    try:
        cursor_resolution: TableCursorResolution = resolve_table_cursor(
            context=context,
            targets=targets,
            is_full_refresh=is_full_refresh,
        )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.STAGING,
            error=exc,
            error_prefix="failed to resolve runtime cursor bounds",
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )
    if cursor_resolution.bounds is not None:
        entry = replace(entry, cursor_bounds=cursor_resolution.bounds)
        context = replace(context, entry=entry)
    if cursor_resolution.empty_cursor_inputs:
        entry = replace(
            entry,
            empty_cursor_inputs=cursor_resolution.empty_cursor_inputs,
            waiting_on_empty_inputs=cursor_resolution.waiting_on_empty_inputs,
        )
        context = replace(context, entry=entry)
    if cursor_resolution.warning is not None:
        warnings.append(cursor_resolution.warning)

    try:
        if not context.schema_prepared:
            adapter.ensure_schema(
                connection=connection,
                database=targets.target_database,
                schema=targets.target_schema,
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

    state: TableLifecycleState = TableLifecycleState(
        warnings=warnings,
        audit_results=audit_results,
        statement_recorder=statement_recorder,
        hook_results=hook_results,
        resolved_sql=cursor_resolution.resolved_sql,
    )

    if promotion_mode == TablePromotionMode.STAGED:
        result: ModelExecutionResult = _staged_lifecycle(
            context=context,
            targets=targets,
            state=state,
            declared_columns=declared_columns,
        )
        return result_with_cursor_safety(result=result, entry=entry)

    if entry.contract_enforced:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.CONTRACT,
            error=ExecutorInputError(
                f"model '{entry.name}': contract enforced requires staged table promotion; "
                "immediate table promotion cannot validate runtime output before target mutation",
                code="K011",
            ),
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    result = _immediate_lifecycle(
        context=context,
        targets=targets,
        state=state,
        declared_columns=declared_columns,
    )
    return result_with_cursor_safety(result=result, entry=entry)


def _staged_lifecycle(
    *,
    context: ModelMaterializationContext,
    targets: TableTargets,
    state: TableLifecycleState,
    declared_columns: tuple[ColumnInfo, ...],
) -> ModelExecutionResult:
    """Staged table lifecycle: CTAS staging, type enforce, audit, promote."""

    entry: ModelPlanEntry = context.entry
    adapter: BaseAdapter = context.adapter
    connection: Any = context.connection
    target_qualified: str = targets.target_qualified
    staging_qualified: str = targets.staging_qualified
    warnings: list[str] = state.warnings
    audit_results: list[AuditExecutionResult] = state.audit_results
    statement_recorder: StatementRecorder = state.statement_recorder
    hook_results: list[HookExecutionResult] = state.hook_results
    try:
        with diagnostics_context(
            sqlbuild_phase="materialize", sqlbuild_action_name="create_staging"
        ):
            reuse_origin_fingerprint: Fingerprint | None = create_staging_relation(
                context=context,
                staging_qualified=staging_qualified,
                resolved_sql=state.resolved_sql,
                statement_recorder=statement_recorder,
            )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.STAGING,
            error=exc,
            staging_relation=staging_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    if entry.type_enforcement and declared_columns:
        try:
            with diagnostics_context(
                sqlbuild_phase="type_enforcement", sqlbuild_action_name="rebuild_staging"
            ):
                enforce_types_staged(
                    adapter=adapter,
                    connection=connection,
                    staging_qualified=staging_qualified,
                    staging_database=targets.target_database,
                    staging_schema=targets.target_schema,
                    staging_table=targets.staging_table,
                    declared_columns=declared_columns,
                    table_config={
                        "table_type": entry.table_type,
                        "physical_storage": entry.physical_storage,
                        "change_tracking": bool(entry.dynamic_readers),
                    },
                    statement_recorder=statement_recorder,
                )
        except Exception as exc:
            return build_failed_result(
                entry=entry,
                phase=ExecutionPhase.TYPE_ENFORCEMENT,
                error=exc,
                staging_relation=staging_qualified,
                warnings=warnings,
                audit_results=audit_results,
                statement_recorder=statement_recorder,
            )

    try:
        with diagnostics_context(
            sqlbuild_phase="contract", sqlbuild_action_name="validate_staging"
        ):
            staging_columns: tuple[ColumnInfo, ...] = (
                ()
                if not entry.contract_enforced
                else inspect_runtime_relation_schema(
                    adapter=adapter,
                    connection=connection,
                    database=targets.target_database,
                    schema=targets.target_schema,
                    name=targets.staging_table,
                )
            )
            validate_runtime_contract(
                entry=entry,
                actual_columns=staging_columns,
                dialect=adapter.sql_analysis_dialect_name,
            )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.CONTRACT,
            error=exc,
            staging_relation=staging_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    final_audit_run: FinalAuditRun = run_final_model_audits(
        relation_overrides={entry.name: staging_qualified},
        model_audits=context.model_audits,
        reuse_origin_fingerprint=reuse_origin_fingerprint,
        adapter=adapter,
        connection=connection,
        model_locations=context.model_locations,
        seed_locations=context.seed_locations,
        source_map=context.source_map,
    )
    audit_results.extend(final_audit_run.results)

    if final_audit_run.has_error:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.AUDIT,
            error=(
                f"final audit for '{entry.name}' failed before replacing target table "
                "with severity level: error"
            ),
            staging_relation=staging_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
        )

    try:
        with diagnostics_context(sqlbuild_phase="promote", sqlbuild_action_name="check_existing"):
            _ = promote_relation_to_destination(
                adapter=adapter,
                connection=connection,
                origin_relation=staging_qualified,
                destination_relation=target_qualified,
                destination_database=targets.target_database,
                destination_schema=targets.target_schema,
                destination_name=targets.target_table,
                statement_recorder=statement_recorder,
                bound_views=context.bound_views,
            )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.PROMOTION,
            error=exc,
            staging_relation=staging_qualified,
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
        promoted_relation=target_qualified,
    )
    if post_hook_outcome.failure is not None:
        return post_hook_outcome.failure
    if post_hook_outcome.skipped:
        return build_skipped_result(
            entry=entry,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
            promoted_relation=target_qualified,
        )

    fingerprint_warnings: tuple[str, ...] = try_write_fingerprint(
        entry=entry,
        adapter=adapter,
        connection=connection,
        run_id=context.run_id,
        query_change_tracking=context.query_change_tracking,
        fingerprint_writes=context.fingerprint_writes,
        model_audits=context.model_audits,
        audit_results=tuple(audit_results),
    )

    return ModelExecutionResult(
        model_name=entry.name,
        status=ExecutionStatus.SUCCESS,
        promoted_relation=target_qualified,
        audit_results=tuple(audit_results),
        warning_messages=tuple(warnings) + fingerprint_warnings,
        lifecycle_events=statement_recorder.snapshot(),
        hook_results=tuple(hook_results),
    )


def _immediate_lifecycle(
    *,
    context: ModelMaterializationContext,
    targets: TableTargets,
    state: TableLifecycleState,
    declared_columns: tuple[ColumnInfo, ...],
) -> ModelExecutionResult:
    """Immediate table lifecycle: CTAS target, audit after, no staging."""

    entry: ModelPlanEntry = context.entry
    adapter: BaseAdapter = context.adapter
    connection: Any = context.connection
    target_qualified: str = targets.target_qualified
    warnings: list[str] = state.warnings
    audit_results: list[AuditExecutionResult] = state.audit_results
    statement_recorder: StatementRecorder = state.statement_recorder
    hook_results: list[HookExecutionResult] = state.hook_results
    reuse_origin_fingerprint: Fingerprint | None = None
    if entry.type_enforcement and declared_columns:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.TYPE_ENFORCEMENT,
            error=(
                f"model '{entry.name}': type enforcement requires staged promotion mode "
                "for runtime column inspection, and [settings] table_promotion_mode is not "
                '"staged"; '
                + setting_help(
                    purpose="to inspect columns before promotion",
                    file_name=PROJECT_CONFIG_FILENAME,
                    section=SETTINGS_SECTION,
                    key=TABLE_PROMOTION_MODE_SETTING_KEY,
                    value=STAGED_PROMOTION_MODE,
                )
            ),
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    try:
        with diagnostics_context(sqlbuild_phase="materialize", sqlbuild_action_name="create_table"):
            adapter.create_table_as(
                connection=connection,
                destination=target_qualified,
                sql=state.resolved_sql,
                config=model_table_config(entry),
                statement_recorder=statement_recorder,
            )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.STAGING,
            error=exc,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    immediate_audit_run: FinalAuditRun = run_final_model_audits(
        relation_overrides=None,
        model_audits=context.model_audits,
        reuse_origin_fingerprint=reuse_origin_fingerprint,
        adapter=adapter,
        connection=connection,
        model_locations=context.model_locations,
        seed_locations=context.seed_locations,
        source_map=context.source_map,
    )
    audit_results.extend(immediate_audit_run.results)

    if immediate_audit_run.has_error:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.AUDIT,
            error=(
                f"final audit for '{entry.name}' failed after target table was replaced "
                "with severity level: error"
            ),
            promoted_relation=target_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    post_hook_outcome: PostHookPhaseOutcome = run_post_hook_phase(
        context=context,
        warnings=warnings,
        audit_results=audit_results,
        hook_results=hook_results,
        statement_recorder=statement_recorder,
        promoted_relation=target_qualified,
    )
    if post_hook_outcome.failure is not None:
        return post_hook_outcome.failure
    if post_hook_outcome.skipped:
        return build_skipped_result(
            entry=entry,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
            promoted_relation=target_qualified,
        )

    fingerprint_warnings: tuple[str, ...] = try_write_fingerprint(
        entry=entry,
        adapter=adapter,
        connection=connection,
        run_id=context.run_id,
        query_change_tracking=context.query_change_tracking,
        fingerprint_writes=context.fingerprint_writes,
        model_audits=context.model_audits,
        audit_results=tuple(audit_results),
    )

    return ModelExecutionResult(
        model_name=entry.name,
        status=ExecutionStatus.SUCCESS,
        promoted_relation=target_qualified,
        audit_results=tuple(audit_results),
        warning_messages=tuple(warnings) + fingerprint_warnings,
        lifecycle_events=statement_recorder.snapshot(),
        hook_results=tuple(hook_results),
    )
