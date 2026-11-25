"""Scenario test execution pipeline."""

from __future__ import annotations

import atexit
import json
import logging
import queue
import sys
import threading
import time
from collections.abc import Callable, Iterator
from concurrent.futures import Future, wait
from contextlib import contextmanager
from contextvars import Context, copy_context
from dataclasses import dataclass, field, replace
from functools import partial
from pathlib import Path
from typing import Any, ClassVar, Protocol

from sqlbuild._native import (
    CompiledSqlScenario,
    CompilePipelineResult,
    Dialect,
    NativeProgressProjector,
    ScenarioExecutionPlan,
    SqlLexicalSyntax,
    current_native_progress_projector,
    error_code,
    error_help,
    error_message,
    scenario_local_type_overrides_for_dialect,
)
from sqlbuild._native import build_cli_scenario_plan as build_scenario_plan
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.adapter.relations.main.relation_metadata_cache_scope import (
    relation_metadata_cache_scope,
)
from sqlbuild.diagnostics.main.log_debug_event import log_debug_event
from sqlbuild.executor.local.constants import (
    RUN_ON_LOCAL,
    RUN_ON_WAREHOUSE,
    STATEMENT_CONSTRUCT,
)
from sqlbuild.executor.local.main._convert_local_sql import session_time_zone
from sqlbuild.executor.pipeline._helpers.connections import (
    close_connections,
    open_worker_connections,
)
from sqlbuild.executor.pipeline._helpers.scenario_timeouts import (
    scenario_timeout,
    timed_out_scenario_result,
)
from sqlbuild.executor.pipeline._helpers.settings import resolve_promotion_mode
from sqlbuild.executor.pipeline.main.raise_warehouse_connection_failure import (
    raise_warehouse_connection_failure,
)
from sqlbuild.executor.scenario.classes.prepared_scenario_schemas import PreparedScenarioSchemas
from sqlbuild.executor.scenario.classes.running_scenarios import RunningScenarios
from sqlbuild.executor.scenario.classes.scenario_interrupts import ScenarioInterrupts
from sqlbuild.executor.scenario.constants import (
    SCENARIO_EXEC_INTERNAL,
    SCENARIO_LOCAL_INTERNAL,
)
from sqlbuild.executor.scenario.main._capture_steps import (
    execute_scenario_snapshot_capture_run,
)
from sqlbuild.executor.scenario.main._cleanup_interrupted import cleanup_interrupted_scenarios
from sqlbuild.executor.scenario.main._local import execute_local_scenario_load_only_run
from sqlbuild.executor.scenario.main._local_refusals import local_scenario_refusals
from sqlbuild.executor.scenario.main._run import execute_scenario_run
from sqlbuild.executor.scenario.main._snapshots import classify_scenario_snapshot_state
from sqlbuild.executor.scenario.models import (
    LocalConversion,
    ScenarioCaptureSettings,
    ScenarioLocalReplaySource,
    ScenarioRunOptions,
    ScenarioRunResult,
    ScenarioSnapshotCaptureRunResult,
    ScenarioSnapshotStateResult,
    ScenarioTestSettings,
)
from sqlbuild.executor.scenario.types import ScenarioLocalRunStatus, ScenarioSnapshotState
from sqlbuild.executor.scheduling.classes.recovering_connection_pool import RecoveringConnectionPool
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.runtime.contracts.models import ConnectionHooks
from sqlbuild.runtime.execution_limits.classes.connection_recovery import ConnectionRecovery
from sqlbuild.runtime.execution_limits.main.item_deadline_scope import item_deadline_scope
from sqlbuild.runtime.execution_limits.models import ItemTimeoutPolicy
from sqlbuild.runtime.execution_limits.types import ItemKind
from sqlbuild.runtime.observability.classes.operation_lifecycle import OperationLifecycle
from sqlbuild.runtime.observability.classes.resource_attempt_lifecycle import (
    ResourceAttemptLifecycle,
)

