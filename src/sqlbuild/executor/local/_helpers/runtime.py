"""Failures of converted SQL in DuckDB that the warehouse would not have: guards and engine bugs."""

from __future__ import annotations

import re

from sqlbuild.errors.contracts.exceptions import LocalRunRefusedError
from sqlbuild.executor.local.constants import (
    CANNOT_RUN_LOCALLY_HELP,
    DUCKDB_ERROR_CATEGORY,
    GUARD_CATEGORY,
)

_GUARD_RAISE: re.Pattern[str] = re.compile(r"'([a-z][a-z0-9_]*)#(?:[0-9a-f]{12}|\?): ([^']*)")
_INTERNAL_PREFIX: re.Pattern[str] = re.compile(r"^\s*INTERNAL Error:\s*")
_INTERNAL_MARKER: re.Pattern[str] = re.compile(r"INTERNAL Error:\s*")
_CONSTRUCT_WIDTH: int = 80
_INTERNAL_EXCEPTION: str = "InternalException"


def local_run_refusal(*, error: Exception, fatal: bool) -> LocalRunRefusedError | None:
    """Why a SQLQuack guard's raise or DuckDB internal error keeps a test off DuckDB, or None."""

    message: str = str(error)
    guarded: LocalRunRefusedError | None = _guard_refusal(message)
    if guarded is not None:
        return guarded
    if not fatal or type(error).__name__ != _INTERNAL_EXCEPTION:
        return None
    return _internal_refusal(_INTERNAL_PREFIX.sub("", message))


def local_failure_refusal(message: str) -> LocalRunRefusedError | None:
    """`local_run_refusal` for a failure known only by its text, as a scenario step reports it."""

    guarded: LocalRunRefusedError | None = _guard_refusal(message)
    if guarded is not None:
        return guarded
    internal: re.Match[str] | None = _INTERNAL_MARKER.search(message)
    return None if internal is None else _internal_refusal(message[internal.end() :])


def _guard_refusal(message: str) -> LocalRunRefusedError | None:
    raised: re.Match[str] | None = _GUARD_RAISE.search(message)
    if raised is None:
        return None
    guard: str = raised.group(1)
    return LocalRunRefusedError(
        f"SQLQuack's guard {guard} raised in DuckDB: {raised.group(2)}",
        category=GUARD_CATEGORY,
        construct=guard,
        help=CANNOT_RUN_LOCALLY_HELP,
    )


def _internal_refusal(message: str) -> LocalRunRefusedError:
    first: str = message.splitlines()[0].strip() if message else ""
    return LocalRunRefusedError(
        f"DuckDB failed on the converted SQL: {first}",
        category=DUCKDB_ERROR_CATEGORY,
        construct=first[:_CONSTRUCT_WIDTH] or "internal error",
        help=CANNOT_RUN_LOCALLY_HELP,
    )
