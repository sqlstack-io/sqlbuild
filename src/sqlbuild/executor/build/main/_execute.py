"""Build execution orchestration over a planned execution schedule."""

from __future__ import annotations

from contextlib import AbstractContextManager, nullcontext
from dataclasses import replace
from typing import Any

from sqlbuild._native import PlanOutput, RetentionPlanPhase, aggregate_build_result
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.relations.main.relation_metadata_cache_scope import (
    relation_metadata_cache_scope,
)
from sqlbuild.compiler.auditing.types import AuditOutcome
from sqlbuild.executor.auditing.main._project_results import (
    AuditResultProjection,
    project_audit_result_batch,
)
from sqlbuild.executor.auditing.main.audit_result_publication_scope import (
    audit_result_publication_scope,
)
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.build._helpers.retention import (
    apply_retention_phase,
    reconcile_retention_after_build,
)
from sqlbuild.executor.build.classes.build_scheduler import BuildScheduler
from sqlbuild.executor.build.models import (
    BuildCallbacks,
    BuildCustomizations,
    BuildExecutionResult,
    BuildInitialState,
    BuildRuntimeParams,
    FunctionExecutionResult,
    SeedExecutionResult,
)
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.maintained_pauses.classes.maintained_hold import MaintainedHold
from sqlbuild.executor.maintained_pauses.main.hold_maintained_models import (
    hold_maintained_models,
)
from sqlbuild.executor.migrations.main._apply import apply_model_migrations
from sqlbuild.executor.migrations.main._apply_columns import apply_column_migrations
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.testing.models import SqlTestExecutionResult


def execute_build_plan(
    *,
    plan: PlanOutput,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connections: tuple[Any, ...],
    scheduler_connection: Any,
    runtime: BuildRuntimeParams,
    callbacks: BuildCallbacks | None = None,
    customizations: BuildCustomizations | None = None,
    initial_state: BuildInitialState | None = None,
    schema_prepared: bool = False,
) -> BuildExecutionResult:
    """Execute a build plan with the DAG scheduler, holding the maintained models it touches."""

    hold: MaintainedHold | None = hold_maintained_models(
        plan=plan, adapter=adapter, runtime=runtime, connection_config=connection_config
    )
    held: AbstractContextManager[object] = (
        nullcontext()
        if hold is None
        else hold.holding(
            connection=scheduler_connection,
            on_progress=None if callbacks is None else callbacks.on_lifecycle_progress,
        )
    )
    with relation_metadata_cache_scope(adapter=adapter), held:
        return _execute_build_plan(
            plan=plan,
            adapter=adapter,
            connection_config=connection_config,
            connections=connections,
            scheduler_connection=scheduler_connection,
            runtime=replace(runtime, maintained_hold=hold),
            callbacks=callbacks,
            customizations=customizations,
            initial_state=initial_state,
            schema_prepared=schema_prepared,
        )


def _execute_build_plan(
    *,
    plan: PlanOutput,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connections: tuple[Any, ...],
    scheduler_connection: Any,
    runtime: BuildRuntimeParams,
    callbacks: BuildCallbacks | None,
    customizations: BuildCustomizations | None,
    initial_state: BuildInitialState | None,
    schema_prepared: bool,
) -> BuildExecutionResult:
    resolved_callbacks: BuildCallbacks = callbacks if callbacks is not None else BuildCallbacks()
    resolved_customizations: BuildCustomizations = (
        customizations if customizations is not None else BuildCustomizations()
    )
    resolved_initial_state: BuildInitialState = (
        initial_state if initial_state is not None else BuildInitialState()
    )
    scheduler: BuildScheduler = BuildScheduler(
        plan=plan,
        adapter=adapter,
        connection_config=connection_config,
        connections=connections,
        scheduler_connection=scheduler_connection,
        runtime=runtime,
        callbacks=resolved_callbacks,
        customizations=resolved_customizations,
        initial_state=resolved_initial_state,
        schema_prepared=schema_prepared,
    )

    _ = apply_model_migrations(
        plan=plan,
        adapter=adapter,
        connection=scheduler_connection,
        run_id=runtime.run_id,
        on_progress=resolved_callbacks.on_lifecycle_progress,
    )
    _ = apply_column_migrations(
        plan=plan,
        adapter=adapter,
        connection=scheduler_connection,
        run_id=runtime.run_id,
        on_progress=resolved_callbacks.on_lifecycle_progress,
    )
    _ = apply_retention_phase(
        plan=plan,
        adapter=adapter,
        connection=scheduler_connection,
        phase=RetentionPlanPhase.PRE,
    )

    model_results: tuple[ModelExecutionResult, ...]
    seed_results: tuple[SeedExecutionResult, ...]
    function_results: tuple[FunctionExecutionResult, ...]
    load_results: tuple[LoadExecutionResult, ...]
    test_results: tuple[SqlTestExecutionResult, ...]
    source_audit_results: tuple[AuditExecutionResult, ...]
    end_audit_results: tuple[AuditExecutionResult, ...]
    with audit_result_publication_scope(plan=plan):
        (
            model_results,
            seed_results,
            function_results,
            load_results,
            test_results,
            source_audit_results,
            end_audit_results,
        ) = scheduler.run()
        all_audit_results: tuple[AuditExecutionResult, ...] = _all_audit_results(
            model_results=model_results,
            source_audit_results=source_audit_results,
            end_audit_results=end_audit_results,
        )
        projection: AuditResultProjection = project_audit_result_batch(
            plan=plan,
            results=all_audit_results,
            adapter=adapter,
            connection=scheduler_connection,
        )

    result: BuildExecutionResult = aggregate_build_result(
        model_results=model_results,
        seed_results=seed_results,
        function_results=function_results,
        load_results=load_results,
        test_results=test_results,
        source_audit_results=source_audit_results,
        end_audit_results=end_audit_results,
    )
    result = replace(
        result,
        audit_result_projection_attempted_count=projection.attempted_count,
        audit_result_projection_written_count=projection.written_count,
        audit_result_projection_failed_count=projection.failed_count,
    )
    if result.status == BuildStatus.SUCCESS:
        _ = reconcile_retention_after_build(
            plan=plan,
            adapter=adapter,
            connection=scheduler_connection,
        )
    return result


def _all_audit_results(
    *,
    model_results: tuple[ModelExecutionResult, ...],
    source_audit_results: tuple[AuditExecutionResult, ...],
    end_audit_results: tuple[AuditExecutionResult, ...],
) -> tuple[AuditExecutionResult, ...]:
    results: list[AuditExecutionResult] = [*source_audit_results, *end_audit_results]
    for model_result in model_results:
        results.extend(model_result.audit_results)
    return tuple(result for result in results if result.outcome != AuditOutcome.SKIPPED)