_DEBUG_LOGGER: logging.Logger = logging.getLogger("sqlbuild.execution")
_STOP_POLL_SECONDS: float = 0.05
_WORKER_EXIT_JOIN_SECONDS: float = 5.0
_WORKER_EXIT_NOTICE: str = "Waiting up to 5s for running scenario statements to stop...\n"
_WORKER_NAME_PREFIX: str = "sqlbuild-scenario-"
_SCENARIO_INTERNAL_ERROR_HELP: str = (
    "This is likely a SQLBuild bug. Please file an issue with the scenario name."
)


@dataclass(frozen=True)
class _ScenarioScheduling:
    worker_count: int = 1
    interrupts: ScenarioInterrupts = field(default_factory=ScenarioInterrupts)
    running: RunningScenarios | None = None


@dataclass(frozen=True)
class _ScenarioOutcome[ResultT]:
    scenario: CompiledSqlScenario
    scenario_plan: ScenarioExecutionPlan | None
    result: ResultT
    resource_attempt_id: str


def _scenario_failure_help(exc: Exception) -> str | None:
    explicit_help: str | None = error_help(exc)
    if explicit_help is not None:
        return explicit_help
    if getattr(exc, "code", None) is not None:
        return None
    return _SCENARIO_INTERNAL_ERROR_HELP


class _ScenarioPipelineResult(Protocol):
    __dataclass_fields__: ClassVar[dict[str, Any]]

    @property
    def status(self) -> ExecutionStatus: ...

    @property
    def error_code(self) -> str | None: ...


class _ScenarioFailureResult[ResultT](Protocol):
    def __call__(self, *, scenario_name: str, exc: Exception) -> ResultT: ...


def run_scenario_test_pipeline(
    *,
    pipeline_result: CompilePipelineResult,
    scenarios: tuple[CompiledSqlScenario, ...],
    connection_config: dict[str, object],
    adapter: BaseAdapter,
    project_name: str,
    settings: ScenarioTestSettings,
    connection_hooks: ConnectionHooks | None = None,
    on_scenario_start: Callable[[CompiledSqlScenario], None] | None = None,
    on_scenario_complete: Callable[
        [CompiledSqlScenario, ScenarioExecutionPlan | None, ScenarioRunResult], None
    ]
    | None = None,
) -> tuple[ScenarioRunResult, ...]:
    """Execute selected scenarios from a compiled project."""

    retain: bool = settings.retain
    policies: dict[str, ItemTimeoutPolicy | None] = {
        scenario.name: scenario_timeout(scenario=scenario, timeouts=settings.timeouts)
        for scenario in scenarios
    }

    worker_count: int = max(1, min(settings.concurrency, len(scenarios)))
    running: RunningScenarios = RunningScenarios(adapter=adapter)
    scheduling: _ScenarioScheduling = _ScenarioScheduling(
        worker_count=worker_count,
        interrupts=ScenarioInterrupts(
            cancel_statements=partial(running.interrupt, close_uncancellable=False)
        ),
        running=running,
    )
    with (
        relation_metadata_cache_scope(adapter=adapter),
        _scenario_target_connections(
            adapter=adapter,
            connection_config=connection_config,
            connection_hooks=connection_hooks,
            connection_count=worker_count,
            abandoned=scheduling.interrupts.abandoned,
        ) as connection_pool,
    ):
        options: ScenarioRunOptions = ScenarioRunOptions(
            promotion_mode=resolve_promotion_mode(
                settings=pipeline_result.project.settings, adapter=adapter
            ),
            prepared_schemas=PreparedScenarioSchemas(),
            interrupts=scheduling.interrupts,
        )

        def execute(scenario_plan: ScenarioExecutionPlan) -> ScenarioRunResult:
            connection: Any = connection_pool.get()
            running.start(scenario_plan=scenario_plan, connection=connection)
            try:
                with item_deadline_scope(
                    kind=ItemKind.SCENARIO,
                    name=scenario_plan.name,
                    policy=policies.get(scenario_plan.name),
                    adapter=adapter,
                    connection=connection,
                ) as deadline:
                    result: ScenarioRunResult = execute_scenario_run(
                        scenario_plan=scenario_plan,
                        adapter=adapter,
                        connection=connection,
                        run_id=pipeline_result.project.run_id,
                        retain=retain,
                        options=options,
                    )
                if (
                    deadline is not None
                    and deadline.expired
                    and result.status != ExecutionStatus.SUCCESS
                ):
                    return timed_out_scenario_result(result=result, report=deadline.report())
                return result
            finally:
                running.finish(connection=connection)
                connection_pool.put(connection)

        def failed(*, scenario_name: str, exc: Exception) -> ScenarioRunResult:
            return ScenarioRunResult(
                scenario_name=scenario_name,
                status=ExecutionStatus.FAILED,
                retained=retain,
                error_code=error_code(error=exc, fallback_code=SCENARIO_EXEC_INTERNAL),
                error_help=_scenario_failure_help(exc),
                error_message=error_message(exc),
            )

        try:
            return _run_scenarios(
                pipeline_result=pipeline_result,
                scenarios=scenarios,
                adapter=adapter,
                project_name=project_name,
                source_lexical_syntax=adapter.sql_lexical_syntax,
                execute=execute,
                failed=failed,
                on_scenario_start=on_scenario_start,
                on_scenario_complete=on_scenario_complete,
                scheduling=scheduling,
            )
        except BaseException:
            if worker_count > 1 and not retain:
                _sweep_interrupted_scenarios(
                    running=running,
                    interrupts=scheduling.interrupts,
                    adapter=adapter,
                    connection_config=connection_config,
                    connection_pool=connection_pool,
                )
            raise


