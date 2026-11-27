"""ClickHouse connection wrapper over one clickhouse-connect HTTP client."""

from __future__ import annotations

import math
import threading
import uuid
from collections.abc import Sequence
from typing import Any

from sqlbuild.adapters.clickhouse.classes.clickhouse_cursor import ClickHouseCursor
from sqlbuild.runtime.execution_limits.classes.item_deadline import ItemDeadline
from sqlbuild.runtime.execution_limits.main.current_execution_deadline import (
    current_execution_deadline,
)
from sqlbuild.runtime.execution_limits.models import ExecutionDeadline
from sqlbuild.runtime.observability.classes.statement_lifecycle import StatementLifecycle

_ROW_COUNT_KEYWORDS: tuple[str, ...] = ("INSERT",)


class _ClickHouseConnection:
    """Run statements without a server session so worker threads never serialize."""

    def __init__(self, *, client: Any, interrupt_client_factory: Any) -> None:
        self.client: Any = client
        self._interrupt_client_factory: Any = interrupt_client_factory
        self._running: set[str] = set()
        self._running_lock: threading.Lock = threading.Lock()

    def execute(self, sql: str) -> ClickHouseCursor:
        with StatementLifecycle(adapter="clickhouse", sql=sql, intent="execute") as lifecycle:
            query_id: str = uuid.uuid4().hex
            lifecycle.submitted(query_id=query_id)
            try:
                cursor: ClickHouseCursor = self._run(sql=sql, query_id=query_id)
            except Exception as error:
                lifecycle.failed(error=error, query_id=query_id)
                raise
            if cursor.description is not None:
                lifecycle.completed(query_id=query_id, row_count=len(cursor._rows))
            else:
                lifecycle.completed(
                    query_id=query_id,
                    affected_rows=cursor.rowcount if cursor.rowcount >= 0 else None,
                )
            return cursor

    def _run(self, *, sql: str, query_id: str) -> ClickHouseCursor:
        deadline: ExecutionDeadline | ItemDeadline | None = current_execution_deadline()
        settings: dict[str, object] = {"query_id": query_id}
        if deadline is not None:
            deadline.enforce()
            settings["max_execution_time"] = max(1, math.ceil(deadline.remaining_seconds))
        with self._running_lock:
            self._running.add(query_id)
        try:
            result: Any = self.client.query(sql, settings=settings)
        finally:
            with self._running_lock:
                self._running.discard(query_id)
        summary: dict[str, Any] = dict(getattr(result, "summary", None) or {})
        rowcount: int = -1
        if sql.lstrip().upper().startswith(_ROW_COUNT_KEYWORDS):
            rowcount = int(summary.get("written_rows", 0) or 0)
        return ClickHouseCursor(
            column_names=tuple(result.column_names or ()),
            column_types=tuple(column.name for column in (result.column_types or ())),
            rows=result.result_rows or (),
            rowcount=rowcount,
            query_id=query_id,
        )

    def insert_csv(self, *, table: str, column_names: Sequence[str], payload: bytes) -> None:
        """Bulk insert CSV text; ClickHouse parses each column into its declared type."""

        self.client.raw_insert(
            table=table,
            column_names=list(column_names),
            insert_block=payload,
            settings={"input_format_csv_empty_as_default": 0},
            fmt="CSV",
        )

    def insert_parquet(self, *, table: str, payload: bytes) -> None:
        """Bulk insert one Parquet file; ClickHouse reads each column into its declared type."""

        self.client.raw_insert(table=table, insert_block=payload, fmt="Parquet")

    def cancel_active_statements(self) -> bool:
        """Kill this connection's running queries from a separate client."""

        with self._running_lock:
            running: tuple[str, ...] = tuple(self._running)
        if not running:
            return True
        killer: Any = self._interrupt_client_factory()
        try:
            identifiers: str = ", ".join(f"'{query_id}'" for query_id in running)
            killer.command(f"KILL QUERY WHERE query_id IN ({identifiers}) ASYNC")
        finally:
            killer.close()
        return True

    def close(self) -> None:
        self.client.close()
