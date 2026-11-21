"""Constants for Python-node execution."""

from __future__ import annotations

MISSING_DEFAULT: object = object()
PYTHON_NODE_RELATION_QUALIFIER_SEPARATOR: str = "."
# A failed node's traceback is kept to this many characters, below the 2,000 the server keeps of
# error text, so the end of it (the exception and its last frames) is what the server keeps.
ERROR_DETAIL_CHARS: int = 1900
TRACEBACK_HEADER: str = "Traceback (most recent call last):\n"
TRACEBACK_ELIDED: str = "  ... {count} earlier lines\n"