def _sweep_interrupted_scenarios(
    *,
    running: RunningScenarios,
    interrupts: ScenarioInterrupts,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection_pool: queue.Queue[Any],
) -> None:
    """Drop started scenarios' relations once workers stop, on a connection still valid."""

    if interrupts.abandoned.is_set():
        return
    if not running.closed_connection:
        cleanup_interrupted_scenarios(
            scenario_plans=running.started_plans,
            adapter=adapter,
            connection=connection_pool.get(),
            interrupts=interrupts,
        )
        return
    connection: Any = adapter.connect(connection_config)
    try:
        cleanup_interrupted_scenarios(
            scenario_plans=running.started_plans,
            adapter=adapter,
            connection=connection,
            interrupts=interrupts,
        )
    finally:
        adapter.close(connection)


def run_scenario_local_test_pipeline(
    *,
    project_dir: Path,
    pipeline_result: CompilePipelineResult,
    scenarios: tuple[CompiledSqlScenario, ...],
    adapter: BaseAdapter,
    project_name: str,
    replay_source: ScenarioLocalReplaySource,
    on_scenario_start: Callable[[CompiledSqlScenario], None] | None = None,
    on_scenario_complete: Callable[
        [CompiledSqlScenario, ScenarioExecutionPlan | None, ScenarioRunResult], None
    ]
    | None = None,
) -> tuple[ScenarioRunResult, ...]:
    """Load selected local scenario snapshots into run-scoped DuckDB databases."""

    promotion_mode: TablePromotionMode = resolve_promotion_mode(
        settings=pipeline_result.project.settings, adapter=adapter
    )
    running: RunningScenarios = RunningScenarios(adapter=adapter)
    conversion: LocalConversion = local_conversion(pipeline_result)

    def execute(scenario_plan: ScenarioExecutionPlan) -> ScenarioRunResult:
        return execute_local_scenario_load_only_run(
            project_dir=project_dir,
            scenario_plan=scenario_plan,
            adapter=adapter,
            promotion_mode=promotion_mode,
            capture_adapter=replay_source.capture_adapter,
            capture_dialect=replay_source.capture_dialect,
            running=running,
            conversion=conversion,
        )

    def failed(*, scenario_name: str, exc: Exception) -> ScenarioRunResult:
        return ScenarioRunResult(
            scenario_name=scenario_name,
            status=ExecutionStatus.FAILED,
            local_status=ScenarioLocalRunStatus.ERROR,
            retained=False,
            error_code=error_code(error=exc, fallback_code=SCENARIO_LOCAL_INTERNAL),
            error_help=_scenario_failure_help(exc),
            error_message=error_message(exc),
        )

    return _run_scenarios(
        pipeline_result=pipeline_result,
        scenarios=scenarios,
        adapter=adapter,
        project_name=project_name,
        source_lexical_syntax=replay_source.lexical_syntax,
        execute=execute,
        failed=failed,
        on_scenario_start=on_scenario_start,
        on_scenario_complete=on_scenario_complete,
        scheduling=_ScenarioScheduling(
            interrupts=ScenarioInterrupts(
                cancel_statements=partial(running.interrupt, close_uncancellable=False)
            )
        ),
    )


