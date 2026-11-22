"""Constants for Python-node execution."""

from __future__ import annotations

MISSING_DEFAULT: object = object()
PYTHON_NODE_RELATION_QUALIFIER_SEPARATOR: str = "."

CANCEL_REPEAT_SECONDS: float = 1.0

# Adapters whose `:memory:` database is private to each connection, by the config key that names it.
PRIVATE_MEMORY_DATABASE_KEYS: dict[str, str] = {
    "duckdb": "database",
    "motherduck": "database",
    "sqlite": "path",
}
