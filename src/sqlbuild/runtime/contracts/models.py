"""Cross-domain runtime contract models."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass

from sqlbuild.runtime.contracts.types import ConnectionElapsedCallback


@dataclass(frozen=True)
class TargetSettings:
    """The active target's database and schemas, resolved as SQLBuild uses them."""

    name: str | None
    database: str | None
    schema: str | None
    loader_schema: str | None
    database_preserved: bool = False
    schema_preserved: bool = False


@dataclass(frozen=True)
class ConnectionHooks:
    """Progress and connection lifecycle callbacks for long-running operations."""

    on_progress: Callable[[str], None] | None = None
    on_connection_start: Callable[[int], None] | None = None
    on_connection_complete: ConnectionElapsedCallback | None = None
    on_connection_error: ConnectionElapsedCallback | None = None
