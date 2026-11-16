"""Shared pre/post hook lifecycle phases for model materializations."""

from __future__ import annotations

from collections.abc import Mapping

from sqlbuild._native import ModelPlanEntry
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.diagnostics.main.diagnostics_context import diagnostics_context
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.run._helpers.execution.hooks import execute_hooks, render_hooks
from sqlbuild.executor.run._helpers.execution.results import (
    build_failed_result,
    build_skipped_result,
)
from sqlbuild.executor.run.models import (
    HookExecutionResult,
    HookRelationLookup,
    HookRunContext,
    ModelExecutionResult,
    ModelMaterializationContext,
    PostHookPhaseOutcome,
)
from sqlbuild.executor.run.types import ExecutionPhase, HookPhase


def run_pre_hook_phase(
    *,
    context: ModelMaterializationContext,
    warnings: list[str],
    audit_results: list[AuditExecutionResult],
    hook_results: list[HookExecutionResult],
    statement_recorder: StatementRecorder,
) -> ModelExecutionResult | None:
    """Run model pre-hooks and return an early-exit result, or None to continue."""

    entry: ModelPlanEntry = context.entry
    try:
        statement_recorder.record_many(
            render_hooks(hooks=entry.pre_hooks, phase=HookPhase.PRE_HOOKS)
        )
        with diagnostics_context(sqlbuild_phase="pre_hook", sqlbuild_action_name="run"):
            pre_hook_skipped: bool = execute_hooks(
                connection=context.connection,
                adapter=context.adapter,
                hooks=entry.pre_hooks,
                phase=HookPhase.PRE_HOOKS,
                hook_functions=context.hook_functions,
                hook_results=hook_results,
                hook_run=build_model_hook_run(
                    context=context, statement_recorder=statement_recorder, warnings=warnings
                ),
            )
        if pre_hook_skipped:
            return build_skipped_result(
                entry=entry,
                warnings=warnings,
                audit_results=audit_results,
                statement_recorder=statement_recorder,
                hook_results=hook_results,
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
    return None


def run_post_hook_phase(
    *,
    context: ModelMaterializationContext,
    warnings: list[str],
    audit_results: list[AuditExecutionResult],
    hook_results: list[HookExecutionResult],
    statement_recorder: StatementRecorder,
    staging_relation: str | None = None,
    promoted_relation: str | None = None,
) -> PostHookPhaseOutcome:
    """Run model post-hooks and return the skip/failure phase outcome."""

    entry: ModelPlanEntry = context.entry
    try:
        statement_recorder.record_many(
            render_hooks(hooks=entry.post_hooks, phase=HookPhase.POST_HOOKS)
        )
        with diagnostics_context(sqlbuild_phase="post_hook", sqlbuild_action_name="run"):
            post_hook_skipped: bool = execute_hooks(
                connection=context.connection,
                adapter=context.adapter,
                hooks=entry.post_hooks,
                phase=HookPhase.POST_HOOKS,
                hook_functions=context.hook_functions,
                hook_results=hook_results,
                hook_run=build_model_hook_run(
                    context=context, statement_recorder=statement_recorder, warnings=warnings
                ),
            )
    except Exception as exc:
        return PostHookPhaseOutcome(
            failure=build_failed_result(
                entry=entry,
                phase=ExecutionPhase.POST_HOOK,
                error=exc,
                staging_relation=staging_relation,
                promoted_relation=promoted_relation,
                warnings=warnings,
                audit_results=audit_results,
                statement_recorder=statement_recorder,
                hook_results=hook_results,
            )
        )
    return PostHookPhaseOutcome(skipped=post_hook_skipped)


def build_model_hook_run(
    *,
    context: ModelMaterializationContext,
    statement_recorder: StatementRecorder,
    warnings: list[str],
    effective_vars: Mapping[str, object] | None = None,
) -> HookRunContext:
    """Build the hook run context for one model's lifecycle hooks."""

    return HookRunContext(
        model_name=context.entry.name,
        destination=context.entry.destination,
        run_id=context.run_id,
        target=context.effective_target_name,
        target_config=context.target_config,
        effective_vars=context.effective_vars if effective_vars is None else effective_vars,
        statement_recorder=statement_recorder,
        providers=context.providers,
        python_identity_recorder=context.python_identity_recorder,
        relation_lookup=HookRelationLookup(
            model_locations=context.model_locations,
            seed_locations=context.seed_locations,
            source_map=context.python_source_read_map,
        ),
        warnings=warnings,
        enforce_explicit_references=context.enforce_explicit_references,
    )
