"""Fakes for the recovering connection pool tests."""

from __future__ import annotations

from typing import Any

from sqlbuild.runtime.execution_limits._helpers.interrupted import interrupted_connections


class LostConnectionAdapter:
    """An adapter whose connection dropped: every statement fails and reconnecting fails too."""

    adapter_name: str = "fake"

    def __init__(self) -> None:
        self.reconnect_attempts: int = 0

    def execute(self, *, connection: Any, sql: str) -> None:
        del connection, sql
        raise ConnectionError("server closed the connection")

    def rollback(self, connection: Any) -> None:
        del connection
        raise ConnectionError("server closed the connection")

    def connect(self) -> Any:
        self.reconnect_attempts += 1
        raise ConnectionError("network is unreachable")

    def close(self, connection: Any) -> None:
        del connection


def timed_out_execute(connection: Any) -> str:
    """An item whose statement a deadline interrupted on its connection."""

    interrupted_connections.mark(connection)
    return "timed out"


def later_execute(adapter: LostConnectionAdapter) -> Any:
    """A later item that runs a statement on whatever connection the pool hands it."""

    def execute(connection: Any) -> str:
        adapter.execute(connection=connection, sql="SELECT 1")
        return "ok"

    return execute


def success_completion(*, key: str, result: str) -> tuple[str, str]:
    return key, result


def failure_completion(*, key: str, error: BaseException) -> tuple[str, str]:
    return key, f"failed: {error}"


class RecordingWarehouseAdapter:
    """Records each warehouse switch; reports `session_warehouse` as the session's warehouse."""

    def __init__(self, *, session_warehouse: str | None) -> None:
        self.session_warehouse: str | None = session_warehouse
        self.statements: list[tuple[Any, str]] = []
        self.session_reads: int = 0

    def use_session_warehouse(self, *, connection: Any, warehouse: str) -> None:
        self.statements.append((connection, warehouse))

    def current_session_warehouse(self, *, connection: Any) -> str | None:
        del connection
        self.session_reads += 1
        return self.session_warehouse


class MissingWarehouseAdapter(RecordingWarehouseAdapter):
    """An adapter whose session cannot use any warehouse it is asked for."""

    def use_session_warehouse(self, *, connection: Any, warehouse: str) -> None:
        del connection
        raise RuntimeError(f"Object does not exist: warehouse {warehouse}")
