"""Execution result construction helpers."""

from __future__ import annotations

import logging
from dataclasses import replace

from sqlbuild._native import (
    CursorBounds,
    FutureCursorSafetyEvidence,
    MaximumStartSafetyEvidence,
    ModelPlanEntry,
    error_code,
    error_help,
    error_message,
    format_waiting_on_empty_inputs,
)
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.diagnostics.main.log_debug_event import log_debug_event
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.run.constants import (
    RUN_AUDIT_FAILED_CODE,
    RUN_CONTRACT_FAILED_CODE,
    RUN_CUSTOM_MATERIALIZATION_FAILED_CODE,
    RUN_DML_FAILED_CODE,
    RUN_FINGERPRINT_FAILED_CODE,
    RUN_POST_HOOK_FAILED_CODE,
    RUN_PRE_HOOK_FAILED_CODE,
    RUN_PROMOTION_FAILED_CODE,
    RUN_SCHEMA_CHANGE_FAILED_CODE,
    RUN_STAGING_FAILED_CODE,
    RUN_TYPE_ENFORCEMENT_FAILED_CODE,
    RUN_UNKNOWN_FAILED_CODE,
)
from sqlbuild.executor.run.models import (
    HookExecutionResult,
    ModelExecutionResult,
    ModelMaterializationContext,
    TableTargets,
)
from sqlbuild.executor.run.types import ExecutionPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus

_DEBUG_LOGGER: logging.Logger = logging.getLogger("sqlbuild.execution")


def build_failed_result(
    *,
    entry: ModelPlanEntry,
    phase: ExecutionPhase,
    error: str | BaseException,
    error_prefix: str | None = None,
    staging_relation: str | None = None,
    promoted_relation: str | None = None,
    warnings: list[str],
    audit_results: list[AuditExecutionResult],
    statement_recorder: StatementRecorder,
    hook_results: list[HookExecutionResult] | None = None,
) -> ModelExecutionResult:
    """Build a failed ModelExecutionResult for a specific phase."""

    rendered_error, rendered_code, rendered_help = _render_failure_error(
        error=error, fallback_code=_fallback_code_for_phase(phase)
    )
    if error_prefix is not None:
        rendered_error = f"{error_prefix}: {rendered_error}"

    statement_recorder.log(f"model {entry.name} failed phase={phase.value} error={rendered_error}")
    if staging_relation is not None:
        statement_recorder.log(f"staging relation kept for inspection: {staging_relation}")

    return ModelExecutionResult(
        model_name=entry.name,
        status=ExecutionStatus.FAILED,
        failed_phase=phase,
        staging_relation=staging_relation,
        promoted_relation=promoted_relation,
        audit_results=tuple(audit_results),
        warning_messages=tuple(warnings),
        lifecycle_events=statement_recorder.snapshot(),
        hook_results=tuple(hook_results or ()),
        error_code=rendered_code,
        error_help=rendered_help,
        error_message=rendered_error,
        failed_sql=_failed_sql(error),
        future_cursor_safety=_future_cursor_safety(entry),
        maximum_start_safety=_maximum_start_safety(entry=entry, error=error),
        microbatch_limit=entry.microbatch_limit,
        microbatch_limit_count=entry.microbatch_limit_count,
        microbatch_limit_action=entry.microbatch_limit_action,
        microbatch_limit_warning=entry.microbatch_limit_warning,
        microbatch_strategy=entry.microbatch_strategy,
        microbatch_plan_reason=entry.reason.value,
    )


def _failed_sql(error: str | BaseException) -> str | None:
    if isinstance(error, str):
        return None
    failed_sql: object = getattr(error, "failed_sql", None)
    return failed_sql if isinstance(failed_sql, str) and failed_sql else None


def build_skipped_result(
    *,
    entry: ModelPlanEntry,
    warnings: list[str],
    audit_results: list[AuditExecutionResult],
    statement_recorder: StatementRecorder,
    hook_results: list[HookExecutionResult],
    staging_relation: str | None = None,
    promoted_relation: str | None = None,
) -> ModelExecutionResult:
    """Build a skipped ModelExecutionResult after a lifecycle skip signal."""

    skipped_hook: HookExecutionResult | None = _last_skipped_hook(hook_results)
    statement_recorder.log(f"model {entry.name} skipped by hook")
    return ModelExecutionResult(
        model_name=entry.name,
        status=ExecutionStatus.SKIPPED,
        staging_relation=staging_relation,
        promoted_relation=promoted_relation,
        audit_results=tuple(audit_results),
        warning_messages=tuple(warnings),
        lifecycle_events=statement_recorder.snapshot(),
        hook_results=tuple(hook_results),
        skip_mode=skipped_hook.skip_mode if skipped_hook is not None else None,
        skip_reason=skipped_hook.skip_reason if skipped_hook is not None else None,
        future_cursor_safety=_future_cursor_safety(entry),
        maximum_start_safety=_maximum_start_safety(entry=entry, error=None),
        microbatch_limit=entry.microbatch_limit,
        microbatch_limit_count=entry.microbatch_limit_count,
        microbatch_limit_action=entry.microbatch_limit_action,
        microbatch_limit_warning=entry.microbatch_limit_warning,
    )


