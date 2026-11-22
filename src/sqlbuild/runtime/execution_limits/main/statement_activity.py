"""Whether this process has run statements, and whether it runs one now."""

from __future__ import annotations

from sqlbuild.runtime.execution_limits._helpers.running import running_statements


def statement_activity() -> tuple[bool, bool]:
    """Whether a statement has ever run in this process, and whether one runs now."""

    return running_statements.activity()
