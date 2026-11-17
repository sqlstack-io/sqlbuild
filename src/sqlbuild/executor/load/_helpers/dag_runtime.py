"""Concurrent DAG helpers for source loader execution."""

from __future__ import annotations

import queue
from collections.abc import Callable
from dataclasses import replace
from typing import Any

from sqlbuild._native import SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.executor.load._helpers.execution import (
    load_resource_kind,
    should_skip_due_to_hard_dependency,
    should_soft_skip_due_to_all_skipped_dependencies,
    skipped_load_result,
)
from sqlbuild.executor.load.main._execute import execute_source_load
from sqlbuild.executor.load.models import (
    LoadDagState,
    LoadDispatchInputs,
    LoadExecutionIndexes,
    LoadExecutionResult,
    LoadRuntimeParams,
)
from sqlbuild.executor.load.types import LoadProgressCallback
from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.executor.scheduling.main._build_in_degree import build_python_node_in_degree
from sqlbuild.executor.scheduling.main._build_ready_queue import build_python_node_ready_queue
from sqlbuild.executor.scheduling.main._run_worker import run_worker_with_completion
from sqlbuild.executor.scheduling.main._unlock_downstream import unlock_downstream_python_nodes
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.runtime.contracts.types import ExecutionResourceKind
from sqlbuild.runtime.observability.classes.resource_attempt_lifecycle import (
    ResourceAttemptLifecycle,
)
from sqlbuild.runtime.observability.main.current_execution_identity import (
    current_execution_identity,
)
from sqlbuild.runtime.observability.models import ExecutionIdentity


def build_load_dag_state(
    *,
    sources: tuple[SourceEntry, ...],
    results: list[LoadExecutionResult | None],
    source_index_by_name: dict[str, int],
    upstream_names: dict[str, tuple[str, ...]],
    downstream_names: dict[str, tuple[str, ...]],
) -> LoadDagState:
    """Build mutable state for concurrent source-loader DAG execution."""

    source_names: tuple[str, ...] = tuple(source.name for source in sources)
    in_degree: dict[str, int] = build_python_node_in_degree(
        node_names=source_names,
        upstream_names=upstream_names,
    )
    ready: list[str] = build_python_node_ready_queue(
        node_names=source_names,
        in_degree=in_degree,
    )
    return LoadDagState(
        results=results,
        in_degree=in_degree,
        ready=ready,
        in_flight=set(),
        failed_or_skipped=set(),
        results_by_name={},
        source_index_by_name=source_index_by_name,
        downstream_names=downstream_names,
        completion_queue=queue.Queue(),
    )


def complete_dag_source(
    *,
    source_name: str,
    result: LoadExecutionResult,
    state: LoadDagState,
    on_load_complete: Callable[[LoadExecutionResult], None] | None,
) -> None:
    """Record one completed source-loader node and unlock downstream nodes."""

    hard_failure: bool = result.status == ExecutionStatus.FAILED or (
        result.status == ExecutionStatus.SKIPPED and result.skip_mode == SkipMode.HARD
    )
    state.record_completion(source_name=source_name, result=result, hard_failure=hard_failure)
    if on_load_complete is not None:
        on_load_complete(result)
    updated_in_degree: dict[str, int]
    newly_ready: tuple[str, ...]
    updated_in_degree, newly_ready = unlock_downstream_python_nodes(
        completed_node_name=source_name,
        in_degree=state.in_degree,
        downstream_names=state.downstream_names,
    )
    state.apply_unlock(in_degree=updated_in_degree, newly_ready=newly_ready)


def execute_ready_dag_source(
    *,
    source_name: str,
    dispatch: LoadDispatchInputs,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection: Any,
    runtime: LoadRuntimeParams,
    on_load_start: Callable[[SourceEntry], None] | None = None,
    on_progress: Callable[[str], None] | None = None,
) -> LoadExecutionResult:
    """Execute one ready DAG node or return a skipped result."""

    identity: ExecutionIdentity | None = current_execution_identity()
    source: SourceEntry = dispatch.source_by_name[source_name]
    if identity is not None and identity.resource_attempt_id is not None:
        if on_load_start is not None:
            on_load_start(source)
        return _execute_ready_dag_source(
            source_name=source_name,
            dispatch=dispatch,
            adapter=adapter,
            connection_config=connection_config,
            connection=connection,
            runtime=runtime,
            on_progress=on_progress,
        )
    resource_kind: ExecutionResourceKind = load_resource_kind(source)
    with ResourceAttemptLifecycle(
        resource_id=f"source:{source_name}",
        resource_kind=resource_kind.value,
        resource_name=source_name,
        run_id=runtime.run_id,
    ) as lifecycle:
        if on_load_start is not None:
            on_load_start(source)
        result: LoadExecutionResult = _execute_ready_dag_source(
            source_name=source_name,
            dispatch=dispatch,
            adapter=adapter,
            connection_config=connection_config,
            connection=connection,
            runtime=runtime,
            on_progress=on_progress,
        )
        if result.status == ExecutionStatus.FAILED:
            lifecycle.failed()
        elif result.status == ExecutionStatus.SKIPPED:
            lifecycle.skipped(
                skip_code="dependency",
                skip_mode=result.skip_mode.value if result.skip_mode is not None else None,
            )
        return result