def _future_cursor_safety(entry: ModelPlanEntry) -> FutureCursorSafetyEvidence | None:
    bounds: CursorBounds | None = entry.microbatch_range or entry.cursor_bounds
    return bounds.future_safety if bounds is not None else None


def _maximum_start_safety(
    *, entry: ModelPlanEntry, error: str | BaseException | None
) -> MaximumStartSafetyEvidence | None:
    bounds: CursorBounds | None = entry.microbatch_range or entry.cursor_bounds
    if bounds is not None and bounds.maximum_start_safety is not None:
        return bounds.maximum_start_safety
    evidence: object | None = getattr(error, "evidence", None)
    return evidence if isinstance(evidence, MaximumStartSafetyEvidence) else None


def _last_skipped_hook(
    hook_results: list[HookExecutionResult],
) -> HookExecutionResult | None:
    hook_result: HookExecutionResult
    for hook_result in reversed(hook_results):
        if hook_result.status == ExecutionStatus.SKIPPED:
            return hook_result
    return None


def _render_failure_error(
    *, error: str | BaseException, fallback_code: str
) -> tuple[str, str | None, str | None]:
    """Render legacy string failures and structured exceptions with a code."""

    if isinstance(error, str):
        return error, fallback_code, None
    return (
        error_message(error),
        error_code(error=error, fallback_code=fallback_code),
        error_help(error),
    )


def _fallback_code_for_phase(phase: ExecutionPhase) -> str:
    """Return the run diagnostic code for a failure phase."""

    if phase == ExecutionPhase.PRE_HOOK:
        return RUN_PRE_HOOK_FAILED_CODE
    if phase == ExecutionPhase.STAGING:
        return RUN_STAGING_FAILED_CODE
    if phase == ExecutionPhase.SCHEMA_CHANGE:
        return RUN_SCHEMA_CHANGE_FAILED_CODE
    if phase == ExecutionPhase.TYPE_ENFORCEMENT:
        return RUN_TYPE_ENFORCEMENT_FAILED_CODE
    if phase == ExecutionPhase.CONTRACT:
        return RUN_CONTRACT_FAILED_CODE
    if phase == ExecutionPhase.AUDIT:
        return RUN_AUDIT_FAILED_CODE
    if phase == ExecutionPhase.PROMOTION:
        return RUN_PROMOTION_FAILED_CODE
    if phase == ExecutionPhase.DML:
        return RUN_DML_FAILED_CODE
    if phase == ExecutionPhase.POST_HOOK:
        return RUN_POST_HOOK_FAILED_CODE
    if phase == ExecutionPhase.FINGERPRINT:
        return RUN_FINGERPRINT_FAILED_CODE
    if phase == ExecutionPhase.CUSTOM_MATERIALIZATION:
        return RUN_CUSTOM_MATERIALIZATION_FAILED_CODE
    return RUN_UNKNOWN_FAILED_CODE


def with_waiting_on_empty_inputs_warning(result: ModelExecutionResult) -> ModelExecutionResult:
    """Count a cursor window stalled on empty inputs as a model warning."""

    if (
        result.status != ExecutionStatus.SUCCESS
        or not result.waiting_on_empty_inputs
        or not result.empty_cursor_inputs
    ):
        return result
    message: str = format_waiting_on_empty_inputs(result.empty_cursor_inputs)
    if message in result.warning_messages:
        return result
    return replace(result, warning_messages=(*result.warning_messages, message))


def rebuilt_table_rows(
    *, context: ModelMaterializationContext, targets: TableTargets
) -> int | None:
    """Exact rows of a freshly rebuilt table with no post-hooks; a failed read gives None."""

    if context.entry.post_hooks:
        return None
    try:
        return context.adapter.rebuilt_table_row_count(
            connection=context.connection,
            database=targets.target_database,
            schema=targets.target_schema,
            name=targets.target_table,
        )
    except Exception as error:
        log_debug_event(
            logger=_DEBUG_LOGGER,
            message="rebuilt table row count unavailable",
            sqlbuild_subject="model",
            sqlbuild_name=context.entry.name,
            sqlbuild_event="row_count_unavailable",
            sqlbuild_error_type=type(error).__name__,
        )
        return None