def decide_scenario_run_on(
    *,
    project_dir: Path,
    pipeline_result: CompilePipelineResult,
    scenarios: tuple[CompiledSqlScenario, ...],
    adapter: BaseAdapter,
    project_name: str,
    replay_source: ScenarioLocalReplaySource,
    run_on: str,
) -> str:
    """Each scenario's decision as JSON: local with a fresh snapshot whose SQL converts."""

    conversion: LocalConversion = local_conversion(pipeline_result)
    decisions: list[dict[str, object]] = []
    scenario: CompiledSqlScenario
    for scenario in scenarios:
        refusals: tuple[dict[str, object], ...] = ()
        if run_on != RUN_ON_WAREHOUSE:
            try:
                refusals = local_scenario_refusals(
                    project_dir=project_dir,
                    scenario_plan=build_scenario_plan(
                        scenario=scenario,
                        pipeline_result=pipeline_result,
                        dialect=Dialect(adapter),
                        project_name=project_name,
                        source_lexical_syntax=replay_source.lexical_syntax,
                    ),
                    capture_adapter=replay_source.capture_adapter,
                    capture_dialect=replay_source.capture_dialect,
                    conversion=conversion,
                )
            except Exception as error:
                refusals = (
                    {
                        "category": "plan",
                        "construct": f"scenario:{scenario.name}",
                        "reason": error_message(error),
                    },
                )
        decisions.append(
            {
                "name": scenario.name,
                "run_on": RUN_ON_LOCAL
                if run_on != RUN_ON_WAREHOUSE and not refusals
                else RUN_ON_WAREHOUSE,
                "refusals": [
                    {
                        **refusal,
                        "construct": refusal.get("construct") or STATEMENT_CONSTRUCT,
                    }
                    for refusal in refusals
                ],
            }
        )
    return json.dumps({"decisions": decisions}, default=str)


def local_conversion(pipeline_result: CompilePipelineResult) -> LocalConversion:
    """The declared column types of every fully typed model, and the target's session time zone."""

    columns: dict[str, tuple[tuple[str, str], ...]] = {}
    for model in pipeline_result.project.models:
        schema_entry: Any = model.schema_entry
        if schema_entry is None or not schema_entry.columns:
            continue
        typed: tuple[tuple[str, str], ...] = tuple(
            (column.name, column.type) for column in schema_entry.columns if column.type
        )
        if len(typed) == len(schema_entry.columns):
            columns[model.name] = typed
    return LocalConversion(
        model_columns=columns,
        time_zone=session_time_zone(pipeline_result.project),
        model_files={
            model.name: (Path(model.relative_path).as_posix(), model.authored_sql)
            for model in pipeline_result.project.models
        },
    )


