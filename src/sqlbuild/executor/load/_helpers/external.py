"""Helpers for external source loader execution."""

from __future__ import annotations

import logging
import time
from collections.abc import Callable

from sqlbuild._native import DiscoveredLoaderFunction, SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.load.models import (
    LoaderContext,
    LoaderDestination,
    LoaderResult,
    LoaderSkipResult,
    LoadExecutionResult,
    LoadRuntimeParams,
)
from sqlbuild.executor.node_results.models import NodeResultEnvelope
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.provider.main.runtime import (
    _empty_provider_container,
    invoke_with_providers,
)
from sqlbuild.runtime.contracts.types import ExecutionResourceKind
from sqlbuild.runtime.observability.classes.operation_lifecycle import OperationLifecycle


def execute_external_source_load(
    *,
    source_entry: SourceEntry,
    loader_function: DiscoveredLoaderFunction,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    destination: LoaderDestination,
    runtime: LoadRuntimeParams,
    statement_recorder: StatementRecorder,
    resource_kind: ExecutionResourceKind,
    start: float,
    on_progress: Callable[[str], None] | None = None,
) -> LoadExecutionResult:
    """Run one external writer while SQLBuild holds no destination connection."""

    destination_relation: str = destination.relation
    try:
        context: LoaderContext = LoaderContext(
            adapter=adapter,
            connection_config=connection_config,
            connection=None,
            destination=destination_relation,
            destination_database=source_entry.database,
            destination_schema=source_entry.schema,
            destination_name=destination.name,
            run_id=runtime.run_id,
            runtime_dir=runtime.runtime_dir,
            target=runtime.target,
            target_config=runtime.target_config,
            vars=runtime.vars,
            is_reload=runtime.is_reload,
            use_color=runtime.use_color,
            current_cursor_value=None,
            logger=logging.getLogger(f"sqlbuild.loader.{loader_function.name}"),
            statement_recorder=statement_recorder,
            start_cursor_ts=runtime.start_cursor_ts,
            end_cursor_ts=runtime.end_cursor_ts,
            start_cursor_int=runtime.start_cursor_int,
            end_cursor_int=runtime.end_cursor_int,
            providers=(
                runtime.providers if runtime.providers is not None else _empty_provider_container()
            ),
            result_store=runtime.result_store,
            on_progress=on_progress,
        )
        with OperationLifecycle(operation_kind="loader", operation_name="external_source_load"):
            raw_rows: object = invoke_with_providers(
                function=loader_function.function,
                context=context,
                providers=runtime.providers,
            )
            if isinstance(raw_rows, LoaderSkipResult):
                return LoadExecutionResult(
                    source_name=source_entry.name,
                    loader_name=loader_function.name,
                    status=ExecutionStatus.SKIPPED,
                    target=destination_relation,
                    resource_kind=resource_kind,
                    staging_relation=None,
                    rows_loaded=0,
                    duration_ms=int((time.monotonic() - start) * 1000),
                    lifecycle_events=statement_recorder.snapshot(),
                    skip_mode=raw_rows.mode,
                    skip_reason=raw_rows.reason,
                )
            if isinstance(raw_rows, LoaderResult):
                return LoadExecutionResult(
                    source_name=source_entry.name,
                    loader_name=loader_function.name,
                    status=ExecutionStatus.SUCCESS,
                    target=destination_relation,
                    resource_kind=resource_kind,
                    staging_relation=None,
                    rows_loaded=0,
                    duration_ms=int((time.monotonic() - start) * 1000),
                    lifecycle_events=statement_recorder.snapshot(),
                    result_payload=raw_rows.payload,
                    result_metadata=raw_rows.metadata,
                    result_materialized=raw_rows.materialized,
                )
            if isinstance(raw_rows, NodeResultEnvelope):
                raise ExecutorInputError(
                    f"Loader '{loader_function.name}' returned a node result envelope; "
                    "use ctx.result(...) to return this loader's own result"
                )
            if raw_rows is not None:
                raise ExecutorInputError(
                    f"External loader '{loader_function.name}' returned rows, but external "
                    "loaders must write their own destination and return None"
                )
            return LoadExecutionResult(
                source_name=source_entry.name,
                loader_name=loader_function.name,
                status=ExecutionStatus.SUCCESS,
                target=destination_relation,
                resource_kind=resource_kind,
                staging_relation=None,
                rows_loaded=0,
                duration_ms=int((time.monotonic() - start) * 1000),
                lifecycle_events=statement_recorder.snapshot(),
            )
    except Exception as error:
        return LoadExecutionResult(
            source_name=source_entry.name,
            loader_name=loader_function.name,
            status=ExecutionStatus.FAILED,
            target=destination_relation,
            resource_kind=resource_kind,
            staging_relation=None,
            duration_ms=int((time.monotonic() - start) * 1000),
            lifecycle_events=statement_recorder.snapshot(),
            error_message=str(error),
        )
