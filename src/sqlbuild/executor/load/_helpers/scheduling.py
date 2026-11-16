"""Load pipeline phases: connections and DAG scheduling."""

from __future__ import annotations

import queue
import time
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from contextvars import copy_context
from functools import partial
from typing import Any

from sqlbuild._native import SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.load._helpers.dag_runtime import (
    build_load_dag_state,
    complete_dag_source,
    execute_ready_dag_source,
    load_dag_worker,
)
from sqlbuild.executor.load._helpers.execution import (
    build_source_downstream_names,
    build_source_upstream_names,
)
from sqlbuild.executor.load.models import (
    LoadCallbacks,
    LoadDagState,
    LoadDispatchInputs,
    LoadExecutionIndexes,
    LoadExecutionResult,
    LoadRuntimeParams,
)
from sqlbuild.executor.pipeline.main.raise_warehouse_connection_failure import (
    raise_warehouse_connection_failure,
)
from sqlbuild.runtime.contracts.types import ConnectionElapsedCallback


def open_load_connections(
    *,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    effective_concurrency: int,
    on_connection_start: Callable[[int], None] | None,
    on_connection_complete: ConnectionElapsedCallback | None,
    on_connection_error: ConnectionElapsedCallback | None,
) -> tuple[Any, ...]:
    """Open one warehouse connection per worker with progress callbacks."""

    if on_connection_start is not None:
        on_connection_start(effective_concurrency)
    start: float = time.monotonic()
    connections: list[Any] = []
    try:
        for _ in range(effective_concurrency):
            connections.append(adapter.connect(connection_config))
    except Exception as error:
        if on_connection_error is not None:
            on_connection_error(effective_concurrency, elapsed_seconds=time.monotonic() - start)
        connection: Any
        for connection in connections:
            adapter.close(connection)
        raise_warehouse_connection_failure(adapter=adapter, error=error)
    if on_connection_complete is not None:
        on_connection_complete(effective_concurrency, elapsed_seconds=time.monotonic() - start)
    return tuple(connections)


def run_load_dag(
    *,
    sources: tuple[SourceEntry, ...],
    source_by_name: dict[str, SourceEntry],
    indexes: LoadExecutionIndexes,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connections: tuple[Any, ...],
    effective_concurrency: int,
    runtime: LoadRuntimeParams,
    callbacks: LoadCallbacks,
) -> tuple[LoadExecutionResult, ...]:
    """Execute the loader DAG sequentially or concurrently."""

    source_index_by_name: dict[str, int] = {
        source.name: index for index, source in enumerate(sources)
    }
    upstream_names: dict[str, tuple[str, ...]] = build_source_upstream_names(
        sources=sources,
        indexes=indexes,
    )
    downstream_names: dict[str, tuple[str, ...]] = build_source_downstream_names(
        upstream_names=upstream_names
    )
    results: list[LoadExecutionResult | None] = [None] * len(sources)
    state: LoadDagState = build_load_dag_state(
        sources=sources,
        results=results,
        source_index_by_name=source_index_by_name,
        upstream_names=upstream_names,
        downstream_names=downstream_names,
    )
    if effective_concurrency == 1:
        _run_sequential_load_dag(
            state=state,
            source_by_name=source_by_name,
            indexes=indexes,
            adapter=adapter,
            connection_config=connection_config,
            connection=connections[0],
            runtime=runtime,
            callbacks=callbacks,
        )
    else:
        _run_concurrent_load_dag(
            state=state,
            source_by_name=source_by_name,
            indexes=indexes,
            adapter=adapter,
            connection_config=connection_config,
            connections=connections,
            effective_concurrency=effective_concurrency,
            runtime=runtime,
            callbacks=callbacks,
        )
    return tuple(result for result in results if result is not None)


def _run_sequential_load_dag(
    *,
    state: LoadDagState,
    source_by_name: dict[str, SourceEntry],
    indexes: LoadExecutionIndexes,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection: Any,
    runtime: LoadRuntimeParams,
    callbacks: LoadCallbacks,
) -> None:
    while state.ready:
        source_name: str = state.pop_next_ready()
        _ = complete_dag_source(
            source_name=source_name,
            result=execute_ready_dag_source(
                source_name=source_name,
                dispatch=LoadDispatchInputs(
                    source_by_name=source_by_name,
                    indexes=indexes,
                    failed_or_hard_skipped=state.failed_or_skipped,
                    results_by_name=state.results_by_name,
                ),
                adapter=adapter,
                connection_config=connection_config,
                connection=connection,
                runtime=runtime,
                on_load_start=callbacks.on_load_start,
            ),
            state=state,
            on_load_complete=callbacks.on_load_complete,
        )


def _run_concurrent_load_dag(
    *,
    state: LoadDagState,
    source_by_name: dict[str, SourceEntry],
    indexes: LoadExecutionIndexes,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connections: tuple[Any, ...],
    effective_concurrency: int,
    runtime: LoadRuntimeParams,
    callbacks: LoadCallbacks,
) -> None:
    connection_pool: queue.Queue[Any] = queue.Queue()
    connection: Any
    for connection in connections:
        connection_pool.put(connection)
    with ThreadPoolExecutor(max_workers=effective_concurrency) as pool:
        while state.ready or state.in_flight:
            while state.ready and len(state.in_flight) < effective_concurrency:
                source_name: str = state.pop_next_ready()
                state.mark_in_flight(source_name)
                worker: Callable[[], None] = partial(
                    load_dag_worker,
                    source_name=source_name,
                    dispatch=LoadDispatchInputs(
                        source_by_name=source_by_name,
                        indexes=indexes,
                        failed_or_hard_skipped=state.failed_or_skipped,
                        results_by_name=state.results_by_name,
                    ),
                    adapter=adapter,
                    connection_config=connection_config,
                    connection_pool=connection_pool,
                    runtime=runtime,
                    completion_queue=state.completion_queue,
                    on_load_start=callbacks.on_load_start,
                    on_load_progress=callbacks.on_load_progress,
                )
                pool.submit(copy_context().run, worker)
            if not state.in_flight:
                break
            completed_source_name, result = state.completion_queue.get()
            state.finish_in_flight(completed_source_name)
            _ = complete_dag_source(
                source_name=completed_source_name,
                result=result,
                state=state,
                on_load_complete=callbacks.on_load_complete,
            )