def run_scenario_capture_pipeline(
    *,
    project_dir: Path,
    pipeline_result: CompilePipelineResult,
    scenarios: tuple[CompiledSqlScenario, ...],
    connection_config: dict[str, object],
    adapter: BaseAdapter,
    project_name: str,
    settings: ScenarioCaptureSettings,
    connection_hooks: ConnectionHooks | None = None,
    on_scenario_start: Callable[[CompiledSqlScenario], None] | None = None,
    on_scenario_complete: Callable[
        [CompiledSqlScenario, ScenarioExecutionPlan | None, ScenarioSnapshotCaptureRunResult], None
    ]
    | None = None,
) -> tuple[ScenarioSnapshotCaptureRunResult, ...]:
    """Capture selected scenario inputs into durable local snapshot files."""

    with _scenario_target_connections(
        adapter=adapter,
        connection_config=connection_config,
        connection_hooks=connection_hooks,
        connection_count=1,
    ) as connection_pool:
        connection: Any = connection_pool.get()
        running: RunningScenarios = RunningScenarios(adapter=adapter)

        def execute(scenario_plan: ScenarioExecutionPlan) -> ScenarioSnapshotCaptureRunResult:
            running.start(scenario_plan=scenario_plan, connection=connection)
            try:
                return execute_scenario_snapshot_capture_run(
                    project_dir=project_dir,
                    scenario_plan=scenario_plan,
                    adapter=adapter,
                    connection=connection,
                    run_id=pipeline_result.project.run_id,
                    settings=settings,
                    local_type_overrides=scenario_local_type_overrides_for_dialect(
                        scenario_config=pipeline_result.project.scenario,
                        sql_analysis_dialect=adapter.sql_analysis_dialect(),
                    ),
                )
            finally:
                running.finish(connection=connection)

        def failed(*, scenario_name: str, exc: Exception) -> ScenarioSnapshotCaptureRunResult:
            return ScenarioSnapshotCaptureRunResult(
                scenario_name=scenario_name,
                status=ExecutionStatus.FAILED,
                retained=settings.retain,
                error_code=error_code(error=exc, fallback_code=SCENARIO_EXEC_INTERNAL),
                error_help=_scenario_failure_help(exc),
                error_message=error_message(exc),
            )

        return _run_scenarios(
            pipeline_result=pipeline_result,
            scenarios=scenarios,
            adapter=adapter,
            project_name=project_name,
            source_lexical_syntax=adapter.sql_lexical_syntax,
            execute=execute,
            failed=failed,
            on_scenario_start=on_scenario_start,
            on_scenario_complete=on_scenario_complete,
            scheduling=_ScenarioScheduling(
                interrupts=ScenarioInterrupts(
                    cancel_statements=partial(running.interrupt, close_uncancellable=False)
                )
            ),
        )


@contextmanager
def _scenario_target_connections(
    *,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection_hooks: ConnectionHooks | None,
    connection_count: int,
    abandoned: threading.Event | None = None,
) -> Iterator[queue.Queue[Any]]:
    hooks: ConnectionHooks = connection_hooks if connection_hooks is not None else ConnectionHooks()
    if hooks.on_connection_start is not None:
        hooks.on_connection_start(connection_count)
    start: float = time.monotonic()
    try:
        with OperationLifecycle(
            operation_kind="scenario", operation_name="scenario_target_connection"
        ):
            connections: tuple[Any, ...] = open_worker_connections(
                adapter=adapter,
                connection_config=connection_config,
                connection_count=connection_count,
            )
    except Exception as error:
        if hooks.on_connection_error is not None:
            hooks.on_connection_error(connection_count, elapsed_seconds=time.monotonic() - start)
        raise_warehouse_connection_failure(adapter=adapter, error=error)
    if hooks.on_connection_complete is not None:
        hooks.on_connection_complete(connection_count, elapsed_seconds=time.monotonic() - start)
    recovery: ConnectionRecovery = ConnectionRecovery(
        adapter=adapter, connect=lambda: adapter.connect(dict(connection_config))
    )
    connection_pool: queue.Queue[Any] = RecoveringConnectionPool(recovery=recovery)
    connection: Any
    for connection in connections:
        connection_pool.put(connection)
    active_error: BaseException | None = None
    try:
        yield connection_pool
    except BaseException as error:
        active_error = error
        raise
    finally:
        if abandoned is None or not abandoned.is_set():
            recovery.close_replacements()
            close_connections(adapter=adapter, connections=connections, active_error=active_error)


