"""Source loader execution."""

from __future__ import annotations

import time
from collections.abc import Callable, Mapping
from dataclasses import replace
from typing import Any

from sqlload_api import Connection

from sqlbuild._native import CostContext, DiscoveredLoaderFunction, SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.relations.main.resolve_qualified_name_parts import (
    resolve_qualified_name_parts,
)
from sqlbuild.executor.load._helpers.execution import load_resource_kind
from sqlbuild.executor.load._helpers.loader_invocation import (
    build_loader_context,
    build_loader_relation_guard,
    interpret_loader_return,
)
from sqlbuild.executor.load.classes.loader_run_context import LoaderRunContext
from sqlbuild.executor.load.main._run_on_engine import run_on_engine
from sqlbuild.executor.load.models import (
    LoaderContext,
    LoaderDestination,
    LoaderRefBindings,
    LoadExecutionResult,
    LoadRuntimeParams,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.provider.main.runtime import invoke_with_providers
from sqlbuild.runtime.contracts.types import ExecutionResourceKind
from sqlbuild.runtime.observability.classes.operation_lifecycle import OperationLifecycle


def execute_source_load(
    *,
    source_entry: SourceEntry,
    loader_function: DiscoveredLoaderFunction,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection: Any,
    runtime: LoadRuntimeParams,
    statement_recorder: StatementRecorder,
    loader_ref_entries: Mapping[Callable[..., object], SourceEntry] | None = None,
    source_ref_entries: Mapping[str, SourceEntry] | None = None,
    on_progress: Callable[[str], None] | None = None,
) -> LoadExecutionResult:
    """Run one loader: SQLLoad loads a write strategy's rows; without one it writes its table."""

    warnings: list[str] = []
    result: LoadExecutionResult = _execute_source_load(
        source_entry=source_entry,
        loader_function=loader_function,
        adapter=adapter,
        connection_config=connection_config,
        connection=connection,
        runtime=runtime,
        statement_recorder=statement_recorder,
        ref_bindings=LoaderRefBindings(
            loader_ref_entries=loader_ref_entries,
            source_ref_entries=source_ref_entries,
            relation_guard=build_loader_relation_guard(
                source_entry=source_entry,
                loader_function=loader_function,
                adapter=adapter,
                runtime=runtime,
                loader_ref_entries=loader_ref_entries,
                warnings=warnings,
            ),
        ),
        on_progress=on_progress,
    )
    if not warnings:
        return result
    return replace(result, warning_messages=tuple(warnings))


def _execute_source_load(
    *,
    source_entry: SourceEntry,
    loader_function: DiscoveredLoaderFunction,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection: Any,
    runtime: LoadRuntimeParams,
    statement_recorder: StatementRecorder,
    ref_bindings: LoaderRefBindings,
    on_progress: Callable[[str], None] | None = None,
) -> LoadExecutionResult:
    destination_name: str = (
        source_entry.table if source_entry.table is not None else source_entry.name
    )
    destination_relation: str = resolve_qualified_name_parts(
        adapter=adapter,
        database=source_entry.database,
        schema=source_entry.schema,
        name=destination_name,
    )
    start: float = time.monotonic()
    resource_kind: ExecutionResourceKind = load_resource_kind(source_entry)
    try:
        if not runtime.schema_prepared:
            adapter.ensure_schema(
                connection=connection,
                database=source_entry.database,
                schema=source_entry.schema,
                statement_recorder=statement_recorder,
            )
        context: LoaderContext = build_loader_context(
            source_entry=source_entry,
            loader_function=loader_function,
            adapter=adapter,
            connection_config=connection_config,
            connection=connection,
            destination=LoaderDestination(relation=destination_relation, name=destination_name),
            runtime=runtime,
            statement_recorder=statement_recorder,
            ref_bindings=ref_bindings,
            on_progress=on_progress,
        )
        if source_entry.write_strategy is not None:
            return run_on_engine(
                source_entry=source_entry,
                loader_function=loader_function,
                adapter=adapter,
                connection_config=connection_config,
                runtime=runtime,
                sqlbuild=context,
            )
        with (
            CostContext.resource_scope(
                resource_type="loader", resource_name=loader_function.name, phase="load"
            ),
            OperationLifecycle(operation_kind="loader", operation_name="managed_source_load"),
        ):
            returned: object = invoke_with_providers(
                function=loader_function.function,
                context=LoaderRunContext(
                    sqlbuild=context,
                    table=loader_function.name,
                    source=loader_function.name,
                    connection=Connection({}, {}),
                    logger=context.logger,
                    is_reload=runtime.is_reload,
                ),
                providers=runtime.providers,
            )
        early_result: LoadExecutionResult | None = interpret_loader_return(
            raw_rows=returned,
            source_entry=source_entry,
            loader_function=loader_function,
            destination_relation=destination_relation,
            resource_kind=resource_kind,
            statement_recorder=statement_recorder,
            start=start,
        )
        if early_result is not None:
            return early_result
        return run_on_engine(
            source_entry=source_entry,
            loader_function=loader_function,
            adapter=adapter,
            connection_config=connection_config,
            runtime=runtime,
            sqlbuild=context,
            returned=returned,
        )
    except Exception as error:
        return LoadExecutionResult(
            source_name=source_entry.name,
            loader_name=loader_function.name,
            status=ExecutionStatus.FAILED,
            target=destination_relation,
            resource_kind=resource_kind,
            duration_ms=int((time.monotonic() - start) * 1000),
            lifecycle_events=statement_recorder.snapshot(),
            error_message=str(error),
        )
