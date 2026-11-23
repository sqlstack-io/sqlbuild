"""Constants for Python-node execution."""

from __future__ import annotations

MISSING_DEFAULT: object = object()
PYTHON_NODE_RELATION_QUALIFIER_SEPARATOR: str = "."
ERROR_DETAIL_CHARS: int = 1900
ERROR_SUMMARY_CHARS: int = 900
ERROR_SUMMARY_CUT: str = "…\n"
TRACEBACK_HEADER: str = "Traceback (most recent call last):\n"
TRACEBACK_ELIDED: str = "  ... {count} earlier lines\n"

CANCEL_REPEAT_SECONDS: float = 1.0
CANCEL_GRACE_SECONDS: float = 5.0
ABANDONABLE_THREAD_PREFIX: str = "sqlbuild-item"
ABANDONED_STEPS_NOTICE: str = (
    "Stopped waiting for {steps} after {seconds:g} seconds; providers are left without teardown.\n"
)
MIN_PARALLEL_ITEMS: int = 2
MEMORY_DATABASE: str = ":memory:"
CONNECTION_PRIVATE_MEMORY_DATABASE_KEYS: dict[str, str] = {
    "duckdb": "database",
    "motherduck": "database",
    "sqlite": "path",
}