def _run_scenarios[ResultT: _ScenarioPipelineResult](
    *,
    pipeline_result: CompilePipelineResult,
    scenarios: tuple[CompiledSqlScenario, ...],
    adapter: BaseAdapter,
    project_name: str,
    source_lexical_syntax: SqlLexicalSyntax,
    execute: Callable[[ScenarioExecutionPlan], ResultT],
    failed: _ScenarioFailureResult[ResultT],
    on_scenario_start: Callable[[CompiledSqlScenario], None] | None,
    on_scenario_complete: Callable[
        [CompiledSqlScenario, ScenarioExecutionPlan | None, ResultT], None
    ]
    | None,
    scheduling: _ScenarioScheduling | None = None,
) -> tuple[ResultT, ...]:
    resolved_scheduling: _ScenarioScheduling = (
        scheduling if scheduling is not None else _ScenarioScheduling()
    )

    def run(*, scenario: CompiledSqlScenario, notify_start: bool) -> _ScenarioOutcome[ResultT]:
        return _run_scenario(
            scenario=scenario,
            pipeline_result=pipeline_result,
            adapter=adapter,
            project_name=project_name,
            source_lexical_syntax=source_lexical_syntax,
            execute=execute,
            failed=failed,
            on_scenario_start=on_scenario_start if notify_start else None,
        )

    def complete(outcome: _ScenarioOutcome[ResultT]) -> ResultT:
        projector: NativeProgressProjector | None = current_native_progress_projector()
        if projector is not None and on_scenario_complete is not None:
            _ = projector.consume_resource_terminal(
                resource_name=outcome.scenario.name,
                resource_id=_scenario_resource_id(outcome.scenario.name),
                resource_attempt_id=outcome.resource_attempt_id,
            )
        if on_scenario_complete is not None:
            on_scenario_complete(outcome.scenario, outcome.scenario_plan, outcome.result)
        return outcome.result

    scenario: CompiledSqlScenario
    for scenario in scenarios:
        _prepare_scenario_presentation(
            scenario_name=scenario.name,
            has_completion_callback=on_scenario_complete is not None,
        )
    interrupts: ScenarioInterrupts = resolved_scheduling.interrupts
    try:
        with interrupts.handle_sigint():
            if resolved_scheduling.worker_count <= 1:
                return _run_sequentially(
                    scenarios=scenarios, run=run, complete=complete, interrupts=interrupts
                )
            return _run_concurrently(
                scenarios=scenarios,
                run=run,
                complete=complete,
                on_scenario_start=on_scenario_start,
                scheduling=resolved_scheduling,
            )
    except BaseException as error:
        if interrupts.abandoned.is_set():
            interrupts.abandon()
        elif isinstance(error, KeyboardInterrupt):
            interrupts.announce_stop()
        raise


def _run_sequentially[OutcomeT, ResultT](
    *,
    scenarios: tuple[CompiledSqlScenario, ...],
    run: Callable[..., OutcomeT],
    complete: Callable[[OutcomeT], ResultT],
    interrupts: ScenarioInterrupts,
) -> tuple[ResultT, ...]:
    """Run on the calling thread so Ctrl-C reaches the in-flight statement directly."""

    results: list[ResultT] = []
    scenario: CompiledSqlScenario
    for scenario in scenarios:
        results.append(complete(run(scenario=scenario, notify_start=True)))
        if interrupts.stop_requested.is_set():
            interrupts.announce_stop()
            raise KeyboardInterrupt
    return tuple(results)


