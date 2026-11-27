"""The plan with SQLLoad's plan of each load, failing when one leaves out a source's table."""

from __future__ import annotations

import dataclasses
import json
from typing import Any

from sqlbuild._native import LoadPlanEntry, PlannerInputError, PlanOutput, plan_sqlload_project
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.load_sessions.constants import DUCKDB_BACKED_ADAPTERS
from sqlbuild.executor.loads._helpers.tables import (
    missing_help,
    missing_message,
    required_tables,
    selected_tables,
)
from sqlbuild.executor.loads.constants import MISSING_LOAD_TABLE_CODE
from sqlbuild.executor.loads.main._load_sessions import load_sessions
from sqlbuild.executor.loads.models import LoadProject


def plan_loads(
    *,
    plan: PlanOutput,
    projects: dict[str, tuple[str, str]],
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    base_connection: Any,
    database: str | None,
) -> PlanOutput:
    """`plan` with each load entry's SQLLoad plan lines; leaving out a source's table fails."""
    if not plan.load_entries:
        return plan
    opened: Any = None
    if base_connection is None and adapter.adapter_name in DUCKDB_BACKED_ADAPTERS:
        opened = adapter.connect(dict(connection_config))
    try:
        entries: tuple[LoadPlanEntry, ...] = tuple(
            _planned_entry(
                entry=entry,
                plan=plan,
                project=LoadProject(*projects[entry.name]),
                sessions=load_sessions(
                    session=projects[entry.name][1],
                    adapter=adapter,
                    connection_config=connection_config,
                    base_connection=base_connection if opened is None else opened,
                    database=database,
                ),
            )
            for entry in plan.load_entries
        )
    finally:
        if opened is not None:
            adapter.close(opened)
    return dataclasses.replace(plan, load_entries=entries)


def _planned_entry(
    *,
    entry: LoadPlanEntry,
    plan: PlanOutput,
    project: LoadProject,
    sessions: dict[str, dict[str, object]],
) -> LoadPlanEntry:
    planned: dict[str, Any] = json.loads(
        plan_sqlload_project(
            project=project.text,
            sessions=sessions,
            tables=(
                selected_tables(entry=entry),
                required_tables(entry=entry, source_map=plan.source_map),
            ),
            reload=entry.is_reload,
        )
    )
    if planned["missing"]:
        raise PlannerInputError(
            missing_message(load=entry.name, missing=planned["missing"], planned=planned["tables"]),
            code=MISSING_LOAD_TABLE_CODE,
            help=missing_help(lines=planned["lines"]),
        )
    return dataclasses.replace(entry, plan_lines=tuple(planned["lines"]))
