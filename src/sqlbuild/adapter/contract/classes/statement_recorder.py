"""Mutable SQL statement lifecycle recorder."""

from __future__ import annotations

import threading
from collections.abc import Callable, Iterable

from sqlbuild.adapter.contract.models import LifeCycleEvent
from sqlbuild.adapter.contract.types import LifeCycleEventKind, LogLevel


class StatementRecorder:
    """Mutable recorder for runtime lifecycle events."""

    def __init__(self, events: list[LifeCycleEvent] | None = None) -> None:
        self.events = events or []
        self.on_log: Callable[[LogLevel, str], None] | None = None
        self._log_lock: threading.Lock = threading.Lock()

    def record(self, statement: str) -> None:
        self.events.append(LifeCycleEvent(kind=LifeCycleEventKind.SQL, content=statement))

    def record_many(self, statements: Iterable[str]) -> None:
        statement: str
        for statement in statements:
            self.events.append(LifeCycleEvent(kind=LifeCycleEventKind.SQL, content=statement))

    def log(self, *, message: str, level: LogLevel | None = None) -> None:
        """Keep the line; a `ctx.log` line also goes to `on_log` at once, in the order kept."""

        with self._log_lock:
            self.events.append(
                LifeCycleEvent(kind=LifeCycleEventKind.LOG, content=message, level=level)
            )
            if level is not None and self.on_log is not None:
                self.on_log(level, message)

    def snapshot(self) -> tuple[LifeCycleEvent, ...]:
        return tuple(self.events)