def _run_concurrently[OutcomeT, ResultT](
    *,
    scenarios: tuple[CompiledSqlScenario, ...],
    run: Callable[..., OutcomeT],
    complete: Callable[[OutcomeT], ResultT],
    on_scenario_start: Callable[[CompiledSqlScenario], None] | None,
    scheduling: _ScenarioScheduling,
) -> tuple[ResultT, ...]:
    futures: tuple[Future[OutcomeT], ...] = tuple(Future() for _ in scenarios)
    results: list[ResultT] = []
    try:
        _start_scenario_workers(
            scenarios=scenarios,
            futures=futures,
            run=run,
            worker_count=scheduling.worker_count,
        )
        index: int
        future: Future[OutcomeT]
        for index, future in enumerate(futures):
            if on_scenario_start is not None:
                on_scenario_start(scenarios[index])
            results.append(complete(future.result()))
    except BaseException as error:
        _stop_scenarios(error=error, futures=futures, scheduling=scheduling)
        raise
    return tuple(results)


def _start_scenario_workers[OutcomeT](
    *,
    scenarios: tuple[CompiledSqlScenario, ...],
    futures: tuple[Future[OutcomeT], ...],
    run: Callable[..., OutcomeT],
    worker_count: int,
) -> None:
    """Start daemon workers so an abandoned run never blocks interpreter exit."""

    queued: queue.SimpleQueue[int] = queue.SimpleQueue()
    index: int
    for index in range(len(scenarios)):
        queued.put(index)

    def work() -> None:
        while True:
            try:
                position: int = queued.get_nowait()
            except queue.Empty:
                return
            future: Future[OutcomeT] = futures[position]
            if not future.set_running_or_notify_cancel():
                continue
            try:
                future.set_result(
                    copy_context().run(run, scenario=scenarios[position], notify_start=False)
                )
            except BaseException as exc:  # noqa: BLE001 - delivered through the future
                future.set_exception(exc)

    atexit.unregister(_join_scenario_workers)
    _ = atexit.register(_join_scenario_workers)
    context: Context = copy_context()
    worker: int
    for worker in range(worker_count):
        threading.Thread(
            target=context.copy().run,
            args=(work,),
            name=f"{_WORKER_NAME_PREFIX}{worker}",
            daemon=True,
        ).start()


def _join_scenario_workers() -> None:
    """Wait a bounded time at exit for abandoned workers, so none is ended inside native code."""

    workers: list[threading.Thread] = [
        thread
        for thread in threading.enumerate()
        if thread.name.startswith(_WORKER_NAME_PREFIX) and thread.is_alive()
    ]
    if not workers:
        return
    _ = sys.stderr.write(_WORKER_EXIT_NOTICE)
    _ = sys.stderr.flush()
    deadline: float = time.monotonic() + _WORKER_EXIT_JOIN_SECONDS
    thread: threading.Thread
    try:
        for thread in workers:
            thread.join(timeout=max(0.0, deadline - time.monotonic()))
    except KeyboardInterrupt:
        return


def _stop_scenarios[OutcomeT](
    *,
    error: BaseException,
    futures: tuple[Future[OutcomeT], ...],
    scheduling: _ScenarioScheduling,
) -> None:
    """Cancel queued scenarios and in-flight statements; a repeat interrupt abandons cleanup."""

    interrupts: ScenarioInterrupts = scheduling.interrupts
    if isinstance(error, KeyboardInterrupt):
        interrupts.announce_stop()
    interrupts.request_stop()
    future: Future[OutcomeT]
    for future in futures:
        _ = future.cancel()
    pending: tuple[Future[OutcomeT], ...] = futures
    try:
        if scheduling.running is not None:
            scheduling.running.interrupt()
        while pending and not interrupts.abandoned.is_set():
            _ = wait(pending, timeout=_STOP_POLL_SECONDS)
            pending = tuple(item for item in pending if not item.done())
    except KeyboardInterrupt:
        interrupts.abandon()
        raise
    if interrupts.abandoned.is_set():
        interrupts.abandon()
        raise KeyboardInterrupt


