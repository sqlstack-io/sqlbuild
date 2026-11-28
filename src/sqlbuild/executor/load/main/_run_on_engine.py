"""Run one `@loader` on SQLLoad's engine, its rows written through the build's warehouse session."""

from __future__ import annotations

import json
import time
from typing import Any

from sqlbuild._native import CostContext, DiscoveredLoaderFunction, SourceEntry, run_sqlload_loader
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.load._helpers.engine_runs import engine_result, engine_run
from sqlbuild.executor.load.classes.engine_loader_run import EngineLoaderRun
from sqlbuild.executor.load.constants import LOADER_SESSION, NOT_CALLED
from sqlbuild.executor.load.models import LoaderContext, LoadExecutionResult, LoadRuntimeParams
from sqlbuild.executor.loads.main._load_sessions import load_sessions


def run_on_engine(
    *,
    source_entry: SourceEntry,
    loader_function: DiscoveredLoaderFunction,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    runtime: LoadRuntimeParams,
    sqlbuild: LoaderContext,
    returned: object = NOT_CALLED,
) -> LoadExecutionResult:
    """Load the loader's rows through SQLLoad: the engine calls it, or loads what it `returned`."""

    started_at: float = time.monotonic()
    run: EngineLoaderRun = engine_run(
        source_entry=source_entry,
        loader_function=loader_function,
        runtime=runtime,
        sqlbuild=sqlbuild,
        returned=returned,
    )
    with CostContext.resource_scope(
        resource_type="loader", resource_name=loader_function.name, phase="load"
    ):
        report: dict[str, Any] = json.loads(
            run_sqlload_loader(
                loader=run,
                sessions=load_sessions(
                    session=LOADER_SESSION,
                    adapter=adapter,
                    connection_config=connection_config,
                    base_connection=sqlbuild.connection,
                    database=(
                        source_entry.database
                        if source_entry.database is not None
                        else runtime.default_database
                    ),
                ),
            )
        )
    result: LoadExecutionResult = engine_result(
        report=report, run=run, source_entry=source_entry, sqlbuild=sqlbuild, started_at=started_at
    )
    if runtime.on_rows_written is not None and result.rows_loaded:
        runtime.on_rows_written(result.rows_loaded)
    return result
