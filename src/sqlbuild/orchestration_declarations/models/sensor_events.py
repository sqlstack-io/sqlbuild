"""Events a sensor's function gets, one type per condition; `id` is stable across redeliveries."""

from dataclasses import dataclass
from datetime import datetime
from typing import Any

from sqlbuild.orchestration_declarations.types import (
    CheckEventKind,
    NodeEventKind,
    PlatformEventKind,
    RunEventKind,
)


@dataclass(frozen=True, kw_only=True)
class RunEvent:
    """A run of the watched job or group ended; `url` opens it when the server knows its address."""

    id: str
    occurred_at: datetime
    job: str
    run_id: str
    status: RunEventKind
    error: str | None
    failed_nodes: tuple[str, ...]
    failed_checks: tuple[str, ...]
    target: str
    url: str | None


@dataclass(frozen=True, kw_only=True)
class NodeEvent:
    """A selected node succeeded, failed or recovered in a run."""

    id: str
    occurred_at: datetime
    run_id: str
    job: str | None
    node: str
    event: NodeEventKind
    target: str
    url: str | None


@dataclass(frozen=True, kw_only=True)
class CheckEvent:
    """A check, audit or SQL test on a selected node failed or warned inside a run."""

    id: str
    occurred_at: datetime
    run_id: str
    job: str | None
    check: str
    status: CheckEventKind
    target: str
    url: str | None


@dataclass(frozen=True, kw_only=True)
class SourceNewDataEvent:
    """A selected source has a new version."""

    id: str
    occurred_at: datetime
    source: str
    data_version: str | None
    target: str


@dataclass(frozen=True, kw_only=True)
class SourceStaleEvent:
    """A selected timestamp source's version is older than `after_seconds`."""

    id: str
    occurred_at: datetime
    source: str
    data_version: str | None
    after_seconds: int
    target: str


@dataclass(frozen=True, kw_only=True)
class PlatformEvent:
    """A server observation of a watched kind; `details` holds its fields."""

    id: str
    occurred_at: datetime
    event: PlatformEventKind
    details: dict[str, Any]
