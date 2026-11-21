"""Mutable SQL statement lifecycle recorder."""

from __future__ import annotations

from collections.abc import Iterable

from sqlbuild.adapter.contract.models import LifeCycleEvent
from sqlbuild.adapter.contract.types import LifeCycleEventKind, LogLevel


class StatementRecorder:
    """Mutable recorder for runtime lifecycle events."""

    def __init__(self, events: list[LifeCycleEvent] | None = None) -> None:
        self.events = events or []

    def record(self, statement: str) -> None:
        self.events.append(LifeCycleEvent(kind=LifeCycleEventKind.SQL, content=statement))

    def record_many(self, statements: Iterable[str]) -> None:
        statement: str
        for statement in statements:
            self.events.append(LifeCycleEvent(kind=LifeCycleEventKind.SQL, content=statement))

    def log(self, message: str, level: LogLevel = LogLevel.INFO) -> None:
        self.events.append(
            LifeCycleEvent(kind=LifeCycleEventKind.LOG, content=message, level=level)
        )

    def snapshot(self) -> tuple[LifeCycleEvent, ...]:
        return tuple(self.events)
