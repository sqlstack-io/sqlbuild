"""Build-scheduler adapter for executable source-load nodes."""

from __future__ import annotations

import dataclasses
import functools
import time
from collections.abc import Callable
from typing import Any

from sqlbuild._native import (
    CompiledObjectKey,
    CostContext,
    DiscoveredLoaderFunction,
    PlanOutput,
    SourceEntry,
    SourceLoadPlanEntry,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.executor.build.models import BuildCallbacks, BuildRuntimeParams
from sqlbuild.executor.load.main._execute import execute_source_load
from sqlbuild.executor.load.main._persist_result import persist_loader_result
from sqlbuild.executor.load.models import LoadExecutionResult, LoadRuntimeParams
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore
from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.runtime.contracts.types import ExecutionResourceKind


def execute_build_source_node(
    *,
    key: CompiledObjectKey,
    plan: PlanOutput,
    loader_functions_by_name: dict[str, DiscoveredLoaderFunction],
    loader_ref_entries: dict[Callable[..., object], SourceEntry],
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection: Any,
    runtime: BuildRuntimeParams,
    callbacks: BuildCallbacks,
    schema_prepared: bool = False,
) -> LoadExecutionResult:
    """Execute one source-load node from the build scheduler."""

    source_entry: SourceEntry = plan.source_map[key.name]
    source_load_entry: SourceLoadPlanEntry | None = next(
        (entry for entry in plan.source_load_entries if entry.key == key), None
    )
    resource_kind: ExecutionResourceKind = (
        source_load_entry.resource_kind
        if source_load_entry is not None
        else ExecutionResourceKind.SOURCE
    )
    loader_name: str = source_entry.loader or ""
    loader_function: DiscoveredLoaderFunction = loader_functions_by_name[loader_name]
    if callbacks.on_progress is not None:
        callbacks.on_progress(f"source: {source_entry.name}")
    if callbacks.on_node_start is not None:
        callbacks.on_node_start(name=source_entry.name, resource_kind=resource_kind)
    start: float = time.monotonic()
    result_store: DirectNodeResultStore = build_direct_node_result_store(
        adapter=adapter,
        connection=connection,
        database=runtime.default_database,
        schema=runtime.default_schema,
        target_config=runtime.target_config,
    )
    result: LoadExecutionResult = execute_source_load(
        source_entry=source_entry,
        loader_function=loader_function,
        adapter=adapter,
        connection_config=connection_config,
        connection=connection,
        runtime=LoadRuntimeParams(
            run_id=runtime.run_id,
            target=runtime.target,
            target_config=runtime.target_config,
            default_database=runtime.default_database,
            default_schema=runtime.default_schema,
            vars=runtime.effective_vars or {},
            is_reload=runtime.loader_is_reload,
            runtime_dir=runtime.runtime_dir,
            start_cursor_ts=runtime.start_cursor_ts,
            end_cursor_ts=runtime.end_cursor_ts,
            start_cursor_int=runtime.start_cursor_int,
            end_cursor_int=runtime.end_cursor_int,
            use_color=runtime.use_color,
            providers=runtime.providers,
            result_store=result_store,
            schema_prepared=schema_prepared,
            project_relations=runtime.project_relations,
            on_rows_written=_rows_written_for(
                key=f"source:{source_entry.name}", callbacks=callbacks
            ),
        ),
        statement_recorder=StatementRecorder(),
        loader_ref_entries=loader_ref_entries,
        source_ref_entries=plan.source_map,
        on_progress=callbacks.on_sub_progress,
    )
    duration: int = int((time.monotonic() - start) * 1000)
    timed_result: LoadExecutionResult = dataclasses.replace(result, duration_ms=duration)
    with CostContext.resource_scope(
        resource_type="loader",
        resource_name=loader_name,
        phase="finalize",
    ):
        persist_loader_result(
            loader_name=loader_name,
            result=timed_result,
            run_id=runtime.run_id,
            result_store=result_store,
        )
    return timed_result


def _rows_written_for(*, key: str, callbacks: BuildCallbacks) -> Callable[[int], None] | None:
    """Report one loader's rows so far under its node key."""

    on_rows_written: Callable[[str, int], None] | None = callbacks.on_rows_written
    if on_rows_written is None:
        return None
    return functools.partial(on_rows_written, key)
