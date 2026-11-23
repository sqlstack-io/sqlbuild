"""Run and record one planner warehouse inspection read."""

from __future__ import annotations

import time
from collections.abc import Callable
from typing import Any

from sqlbuild.adapter.relations._helpers.inspection_context import emit_inspection_record
from sqlbuild.adapter.relations.models import InspectionQueryRecord


def run_recorded_inspection_query(
    *,
    sql: str,
    run: Callable[[], list[Any]],
    answers_empty: Callable[[Exception], bool] | None = None,
) -> list[Any]:
    """Record one inspection read; an error ``answers_empty`` accepts records as an empty read."""

    started: float = time.monotonic()
    try:
        rows: list[Any] = run()
    except Exception as error:
        elapsed: float = time.monotonic() - started
        failure: InspectionQueryRecord = InspectionQueryRecord(
            sql=sql, elapsed_seconds=elapsed, row_count=None, error=str(error)
        )
        try:
            empty: bool = answers_empty is not None and answers_empty(error)
        except Exception:
            _ = emit_inspection_record(record=failure)
            raise
        if not empty:
            _ = emit_inspection_record(record=failure)
            raise
        _ = emit_inspection_record(
            record=InspectionQueryRecord(sql=sql, elapsed_seconds=elapsed, row_count=0)
        )
        return []
    _ = emit_inspection_record(
        record=InspectionQueryRecord(
            sql=sql, elapsed_seconds=time.monotonic() - started, row_count=len(rows)
        )
    )
    return rows
