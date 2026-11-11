"""Shared fakes for adapter unit tests."""

from __future__ import annotations

import threading
import time
from collections.abc import Callable, Iterator
from dataclasses import dataclass
from typing import Any

import pytest


class FetchedRows:
    """A cursor result that returns fixed rows."""

    def __init__(self, rows: list[tuple[str, str]]) -> None:
        self._rows: list[tuple[str, str]] = rows

    def fetchall(self) -> list[tuple[str, str]]:
        """Return the fixed rows."""

        return self._rows


def recording_execute(
    *, rows: list[tuple[str, str]], executed: list[str]
) -> Callable[..., FetchedRows]:
    """Return an adapter execute stub that records SQL and returns fixed rows."""

    def execute(*, connection: Any, sql: str) -> FetchedRows:
        del connection
        executed.append(sql)
        return FetchedRows(rows)

    return execute


class GrantRows:
    """A cursor result that returns fixed grant rows of any width."""

    def __init__(self, rows: list[tuple[object, ...]]) -> None:
        self._rows: list[tuple[object, ...]] = rows

    def fetchall(self) -> list[tuple[object, ...]]:
        """Return the fixed rows."""

        return self._rows


class GrantConnection:
    """A connection that records SQL, answers queries in order, and names a project."""

    def __init__(self, answers: tuple[list[tuple[object, ...]], ...]) -> None:
        self.answers: Iterator[list[tuple[object, ...]]] = iter(answers)
        self.executed: list[str] = []
        self.location: str = "US"
        self.client: GrantClient = GrantClient()

    def execute(self, sql: str) -> GrantRows:
        """Record the SQL and return the next answer, or no rows once they run out."""

        self.executed.append(sql)
        return GrantRows(next(self.answers, []))


class GrantClient:
    """The BigQuery client attribute of a fake connection."""

    project: str = "orders-project"


def grant_execute(connection: GrantConnection) -> Callable[..., GrantRows]:
    """Return an adapter execute stub that routes SQL to a recording connection."""

    def execute(*, connection: Any, sql: str) -> GrantRows:
        return connection.execute(sql)

    del connection
    return execute


class MetadataCursor:
    """A cursor that records metadata SQL and returns fixed rows."""

    def __init__(
        self,
        *,
        rows: tuple[tuple[object, ...], ...],
        executed: list[str],
        columns: tuple[str, ...] = (),
    ) -> None:
        self._rows: tuple[tuple[object, ...], ...] = rows
        self._executed: list[str] = executed
        self.description: tuple[tuple[str], ...] = tuple((column,) for column in columns)

    def execute(self, sql: str, params: tuple[object, ...] | None = None) -> MetadataCursor:
        self._executed.append(f"{sql} {params or ()}")
        return self

    def fetchone(self) -> tuple[object, ...] | None:
        return next(iter(self._rows), None)

    def fetchall(self) -> list[tuple[object, ...]]:
        return list(self._rows)

    def close(self) -> None:
        return None


class MetadataConnection:
    """A connection whose cursors share one SQL log and fixed rows."""

    def __init__(self, rows: tuple[tuple[object, ...], ...], columns: tuple[str, ...] = ()) -> None:
        self.rows: tuple[tuple[object, ...], ...] = rows
        self.columns: tuple[str, ...] = columns
        self.executed: list[str] = []

    def cursor(self) -> MetadataCursor:
        return MetadataCursor(rows=self.rows, executed=self.executed, columns=self.columns)


@dataclass(frozen=True)
class FakeBigQueryTable:
    """BigQuery tables API metadata."""

    table_type: str
    num_rows: int | None
    num_bytes: int | None = None


class FakeBigQueryClient:
    """BigQuery client that records requested table ids."""

    def __init__(self, table: FakeBigQueryTable) -> None:
        self.table: FakeBigQueryTable = table
        self.requested: list[str] = []

    def get_table(self, table_id: str) -> FakeBigQueryTable:
        self.requested.append(table_id)
        return self.table


@dataclass(frozen=True)
class FakeBigQueryConnection:
    """BigQuery connection holding a fake client."""

    client: FakeBigQueryClient


def execute_through_cursor(*, connection: MetadataConnection, sql: str) -> MetadataCursor:
    """Adapter execute stub that runs SQL on a fresh fake cursor."""

    return connection.cursor().execute(sql)


BLOCKING_SQL: str = "SELECT * FROM orders"


@dataclass
class TransactionStatus:
    """A psycopg-style transaction status, named like `psycopg.pq.TransactionStatus`."""

    name: str = "IDLE"


class TransactionInfo:
    """Tracks whether the fake connection has an open transaction, as psycopg reports it."""

    def __init__(self) -> None:
        self.transaction_status: TransactionStatus = TransactionStatus()

    def track(self, sql: str) -> None:
        self.transaction_status = _TRANSACTION_STATUS_AFTER.get(
            sql.strip().upper(), self.transaction_status
        )


_TRANSACTION_STATUS_AFTER: dict[str, TransactionStatus] = {
    "BEGIN": TransactionStatus(name="INTRANS"),
    "COMMIT": TransactionStatus(),
    "ROLLBACK": TransactionStatus(),
}


