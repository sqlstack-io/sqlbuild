"""Mark a finished node's own writes, which a cancel or termination must not cut short."""

from __future__ import annotations

from collections.abc import Generator
from contextlib import contextmanager
from contextvars import Token

from sqlbuild.runtime.execution_limits._helpers.context import current_bookkeeping_context


@contextmanager
def bookkeeping_statements() -> Generator[None]:
    """Keep the statements run inside out of what a cancel or termination cancels."""

    token: Token[bool] = current_bookkeeping_context.set(True)
    try:
        yield
    finally:
        current_bookkeeping_context.reset(token)
