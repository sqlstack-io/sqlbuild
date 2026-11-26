"""Run one load through SQLLoad on the build's warehouse sessions."""

from __future__ import annotations

import json
from collections.abc import Mapping
from typing import Any

from sqlbuild._native import LoadPlanEntry, SourceEntry, run_sqlload_project
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.loads._helpers.tables import (
    missing_message,
    required_tables,
    selected_tables,
)
from sqlbuild.executor.loads.exceptions import LoadRunError
from sqlbuild.executor.loads.main._load_sessions import load_sessions
from sqlbuild.executor.loads.models import LoadedTable, LoadProject


def run_load(
    *,
    entry: LoadPlanEntry,
    project: LoadProject,
    source_map: Mapping[str, SourceEntry],
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    base_connection: Any,
    database: str | None,
) -> tuple[LoadedTable, ...]:
    """Each table `entry` loads; nothing loads when its plan leaves out a source's table."""
    report: dict[str, Any] = json.loads(
        run_sqlload_project(
            project=project.text,
            sessions=load_sessions(
                session=project.session,
                adapter=adapter,
                connection_config=connection_config,
                base_connection=base_connection,
                database=database,
            ),
            tables=(
                selected_tables(entry=entry),
                required_tables(entry=entry, source_map=source_map),
            ),
            reload=entry.is_reload,
        )
    )
    if report["missing"]:
        raise LoadRunError(
            missing_message(load=entry.name, missing=report["missing"], planned=report["planned"])
        )
    return tuple(
        LoadedTable(
            table=table["table"],
            rows=table["rows"],
            error=table["error"],
            changes=tuple(table["changes"]),
            retryable=table["retryable"],
        )
        for table in report["tables"]
    )
