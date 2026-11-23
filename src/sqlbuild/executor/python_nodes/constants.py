"""Constants for Python-node execution."""

from __future__ import annotations

MISSING_DEFAULT: object = object()
PYTHON_NODE_RELATION_QUALIFIER_SEPARATOR: str = "."

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
