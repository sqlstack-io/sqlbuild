"""Connections running statements under a deadline, so the deadline can cancel them."""

from __future__ import annotations

import threading
from collections.abc import Callable
from typing import Any


class ActiveConnections:
    """Adapters and connections with a statement in flight, counted per connection."""

    def __init__(self) -> None:
        self._connections: dict[int, tuple[Any, Any, int]] = {}
        self._lock: threading.Lock = threading.Lock()
        self._used: bool = False

    def activity(self) -> tuple[bool, bool]:
        """Whether a statement has ever run here, and whether one runs now."""

        return self._used, bool(self._connections)

    def note_used(self) -> None:
        """Count a statement as run here without registering it for cancels."""

        with self._lock:
            self._used = True

    def add(self, *, adapter: Any, connection: Any) -> None:
        with self._lock:
            self._used = True
            _, _, count = self._connections.get(id(connection), (adapter, connection, 0))
            self._connections[id(connection)] = (adapter, connection, count + 1)

    def remove(self, *, connection: Any) -> None:
        with self._lock:
            entry: tuple[Any, Any, int] | None = self._connections.get(id(connection))
            if entry is None:
                return
            adapter, _, count = entry
            if count <= 1:
                del self._connections[id(connection)]
            else:
                self._connections[id(connection)] = (adapter, connection, count - 1)

    def cancel_running[ResultT](self, *, cancel: Callable[[Any, Any], ResultT]) -> list[ResultT]:
        """Cancel each still-registered statement, holding the lock so none can be replaced."""

        with self._lock:
            return [
                cancel(adapter, connection) for adapter, connection, _ in self._connections.values()
            ]