class BlockingCursor:
    """A DB-API cursor whose query blocks until another thread cancels it."""

    def __init__(
        self, *, executed: list[str], started: threading.Event, info: TransactionInfo
    ) -> None:
        self.executed: list[str] = executed
        self.info: TransactionInfo = info
        self.started: threading.Event = started
        self.cancelled: threading.Event = threading.Event()
        self.connection: object | None = None
        self.description: object | None = None
        self.rowcount: int = -1
        self._request_id: str = "11111111-1111-4111-8111-111111111111"
        self.sfqid: str | None = None
        self.statement_params: dict[str, str] | None = None

    def execute(self, sql: str, *args: Any, **kwargs: Any) -> BlockingCursor:
        del args
        self.executed.append(sql)
        self.info.track(sql)
        self.statement_params = kwargs.get("_statement_params")
        responses: dict[bool, Callable[[], BlockingCursor]] = {
            True: self._block_until_cancelled,
            False: lambda: self,
        }
        return responses[sql.endswith(BLOCKING_SQL)]()

    def _block_until_cancelled(self) -> BlockingCursor:
        self.started.set()
        assert self.cancelled.wait(timeout=5), "statement was not cancelled"
        raise RuntimeError("statement cancelled")

    def cancel(self) -> None:
        self.cancelled.set()

    def close(self) -> None:
        return None


class CancellableRawConnection:
    """A raw driver connection exposing each driver's cancel entry point."""

    def __init__(self) -> None:
        self.info: TransactionInfo = TransactionInfo()
        self.executed: list[str] = []
        self.cursors: list[BlockingCursor] = []
        self.cancel_requests: list[tuple[str, str]] = []
        self.started: threading.Event = threading.Event()
        self._conn: CancellableRawConnection = self

    def cursor(self) -> BlockingCursor:
        cursor: BlockingCursor = BlockingCursor(
            executed=self.executed, started=self.started, info=self.info
        )
        cursor.connection = self
        self.cursors.append(cursor)
        return cursor

    def execute(self, sql: str) -> None:
        self.executed.append(sql)
        self.info.track(sql)

    def cancel_safe(self) -> None:
        self._cancel_running()

    def cancel(self) -> None:
        self._cancel_running()

    def _cancel_query(self, sql: str, request_id: str) -> dict[str, bool]:
        self.cancel_requests.append((sql, request_id))
        self._cancel_running()
        return {"success": True}

    def _cancel_running(self) -> None:
        for cursor in self.cursors:
            cursor.cancel()


def cancel_from_watchdog(*, adapter: Any, connection: Any, raw: CancellableRawConnection) -> None:
    """Run a blocking query while another thread asks the adapter to cancel it."""

    def watchdog() -> None:
        assert raw.started.wait(timeout=5)
        assert adapter.interrupt_connection(connection) is True

    thread: threading.Thread = threading.Thread(target=watchdog)
    thread.start()
    with pytest.raises(RuntimeError, match="statement cancelled"):
        connection.execute(BLOCKING_SQL)
    thread.join(timeout=5)


class CancellableJob:
    """A BigQuery job whose result blocks until it is cancelled."""

    def __init__(self) -> None:
        self.job_id: str = "job-1"
        self.cancelled: threading.Event = threading.Event()
        self.waiting: threading.Event = threading.Event()

    def result(self) -> list[object]:
        self.waiting.set()
        assert self.cancelled.wait(timeout=5), "job was not cancelled"
        raise RuntimeError("statement cancelled")

    def cancel(self) -> bool:
        self.cancelled.set()
        return True


class QueuedJob:
    """A BigQuery job that waits in a queue for a few polls before it runs and completes."""

    def __init__(self, *, pending_polls: int, poll_seconds: float) -> None:
        self.job_id: str = "job-queued"
        self.state: str = "PENDING"
        self.statement_type: str | None = None
        self.num_dml_affected_rows: int | None = None
        self._pending_polls: int = pending_polls
        self._poll_seconds: float = poll_seconds

    def reload(self) -> None:
        time.sleep(self._poll_seconds)
        self._pending_polls -= 1
        self.state = ("RUNNING", "PENDING")[self._pending_polls > 0]

    def result(self) -> list[object]:
        self.state = "DONE"
        return []

    def cancel(self) -> bool:
        return True


class CancellableJobClient:
    """A BigQuery client returning one cancellable job and recording each job config."""

    def __init__(self) -> None:
        self.job: CancellableJob | QueuedJob = CancellableJob()
        self.job_configs: list[Any] = []

    def query(self, sql: str, **kwargs: Any) -> CancellableJob | QueuedJob:
        del sql
        self.job_configs.append(kwargs.get("job_config"))
        return self.job


def without_limits(statements: list[str]) -> tuple[str, ...]:
    """The statements a fake connection ran, without per-transaction statement limits."""

    return tuple(filter(lambda sql: not sql.startswith("SET LOCAL statement_timeout"), statements))


def expire_after_start(*, deadline: Any, raw: CancellableRawConnection) -> threading.Thread:
    """Expire `deadline` from another thread once the fake connection's blocking query starts."""

    def watchdog() -> None:
        assert raw.started.wait(timeout=5)
        deadline.expire()

    thread: threading.Thread = threading.Thread(target=watchdog)
    thread.start()
    return thread