def _execute_ready_dag_source(
    *,
    source_name: str,
    dispatch: LoadDispatchInputs,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection: Any,
    runtime: LoadRuntimeParams,
    on_progress: Callable[[str], None] | None = None,
) -> LoadExecutionResult:
    """Execute one ready DAG node within its caller-owned lifecycle boundary."""

    indexes: LoadExecutionIndexes = dispatch.indexes
    source: SourceEntry = dispatch.source_by_name[source_name]
    if should_skip_due_to_hard_dependency(
        source=source,
        failed_or_hard_skipped=dispatch.failed_or_hard_skipped,
        indexes=indexes,
    ):
        return skipped_load_result(source=source, reason="Upstream loader hard-skipped")
    if should_soft_skip_due_to_all_skipped_dependencies(
        source=source,
        results_by_name=dispatch.results_by_name,
        indexes=indexes,
    ):
        return skipped_load_result(
            source=source,
            reason="All upstream loaders were soft-skipped",
            mode=SkipMode.SOFT,
        )
    resolved_result_store: Any | None = runtime.result_store
    if resolved_result_store is None and connection is not None:
        resolved_result_store = build_direct_node_result_store(
            adapter=adapter,
            connection=connection,
            database=runtime.default_database,
            schema=runtime.default_schema,
            target_config=runtime.target_config,
        )
    return execute_source_load(
        source_entry=source,
        loader_function=indexes.loader_by_name[source.loader or ""],
        adapter=adapter,
        connection_config=connection_config,
        connection=connection,
        runtime=replace(runtime, result_store=resolved_result_store),
        statement_recorder=StatementRecorder(),
        loader_ref_entries=indexes.loader_ref_entries,
        source_ref_entries=indexes.source_by_name,
        on_progress=on_progress,
    )


def load_dag_worker(
    *,
    source_name: str,
    dispatch: LoadDispatchInputs,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection_pool: queue.Queue[Any],
    runtime: LoadRuntimeParams,
    completion_queue: queue.Queue[tuple[str, LoadExecutionResult]],
    on_load_start: Callable[[SourceEntry], None] | None = None,
    on_load_progress: LoadProgressCallback | None = None,
) -> None:
    """Worker wrapper for concurrent DAG source-loader execution."""

    source_by_name: dict[str, SourceEntry] = dispatch.source_by_name

    def _execute(connection: Any) -> LoadExecutionResult:
        return execute_ready_dag_source(
            source_name=source_name,
            dispatch=dispatch,
            adapter=adapter,
            connection_config=connection_config,
            connection=connection,
            runtime=runtime,
            on_load_start=on_load_start,
            on_progress=(
                None
                if on_load_progress is None
                else lambda message: on_load_progress(source_by_name[source_name], message=message)
            ),
        )

    run_worker_with_completion(
        key=source_name,
        connection_pool=connection_pool,
        completion_queue=completion_queue,
        execute=_execute,
        build_success=lambda *, key, result: _load_dag_success_completion(
            source_name=key, result=result
        ),
        build_failure=lambda *, key, error: _load_dag_failure_completion(
            source_name=key,
            source_by_name=source_by_name,
            error=error,
        ),
    )


def _load_dag_success_completion(
    *, source_name: str, result: LoadExecutionResult
) -> tuple[str, LoadExecutionResult]:
    return (source_name, result)


def _load_dag_failure_completion(
    *, source_name: str, source_by_name: dict[str, SourceEntry], error: BaseException
) -> tuple[str, LoadExecutionResult]:
    source: SourceEntry | None = source_by_name.get(source_name)
    loader_name: str = (source.loader or "") if source is not None else ""
    target: str = (source.table or source.name) if source is not None else source_name
    resource_kind: ExecutionResourceKind = (
        load_resource_kind(source) if source is not None else ExecutionResourceKind.SOURCE
    )
    return (
        source_name,
        LoadExecutionResult(
            source_name=source.name if source is not None else source_name,
            loader_name=loader_name,
            status=ExecutionStatus.FAILED,
            target=target,
            resource_kind=resource_kind,
            error_message=str(error),
        ),
    )
