"""Run one converted comparison statement on its own in-memory DuckDB connection."""

from __future__ import annotations

import threading
from typing import Any

from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.testing.constants import LOCAL_DUCKDB_CONNECTION
from sqlbuild.executor.testing.models import LocalStatementRun


def run_converted_statement(
    *, sql: str, timeout_seconds: float | None, setup: tuple[str, ...] = ()
) -> LocalStatementRun:
    """The statement's rows after `setup`, or the error DuckDB raised; interrupted on time-out."""

    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect(dict(LOCAL_DUCKDB_CONNECTION))
    timed_out: threading.Event = threading.Event()
    timer: threading.Timer | None = None
    if timeout_seconds is not None:
        timer = threading.Timer(
            timeout_seconds,
            lambda: (timed_out.set(), adapter.interrupt_connection(connection)),
        )
        timer.start()
    try:
        for statement in setup:
            _ = adapter.execute(connection=connection, sql=statement)
        cursor: Any = adapter.execute(connection=connection, sql=sql)
        rows: list[Any] = cursor.fetchall()
        columns: tuple[str, ...] = tuple(str(column[0]) for column in cursor.description or ())
    except Exception as error:  # noqa: BLE001 - any engine error is this test's result
        return LocalStatementRun(
            rows=(), error=f"{type(error).__name__}: {error}", timed_out=timed_out.is_set()
        )
    finally:
        if timer is not None:
            timer.cancel()
        adapter.close(connection)
    return LocalStatementRun(rows=tuple(rows), error=None, timed_out=False, columns=columns)
