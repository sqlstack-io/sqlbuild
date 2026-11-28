"""The cursor window a loader that writes its own table reads."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.executor.load._helpers.relation_refs import build_relation_ref
from sqlbuild.executor.load.models import LoadRuntimeParams


def self_managed_cursor_window(
    *,
    source_entry: SourceEntry,
    runtime: LoadRuntimeParams,
    adapter: BaseAdapter,
    connection: Any,
    statement_recorder: StatementRecorder,
) -> tuple[object | None, object | None]:
    """The loader's `ctx.cursor_value` and `ctx.cursor_end`, from the flags or the target."""

    start: object | None = (
        runtime.start_cursor_ts if runtime.start_cursor_ts is not None else runtime.start_cursor_int
    )
    end: object | None = (
        runtime.end_cursor_ts if runtime.end_cursor_ts is not None else runtime.end_cursor_int
    )
    if start is None and not runtime.is_reload:
        start = build_relation_ref(
            adapter=adapter,
            connection=connection,
            source_entry=source_entry,
            statement_recorder=statement_recorder,
        ).current_cursor_value
    return start, end
