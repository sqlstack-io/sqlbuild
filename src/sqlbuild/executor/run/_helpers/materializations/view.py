"""View materialization lifecycle."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from typing import Any

from sqlbuild._native import (
    AuditPlanEntry,
    CompiledRelationLocation,
    DiscoveredHookFunction,
    ModelPlanEntry,
    SourceEntry,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.types import RelationType
from sqlbuild.adapter.relations.main.cached_relation_type import cached_relation_type
from sqlbuild.adapter.relations.main.resolve_relation_location_qualified_name import (
    resolve_relation_location_qualified_name,
)
from sqlbuild.compiler.auditing.types import AuditRunScope
from sqlbuild.diagnostics.main.diagnostics_context import diagnostics_context
from sqlbuild.executor.auditing.main._execute import execute_audit
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.run._helpers.execution.hook_phases import build_model_hook_run
from sqlbuild.executor.run._helpers.execution.hooks import execute_hooks, render_hooks
from sqlbuild.executor.run._helpers.execution.results import (
    build_failed_result,
    build_skipped_result,
)
from sqlbuild.executor.run._helpers.reuse.fingerprinting import try_write_fingerprint
from sqlbuild.executor.run.models import (
    HookExecutionResult,
    ModelExecutionResult,
    ModelMaterializationContext,
)
from sqlbuild.executor.run.types import ExecutionPhase, HookPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus


def execute_view_entry(
    *,
    context: ModelMaterializationContext,
) -> ModelExecutionResult:
    """Execute one view model through its full materialization lifecycle."""

    def materialize(*, target_qualified: str, statement_recorder: StatementRecorder) -> None:
        _create_view_replacing_table(
            adapter=context.adapter,
            connection=context.connection,
            entry=context.entry,
            target_qualified=target_qualified,
            statement_recorder=statement_recorder,
        )

    return execute_in_place_entry(
        context=context, materialize=materialize, action_name="create_view", built="view creation"
    )


def execute_in_place_entry(
    *,
    context: ModelMaterializationContext,
    materialize: Callable[..., None],
    action_name: str,
    built: str,
) -> ModelExecutionResult:
    """Hooks, schema, `materialize` on the destination, final audits and the fingerprint."""

    entry: ModelPlanEntry = context.entry
    adapter: BaseAdapter = context.adapter
    connection: Any = context.connection
    model_locations: dict[str, CompiledRelationLocation] = context.model_locations
    seed_locations: dict[str, CompiledRelationLocation] = context.seed_locations
    source_map: dict[str, SourceEntry] = context.source_map
    model_audits: tuple[AuditPlanEntry, ...] = context.model_audits
    run_id: str = context.run_id
    query_change_tracking: bool = context.query_change_tracking
    hook_functions: tuple[DiscoveredHookFunction, ...] = context.hook_functions
    effective_vars: Mapping[str, object] | None = context.effective_vars
    target_database: str | None = entry.destination.database
    target_schema: str | None = entry.destination.schema
    target_qualified: str = resolve_relation_location_qualified_name(
        adapter=adapter, location=entry.destination
    )
    warnings: list[str] = []
    audit_results: list[AuditExecutionResult] = []
    hook_results: list[HookExecutionResult] = []
    statement_recorder: StatementRecorder = StatementRecorder()

    try:
        statement_recorder.record_many(
            render_hooks(hooks=entry.pre_hooks, phase=HookPhase.PRE_HOOKS)
        )
        with diagnostics_context(sqlbuild_phase="pre_hook", sqlbuild_action_name="run"):
            pre_hook_skipped: bool = execute_hooks(
                connection=connection,
                adapter=adapter,
                hooks=entry.pre_hooks,
                phase=HookPhase.PRE_HOOKS,
                hook_functions=hook_functions,
                hook_results=hook_results,
                hook_run=build_model_hook_run(
                    context=context,
                    statement_recorder=statement_recorder,
                    warnings=warnings,
                    effective_vars=effective_vars,
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

    try:
        if not context.schema_prepared:
            adapter.ensure_schema(
                connection=connection,
                database=target_database,
                schema=target_schema,
                statement_recorder=statement_recorder,
            )
        with diagnostics_context(sqlbuild_phase="materialize", sqlbuild_action_name=action_name):
            materialize(target_qualified=target_qualified, statement_recorder=statement_recorder)
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

    audit_error: bool = False
    audit: AuditPlanEntry
    for audit in model_audits:
        result: AuditExecutionResult = execute_audit(
            audit=audit,
            adapter=adapter,
            connection=connection,
            model_locations=model_locations,
            seed_locations=seed_locations,
            source_map=source_map,
            relation_overrides=None,
            run_scope_phase=AuditRunScope.FINAL,
            quality_scope="final",
        )
        audit_results.append(result)
        if result.blocks:
            audit_error = True

    if audit_error:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.AUDIT,
            error=(
                f"final audit for '{entry.name}' failed after {built} with severity level: error"
            ),
            promoted_relation=target_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    try:
        statement_recorder.record_many(
            render_hooks(hooks=entry.post_hooks, phase=HookPhase.POST_HOOKS)
        )
        with diagnostics_context(sqlbuild_phase="post_hook", sqlbuild_action_name="run"):
            post_hook_skipped: bool = execute_hooks(
                connection=connection,
                adapter=adapter,
                hooks=entry.post_hooks,
                phase=HookPhase.POST_HOOKS,
                hook_functions=hook_functions,
                hook_results=hook_results,
                hook_run=build_model_hook_run(
                    context=context,
                    statement_recorder=statement_recorder,
                    warnings=warnings,
                    effective_vars=effective_vars,
                ),
            )
        if post_hook_skipped:
            return build_skipped_result(
                entry=entry,
                warnings=warnings,
                audit_results=audit_results,
                statement_recorder=statement_recorder,
                hook_results=hook_results,
                promoted_relation=target_qualified,
            )
    except Exception as exc:
        return build_failed_result(
            entry=entry,
            phase=ExecutionPhase.POST_HOOK,
            error=exc,
            promoted_relation=target_qualified,
            warnings=warnings,
            audit_results=audit_results,
            statement_recorder=statement_recorder,
            hook_results=hook_results,
        )

    warnings.extend(
        try_write_fingerprint(
            entry=entry,
            adapter=adapter,
            connection=connection,
            run_id=run_id,
            query_change_tracking=query_change_tracking,
            model_audits=model_audits,
            audit_results=tuple(audit_results),
            fingerprint_writes=context.fingerprint_writes,
        )
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


def _create_view_replacing_table(
    *,
    adapter: BaseAdapter,
    connection: Any,
    entry: ModelPlanEntry,
    target_qualified: str,
    statement_recorder: StatementRecorder,
) -> None:
    """Create the view; a table the model built before it became a view is dropped first."""

    try:
        adapter.create_view_as(
            connection=connection,
            destination=target_qualified,
            sql=entry.resolved_sql,
            statement_recorder=statement_recorder,
        )
    except Exception as error:
        if not _holds_table(adapter=adapter, connection=connection, entry=entry):
            raise error
        adapter.drop(
            connection=connection,
            destination=target_qualified,
            if_exists=True,
            statement_recorder=statement_recorder,
        )
        adapter.create_view_as(
            connection=connection,
            destination=target_qualified,
            sql=entry.resolved_sql,
            statement_recorder=statement_recorder,
        )


def _holds_table(*, adapter: BaseAdapter, connection: Any, entry: ModelPlanEntry) -> bool:
    """Whether the view's destination holds a relation other than a view."""

    existing: RelationType | None = cached_relation_type(
        adapter=adapter,
        connection=connection,
        database=entry.destination.database,
        schema=entry.destination.schema,
        name=entry.destination.name,
    )
    return existing is not None and existing != RelationType.VIEW
