"""What one loader's engine run is given and how its report becomes the loader's result."""

from __future__ import annotations

import json
import time
from collections.abc import Callable, Mapping
from datetime import datetime
from itertools import chain
from typing import Any, cast

from sqlload_api import Context, literal_paths

from sqlbuild._native import DiscoveredLoaderFunction, SourceEntry
from sqlbuild.executor.load._helpers.execution import load_resource_kind
from sqlbuild.executor.load.classes.engine_loader_run import EngineLoaderRun
from sqlbuild.executor.load.classes.loader_run_context import LoaderRunContext
from sqlbuild.executor.load.constants import (
    HTTP_CONTEXT_CLIENTS,
    INTERMEDIATE_WRITE_STRATEGY,
    LOADER_NAMING,
    NOT_CALLED,
)
from sqlbuild.executor.load.models import (
    EngineLoaderInputs,
    LoaderContext,
    LoadExecutionResult,
    LoadRuntimeParams,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.provider.main.runtime import invoke_with_providers


def engine_run(
    *,
    source_entry: SourceEntry,
    loader_function: DiscoveredLoaderFunction,
    runtime: LoadRuntimeParams,
    sqlbuild: LoaderContext,
    returned: object,
) -> EngineLoaderRun:
    """The loader's run: its compiled project and source with this run's destination and cursors."""

    project, declared = runtime.loader_projects[loader_function.name]
    schema: str | None = (
        source_entry.schema if source_entry.schema is not None else runtime.default_schema
    )

    def started(read: Mapping[str, Any]) -> tuple[Context, object]:
        context: LoaderRunContext = cast(
            LoaderRunContext,
            LoaderRunContext.from_read(
                read,
                logger=sqlbuild.logger,
                sqlbuild=sqlbuild,
                connection_name=read.get("connection"),
            ),
        )
        if returned is not NOT_CALLED:
            return context, returned
        return context, invoke_with_providers(
            function=loader_function.function, context=context, providers=runtime.providers
        )

    return EngineLoaderRun(
        inputs=EngineLoaderInputs(
            name=loader_function.name,
            project=project,
            source=json.dumps(
                _run_source(
                    declared=json.loads(declared),
                    function=(loader_function.name, loader_function.function),
                    destination=(schema, sqlbuild.destination_name),
                )
            ),
            reload=runtime.is_reload,
            start_cursor_ts=_cursor_text(runtime.start_cursor_ts),
            end_cursor_ts=_cursor_text(runtime.end_cursor_ts),
            start_cursor_int=_cursor_text(runtime.start_cursor_int),
            end_cursor_int=_cursor_text(runtime.end_cursor_int),
        ),
        start=started,
    )


def engine_result(
    *,
    report: Mapping[str, Any],
    run: EngineLoaderRun,
    source_entry: SourceEntry,
    sqlbuild: LoaderContext,
    started_at: float,
) -> LoadExecutionResult:
    """The loader's result from its engine run's `report`: loaded, failed, or skipped."""

    tables: list[dict[str, Any]] = list(report["tables"])
    errors: list[str] = [] if run.skipped is not None else _table_errors(tables)
    rows: int = sum(table["rows"] or 0 for table in tables)
    return LoadExecutionResult(
        source_name=source_entry.name,
        loader_name=run.name,
        status=_status(skipped=run.skipped is not None, failed=bool(errors)),
        target=sqlbuild.destination,
        resource_kind=load_resource_kind(source_entry),
        rows_loaded=rows,
        rows_affected=None if errors else rows,
        duration_ms=int((time.monotonic() - started_at) * 1000),
        lifecycle_events=sqlbuild.statement_recorder.snapshot(),
        skip_mode=None if run.skipped is None else run.skipped.mode,
        skip_reason=None if run.skipped is None else run.skipped.reason,
        error_message="\n".join(errors) if errors else None,
        result_metadata={"changes": list(chain.from_iterable(t["changes"] for t in tables))},
        warning_messages=tuple(chain.from_iterable(table["warnings"] for table in tables)),
    )


def _table_errors(tables: list[dict[str, Any]]) -> list[str]:
    return [table["error"] for table in tables if table["error"]]


def _status(*, skipped: bool, failed: bool) -> ExecutionStatus:
    if skipped:
        return ExecutionStatus.SKIPPED
    return ExecutionStatus.FAILED if failed else ExecutionStatus.SUCCESS


def _run_source(
    *,
    declared: dict[str, Any],
    function: tuple[str, Callable[..., object]],
    destination: tuple[str | None, str],
) -> dict[str, Any]:
    """The loader's compiled Python source with this run's destination and http paths."""

    name, callable_ = function
    schema, table_name = destination
    settings: dict[str, Any] = declared["settings"]
    settings["table_name"] = table_name
    settings.setdefault("naming", LOADER_NAMING)
    if schema is not None:
        settings["destination_schema"] = schema
    settings["tables"][name].setdefault("write_strategy", INTERMEDIATE_WRITE_STRATEGY)
    paths: dict[str, Any] | None = literal_paths(callable_, context_clients=HTTP_CONTEXT_CLIENTS)
    return {**declared, "http_paths": {} if paths is None else {name: paths}}


def _cursor_text(value: datetime | int | None) -> str | None:
    if value is None:
        return None
    return value.isoformat() if isinstance(value, datetime) else str(value)
