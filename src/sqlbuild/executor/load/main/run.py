"""Source loader execution pipeline."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import DiscoveredLoaderFunction, SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.load._helpers.execution import build_load_execution_indexes
from sqlbuild.executor.load._helpers.scheduling import (
    open_load_connections,
    run_load_dag,
)
from sqlbuild.executor.load.models import (
    LoadCallbacks,
    LoadExecutionIndexes,
    LoadExecutionResult,
    LoadRuntimeParams,
)


def run_load_pipeline(
    *,
    sources: tuple[SourceEntry, ...],
    reference_sources: tuple[SourceEntry, ...] = (),
    loader_functions: tuple[DiscoveredLoaderFunction, ...],
    connection_config: dict[str, object],
    adapter: BaseAdapter,
    runtime: LoadRuntimeParams,
    callbacks: LoadCallbacks | None = None,
    max_concurrency: int = 1,
) -> tuple[LoadExecutionResult, ...]:
    """Execute selected source loaders."""

    resolved_callbacks: LoadCallbacks = callbacks if callbacks is not None else LoadCallbacks()
    index_sources: tuple[SourceEntry, ...] = (*sources, *reference_sources)
    indexes: LoadExecutionIndexes = build_load_execution_indexes(
        sources=index_sources,
        loader_functions=loader_functions,
    )
    source_count: int = sum(1 for source in sources if source.loader is not None)
    if source_count == 0:
        return ()
    source_by_name: dict[str, SourceEntry] = {source.name: source for source in index_sources}
    effective_concurrency: int = max(1, min(max_concurrency, source_count))
    connections: tuple[Any, ...] = open_load_connections(
        adapter=adapter,
        connection_config=connection_config,
        effective_concurrency=effective_concurrency,
        on_connection_start=resolved_callbacks.on_connection_start,
        on_connection_complete=resolved_callbacks.on_connection_complete,
        on_connection_error=resolved_callbacks.on_connection_error,
    )
    try:
        dag_results: tuple[LoadExecutionResult, ...] = run_load_dag(
            sources=sources,
            source_by_name=source_by_name,
            indexes=indexes,
            adapter=adapter,
            connection_config=connection_config,
            connections=connections,
            effective_concurrency=effective_concurrency,
            runtime=runtime,
            callbacks=resolved_callbacks,
        )
    finally:
        connection: Any
        for connection in connections:
            adapter.close(connection)
    return dag_results
