"""Helpers for source loader execution model tests."""

from __future__ import annotations

from typing import Any, ClassVar

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.assets import asset
from sqlbuild.observability import LifecycleEvent


def operation_events(events: list[LifecycleEvent]) -> tuple[LifecycleEvent, ...]:
    return tuple(filter(lambda event: event.event_type.startswith("operation_"), events))


def statement_events(events: list[LifecycleEvent]) -> tuple[LifecycleEvent, ...]:
    return tuple(filter(lambda event: event.event_type.startswith("statement_"), events))


def operation_statement_events(
    *, events: tuple[LifecycleEvent, ...], operation_id: str | None
) -> tuple[LifecycleEvent, ...]:
    return tuple(filter(lambda event: event.operation_id == operation_id, events))


class LoaderContextTestCursor:
    def __init__(self, row: tuple[object, ...] | None) -> None:
        self._row = row

    def fetchone(self) -> tuple[object, ...] | None:
        return self._row


class LoaderContextTestAdapter(BaseAdapter):
    """Adapter that records SQL and returns deterministic values."""

    adapter_name: ClassVar[str] = "loader-context-test"

    def __init__(self) -> None:
        self.executed_sql: list[str] = []

    def connect(self, config: dict[str, object]) -> object:
        del config
        return object()

    def close(self, connection: object) -> None:
        del connection

    def relation_exists(
        self,
        connection: Any,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> bool:
        del connection, database, schema, name
        return True

    def _execute(self, connection: Any, sql: str) -> object:
        del connection
        self.executed_sql.append(sql)
        return {
            True: LoaderContextTestCursor(("max-value",)),
            False: f"result:{sql}",
        }[sql.startswith("SELECT MAX")]


class CountingLoaderContextTestAdapter(LoaderContextTestAdapter):
    """Adapter that records connection attempts."""

    def __init__(self) -> None:
        super().__init__()
        self.connection_count: int = 0

    def connect(self, config: dict[str, object]) -> object:
        self.connection_count += 1
        return super().connect(config)


@asset
def score_orders(ctx: object) -> object:
    """A task a loader may read through `ctx.result_of`, never `ctx.loader`."""

    return ctx


@asset
def scored_orders_export(ctx: object) -> object:
    """An asset a loader may read through `ctx.result_of`, never `ctx.loader`."""

    return ctx


def plain_function(ctx: object) -> object:
    """A function that is no Python node."""

    return ctx
