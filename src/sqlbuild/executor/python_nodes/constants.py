"""Constants for Python-node execution."""

from __future__ import annotations

MISSING_DEFAULT: object = object()
PYTHON_NODE_RELATION_QUALIFIER_SEPARATOR: str = "."
ERROR_DETAIL_CHARS: int = 1900
ERROR_SUMMARY_CHARS: int = 900
ERROR_SUMMARY_CUT: str = "…\n"
TRACEBACK_HEADER: str = "Traceback (most recent call last):\n"
TRACEBACK_ELIDED: str = "  ... {count} earlier lines\n"