def _run_scenario[ResultT: _ScenarioPipelineResult](
    *,
    scenario: CompiledSqlScenario,
    pipeline_result: CompilePipelineResult,
    adapter: BaseAdapter,
    project_name: str,
    source_lexical_syntax: SqlLexicalSyntax,
    execute: Callable[[ScenarioExecutionPlan], ResultT],
    failed: _ScenarioFailureResult[ResultT],
    on_scenario_start: Callable[[CompiledSqlScenario], None] | None,
) -> _ScenarioOutcome[ResultT]:
    scenario_plan: ScenarioExecutionPlan | None = None
    started: float = time.monotonic()
    with ResourceAttemptLifecycle(
        resource_id=_scenario_resource_id(scenario.name),
        resource_kind="scenario",
        resource_name=scenario.name,
        run_id=pipeline_result.project.run_id,
    ) as lifecycle:
        if on_scenario_start is not None:
            on_scenario_start(scenario)
        try:
            scenario_plan = build_scenario_plan(
                scenario=scenario,
                pipeline_result=pipeline_result,
                dialect=Dialect(adapter),
                project_name=project_name,
                source_lexical_syntax=source_lexical_syntax,
            )
            result: ResultT = execute(scenario_plan)
        except Exception as exc:
            result = failed(scenario_name=scenario.name, exc=exc)
        result = replace(result, duration_ms=(time.monotonic() - started) * 1000)
        if result.status == ExecutionStatus.FAILED:
            lifecycle.failed(error_code=result.error_code)
    return _ScenarioOutcome(
        scenario=scenario,
        scenario_plan=scenario_plan,
        result=result,
        resource_attempt_id=lifecycle.resource_attempt_id,
    )


def _scenario_resource_id(scenario_name: str) -> str:
    return f"sql_scenario:{scenario_name}"


def _prepare_scenario_presentation(*, scenario_name: str, has_completion_callback: bool) -> None:
    projector: NativeProgressProjector | None = current_native_progress_projector()
    if projector is not None and has_completion_callback:
        projector.expect_resource_enrichment(resource_name=scenario_name)


def select_scenario_snapshot_capture_candidates(
    *,
    project_dir: Path,
    pipeline_result: CompilePipelineResult,
    scenarios: tuple[CompiledSqlScenario, ...],
    adapter: BaseAdapter,
    project_name: str,
    capture_adapter: str,
    capture_dialect: str,
    refresh: bool,
    source_lexical_syntax: SqlLexicalSyntax,
) -> tuple[str, ...]:
    """Return selected scenario names that need snapshot capture before local replay."""

    names: list[str] = []
    scenario: CompiledSqlScenario
    for scenario in scenarios:
        if refresh:
            names.append(scenario.name)
            continue
        try:
            scenario_plan: ScenarioExecutionPlan = build_scenario_plan(
                scenario=scenario,
                pipeline_result=pipeline_result,
                dialect=Dialect(adapter),
                project_name=project_name,
                source_lexical_syntax=source_lexical_syntax,
            )
            snapshot_state: ScenarioSnapshotStateResult = classify_scenario_snapshot_state(
                project_dir=project_dir,
                scenario_plan=scenario_plan,
                capture_adapter=capture_adapter,
                capture_dialect=capture_dialect,
            )
        except Exception as error:
            log_debug_event(
                logger=_DEBUG_LOGGER,
                message="scenario snapshot state classification failed; skipping auto-capture",
                scenario=scenario.name,
                sqlbuild_error=str(error),
            )
            continue
        if snapshot_state.state in (ScenarioSnapshotState.MISSING, ScenarioSnapshotState.STALE):
            names.append(scenario.name)
    return tuple(names)
