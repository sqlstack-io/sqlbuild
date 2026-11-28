"""Cross-domain runtime contract types."""

from __future__ import annotations

from enum import StrEnum
from typing import Protocol


class ExecutionResourceKind(StrEnum):
    """Top-level resource kind displayed during execution."""

    SOURCE = "source"
    LOADER = "loader"
    SEED = "seed"
    UDF = "udf"
    TABLE_FN = "table_fn"
    VIEW = "view"
    TABLE = "table"
    CUSTOM = "custom"
    SNAPSHOT = "snapshot"
    LOAD = "load"


class ConnectionElapsedCallback(Protocol):
    """Report elapsed time for a connection lifecycle event."""

    def __call__(self, connection_count: int, *, elapsed_seconds: int | float) -> None: ...


class NodeStartCallback(Protocol):
    """Report the start of an execution node."""

    def __call__(self, name: str, *, resource_kind: ExecutionResourceKind) -> None: ...
