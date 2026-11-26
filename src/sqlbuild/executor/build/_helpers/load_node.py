"""Build-scheduler adapter for load nodes: one SQLLoad run of a load's tables."""

from __future__ import annotations

import time
from itertools import chain
from typing import Any

from sqlbuild._native import CompiledObjectKey, CostContext, LoadPlanEntry, PlanOutput
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.build.models import BuildCallbacks, BuildRuntimeParams
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.loads.exceptions import LoadRunError
from sqlbuild.executor.loads.main._persist_load_result import persist_load_result
from sqlbuild.executor.loads.main._run_load import run_load
from sqlbuild.executor.loads.models import LoadedTable, LoadProject
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore
from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.runtime.contracts.types import ExecutionResourceKind


def execute_build_load_node(
    *,
    entry: LoadPlanEntry,
    plan: PlanOutput,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection: Any,
    runtime: BuildRuntimeParams,
    callbacks: BuildCallbacks,
) -> LoadExecutionResult:
    """Run `entry`'s load, record its node result, and report it as one load result."""

    if callbacks.on_progress is not None:
        callbacks.on_progress(f"load: {entry.name}")
    if callbacks.on_node_start is not None:
        callbacks.on_node_start(name=entry.name, resource_kind=ExecutionResourceKind.LOAD)
    start: float = time.monotonic()
    tables: tuple[LoadedTable, ...] = ()
    error: str | None = None
    try:
        tables = run_load(
            entry=entry,
            project=LoadProject(*runtime.load_projects[entry.name]),
            source_map=plan.source_map,
            adapter=adapter,
            connection_config=connection_config,
            base_connection=connection,
            database=runtime.default_database,
        )
    except (LoadRunError, RuntimeError, KeyError) as raised:
        error = str(raised)
    failures: list[str] = [f"{table.table}: {table.error}" for table in tables if table.error]
    if failures:
        error = "\n".join(failures)
    result: LoadExecutionResult = LoadExecutionResult(
        source_name=entry.name,
        loader_name="",
        status=ExecutionStatus.FAILED if error else ExecutionStatus.SUCCESS,
        target=entry.name,
        resource_kind=ExecutionResourceKind.LOAD,
        rows_loaded=sum(table.rows or 0 for table in tables),
        duration_ms=int((time.monotonic() - start) * 1000),
        error_message=error,
        result_payload=[_table_report(table=table) for table in tables],
        result_metadata={"tables": [table.table for table in tables]},
        warning_messages=tuple(chain.from_iterable(table.warnings for table in tables)),
    )
    result_store: DirectNodeResultStore = build_direct_node_result_store(
        adapter=adapter,
        connection=connection,
        database=runtime.default_database,
        schema=runtime.default_schema,
        target_config=runtime.target_config,
    )
    with CostContext.resource_scope(
        resource_type="load", resource_name=entry.name, phase="finalize"
    ):
        persist_load_result(result_store=result_store, result=result, run_id=runtime.run_id)
    return result


def failed_load_result(*, key: CompiledObjectKey, error_message: str) -> LoadExecutionResult:
    """The failed result of the load `key`, so a load's failure handling applies."""

    return LoadExecutionResult(
        source_name=key.name,
        loader_name="",
        status=ExecutionStatus.FAILED,
        target=key.name,
        resource_kind=ExecutionResourceKind.LOAD,
        error_message=error_message,
    )


def _table_report(*, table: LoadedTable) -> dict[str, object]:
    return {
        "table": table.table,
        "rows": table.rows,
        "error": table.error,
        "changes": list(table.changes),
        "retryable": table.retryable,
    }
