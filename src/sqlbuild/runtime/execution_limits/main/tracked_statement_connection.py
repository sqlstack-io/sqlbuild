"""Register a statement's connection with the deadlines that may need to cancel it."""

from __future__ import annotations

from collections.abc import Generator
from contextlib import contextmanager
from typing import Any

from sqlbuild.runtime.execution_limits._helpers.context import (
    current_bookkeeping_context,
    current_invocation_connections_context,
    current_item_deadline_context,
)
from sqlbuild.runtime.execution_limits._helpers.running import running_statements
from sqlbuild.runtime.execution_limits.classes.active_connections import ActiveConnections
from sqlbuild.runtime.execution_limits.classes.item_deadline import ItemDeadline


@contextmanager
def tracked_statement_connection(*, adapter: Any, connection: Any) -> Generator[None]:
    """Record a statement on `connection` for deadlines and, unless bookkeeping, for cancels."""

    registries: list[ActiveConnections] = (
        [] if current_bookkeeping_context.get() else [running_statements]
    )
    invocation: ActiveConnections | None = current_invocation_connections_context.get()
    if invocation is not None:
        registries.append(invocation)
    deadline: ItemDeadline | None = current_item_deadline_context.get()
    if deadline is not None:
        registries.extend(enclosing.connections for enclosing in deadline.chain())
    for registry in registries:
        registry.add(adapter=adapter, connection=connection)
    try:
        yield
    finally:
        for registry in registries:
            registry.remove(connection=connection)
