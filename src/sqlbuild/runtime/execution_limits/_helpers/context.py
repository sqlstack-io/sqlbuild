"""Private execution deadline context storage."""

from contextvars import ContextVar

from sqlbuild.runtime.execution_limits.classes.active_connections import ActiveConnections
from sqlbuild.runtime.execution_limits.classes.item_deadline import ItemDeadline
from sqlbuild.runtime.execution_limits.models import ExecutionDeadline

current_deadline_context: ContextVar[ExecutionDeadline | None] = ContextVar(
    "sqlbuild_execution_deadline",
    default=None,
)
current_item_deadline_context: ContextVar[ItemDeadline | None] = ContextVar(
    "sqlbuild_item_deadline",
    default=None,
)
current_bookkeeping_context: ContextVar[bool] = ContextVar(
    "sqlbuild_bookkeeping_statements",
    default=False,
)
current_invocation_connections_context: ContextVar[ActiveConnections | None] = ContextVar(
    "sqlbuild_invocation_connections",
    default=None,
)
