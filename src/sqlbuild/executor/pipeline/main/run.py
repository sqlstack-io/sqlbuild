"""Executor pipeline orchestration for build execution."""

from __future__ import annotations

import time
from dataclasses import replace
from typing import Any

from sqlbuild._native import BuildPhaseTimingTracker, CostContext, PlanOutput, SettingsConfig
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.diagnostics.main.diagnostics_context import diagnostics_context
from sqlbuild.executor.build.main._execute import execute_build_plan
from sqlbuild.executor.build.models import (
    BuildCallbacks,
    BuildCustomizations,
    BuildExecutionResult,
    BuildExecutionTimings,
    BuildInitialState,
    BuildRuntimeParams,
)
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.pipeline._helpers.auditing import (
    run_audit_pipeline as run_audit_pipeline,
)
from sqlbuild.executor.pipeline._helpers.auditing import (
    run_audit_pipeline_with_callbacks as run_audit_pipeline_with_callbacks,
)
from sqlbuild.executor.pipeline._helpers.connections import (
    close_connections,
    prepare_build_connections,
)
from sqlbuild.executor.pipeline._helpers.graph_width import runnable_graph_width
from sqlbuild.executor.pipeline._helpers.scenario import (
    decide_scenario_run_on as decide_scenario_run_on,
)
from sqlbuild.executor.pipeline._helpers.scenario import (
    run_scenario_capture_pipeline as run_scenario_capture_pipeline,
)
from sqlbuild.executor.pipeline._helpers.scenario import (
    run_scenario_local_test_pipeline as run_scenario_local_test_pipeline,
)
from sqlbuild.executor.pipeline._helpers.scenario import (
    run_scenario_test_pipeline as run_scenario_test_pipeline,
)
from sqlbuild.executor.pipeline._helpers.scenario import (
    select_scenario_snapshot_capture_candidates as select_scenario_snapshot_capture_candidates,
)
from sqlbuild.executor.pipeline._helpers.seeding import (
    run_seed_pipeline as run_seed_pipeline,
)
from sqlbuild.executor.pipeline._helpers.settings import plannable_plan, resolve_build_inputs
from sqlbuild.executor.pipeline._helpers.testing import (
    run_test_pipeline as run_test_pipeline,
)
from sqlbuild.executor.pipeline.models import (
    AuditPipelineCallbacks as AuditPipelineCallbacks,
)
from sqlbuild.executor.pipeline.models import (
    BuildConnectionPreparation,
    ResolvedBuildInputs,
)
from sqlbuild.runtime.observability.classes.event_dispatcher import EventDispatcher
from sqlbuild.runtime.observability.main.create_lifecycle_event import create_lifecycle_event
from sqlbuild.runtime.observability.main.current_event_dispatcher import current_event_dispatcher
from sqlbuild.runtime.observability.main.run_scope import run_scope


def run_build_pipeline(
    *,
    plan: PlanOutput,
    connection_config: dict[str, object],
    adapter: BaseAdapter,
    settings: SettingsConfig,
    runtime: BuildRuntimeParams,
    callbacks: BuildCallbacks | None = None,
    customizations: BuildCustomizations | None = None,
    initial_state: BuildInitialState | None = None,
) -> BuildExecutionResult:
    """Execute a full build pipeline: resolve settings, open connections, run plan, close."""

    plan = plannable_plan(plan)
    with run_scope(runtime.run_id) as identity:
        dispatcher: EventDispatcher | None = current_event_dispatcher()
        started: float = time.monotonic()
        if dispatcher is not None:
            dispatcher.publish_lifecycle(
                create_lifecycle_event(event_type="run_started", payload={"run_kind": "build"})
            )
        with diagnostics_context(
            sqlbuild_invocation_id=identity.invocation_id,
            sqlbuild_run_id=identity.run_id,
        ):
            with CostContext.scope(
                run_id=runtime.run_id,
                resource_type="run",
                resource_name=runtime.target,
                ledger_path=(
                    runtime.runtime_dir / "executions" / runtime.run_id / "statements.jsonl"
                ),
                phase="build",
                on_statement_complete=(
                    None if callbacks is None else callbacks.on_statement_complete
                ),
            ):
                try:
                    result: BuildExecutionResult = _run_build_pipeline(
                        plan=plan,
                        connection_config=connection_config,
                        adapter=adapter,
                        settings=settings,
                        runtime=runtime,
                        callbacks=callbacks,
                        customizations=customizations,
                        initial_state=initial_state,
                    )
                except BaseException as error:
                    if dispatcher is not None:
                        dispatcher.publish_lifecycle(
                            create_lifecycle_event(
                                event_type="run_failed",
                                payload={
                                    "run_kind": "build",
                                    "duration_ms": int((time.monotonic() - started) * 1000),
                                    "error_type": type(error).__name__,
                                },
                            )
                        )
                    raise
                if dispatcher is not None:
                    event_type: str = (
                        "run_completed" if result.status == BuildStatus.SUCCESS else "run_failed"
                    )
                    dispatcher.publish_lifecycle(
                        create_lifecycle_event(
                            event_type=event_type,
                            payload={
                                "run_kind": "build",
                                "duration_ms": int((time.monotonic() - started) * 1000),
                                "succeeded_count": result.success_count,
                                "failed_count": result.failure_count,
                                "skipped_count": result.skipped_count,
                            },
                        )
                    )
                return result


