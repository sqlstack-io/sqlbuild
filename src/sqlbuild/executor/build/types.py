"""Build executor domain types."""

from __future__ import annotations

from enum import StrEnum
from typing import TYPE_CHECKING, Any, Protocol

if TYPE_CHECKING:
    from sqlbuild._native import ModelPlanEntry, SeedPlanEntry

from sqlbuild.executor.scheduling.types import ExecutionStatus as ExecutionStatus
from sqlbuild.runtime.contracts.types import ExecutionResourceKind as ExecutionResourceKind


class BeforeNodeWriteCallback(Protocol):
    """Called before a model or seed changes its relation; raising fails that node."""

    def __call__(self, *, entry: ModelPlanEntry | SeedPlanEntry, connection: Any) -> None: ...


class BuildStatus(StrEnum):
    SUCCESS = "success"
    FAILED = "failed"
