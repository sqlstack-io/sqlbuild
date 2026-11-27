"""The tables a load is asked for, and the message when its plan leaves some out."""

from __future__ import annotations

from collections.abc import Mapping

from sqlbuild._native import LoadPlanEntry, SourceEntry
from sqlbuild.executor.loads.constants import PLAN_ERRORS_HEADING


def missing_message(*, load: str, missing: list[str], planned: list[str]) -> str:
    """Why sources `load` fills can't be read: it loads no table into any of `missing`."""
    sources: str = ", ".join(f"'{source}'" for source in missing)
    loaded: str = ", ".join(planned) or "no tables"
    return f"load '{load}' loads no table into {sources}; its tables load into {loaded}"


def missing_help(*, lines: list[str]) -> str:
    """SQLLoad's own account of the tables it couldn't plan, else how to include a table."""
    if any(line.startswith(PLAN_ERRORS_HEADING) for line in lines):
        return "SQLLoad's plan:\n" + "\n".join(lines)
    return "read a table the load includes, or include the table in the load"


def required_tables(*, entry: LoadPlanEntry, source_map: Mapping[str, SourceEntry]) -> list[str]:
    """The tables `entry` must load: every source its load fills, or only its selected ones."""
    if not entry.every_table:
        return list(entry.sources)
    return sorted(name for name, source in source_map.items() if source.load == entry.name)


def selected_tables(*, entry: LoadPlanEntry) -> list[str]:
    """What SQLLoad selects: nothing, which is every table, or the selected sources' tables."""
    if entry.every_table:
        return []
    return list(entry.sources)