def _run_build_pipeline(
    *,
    plan: PlanOutput,
    connection_config: dict[str, object],
    adapter: BaseAdapter,
    settings: SettingsConfig,
    runtime: BuildRuntimeParams,
    callbacks: BuildCallbacks | None = None,
    customizations: BuildCustomizations | None = None,
    initial_state: BuildInitialState | None = None,
) -> BuildExecutionResult:
    """Run the build while the public wrapper owns telemetry context."""

    inputs: ResolvedBuildInputs = resolve_build_inputs(
        settings=settings,
        adapter=adapter,
        runtime=runtime,
        callbacks=callbacks,
        customizations=customizations,
        initial_state=initial_state,
    )
    effective_concurrency: int = min(
        max(1, runtime.max_concurrency), runnable_graph_width(plan=plan)
    )
    preparation: BuildConnectionPreparation = prepare_build_connections(
        plan=plan,
        adapter=adapter,
        connection_config=connection_config,
        connection_count=effective_concurrency,
        callbacks=inputs.callbacks,
    )
    _ = _publish_execution_start(
        concurrency=effective_concurrency,
        connections=len(preparation.worker_connections) + 1,
    )
    timing_tracker: BuildPhaseTimingTracker | None = BuildPhaseTimingTracker.current()
    execution_error: BaseException | None = None
    execution_start: float = time.monotonic()
    try:
        result: BuildExecutionResult = execute_build_plan(
            plan=plan,
            adapter=adapter,
            connection_config=connection_config,
            connections=preparation.worker_connections,
            scheduler_connection=preparation.scheduler_connection,
            runtime=inputs.runtime,
            callbacks=inputs.callbacks,
            customizations=inputs.customizations,
            initial_state=inputs.initial_state,
            schema_prepared=True,
        )
        execution_seconds: float = time.monotonic() - execution_start
        return replace(
            result,
            timings=BuildExecutionTimings(
                connection_preparation_seconds=preparation.connection_seconds,
                schema_preparation_seconds=preparation.schema_seconds,
                execution_seconds=execution_seconds,
            ),
        )
    except BaseException as error:
        execution_error = error
        raise
    finally:
        execution_seconds = time.monotonic() - execution_start
        if timing_tracker is not None:
            timing_tracker.execution_seconds = execution_seconds
        cleanup_connections: tuple[Any, ...] = preparation.worker_connections + (
            preparation.scheduler_connection,
        )
        _ = close_connections(
            adapter=adapter,
            connections=cleanup_connections,
            active_error=execution_error,
        )


def _publish_execution_start(*, concurrency: int, connections: int) -> None:
    dispatcher: EventDispatcher | None = current_event_dispatcher()
    if dispatcher is not None:
        dispatcher.publish_lifecycle(
            create_lifecycle_event(
                event_type="invocation_phase",
                payload={
                    "phase": "started_execution",
                    "count": concurrency,
                    "detail": f"{connections} connections",
                },
            )
        )
