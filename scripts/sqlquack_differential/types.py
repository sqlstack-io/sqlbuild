"""SQLQuack dialect differential enumerations."""

from __future__ import annotations

from enum import StrEnum


class Mutation(StrEnum):
    """How a mutated case breaks one name of its query."""

    UNKNOWN_COLUMN = "unknown_column"
    QUOTED_UPPER = "quoted_upper"
    UNKNOWN_QUALIFIER = "unknown_qualifier"
    UNQUALIFIED_SHARED = "unqualified_shared"
