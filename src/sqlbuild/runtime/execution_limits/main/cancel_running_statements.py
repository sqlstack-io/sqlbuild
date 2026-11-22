"""Cancel every statement the process is running, as a termination request does."""

from __future__ import annotations

from sqlbuild.runtime.execution_limits._helpers.cancellation import cancel_connection
from sqlbuild.runtime.execution_limits._helpers.running import running_statements


def cancel_running_statements() -> None:
    """Ask each connection with a statement in flight to cancel it."""

    _ = running_statements.cancel_running(
        cancel=lambda adapter, connection: cancel_connection(adapter=adapter, connection=connection)
    )
