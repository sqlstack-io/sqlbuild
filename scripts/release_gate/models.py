"""Release gate policy models."""

from __future__ import annotations

from dataclasses import dataclass

from scripts.release_gate.types import CheckState


@dataclass(frozen=True)
class WorkflowEvent:
    """The parts of a workflow trigger that decide how the Verify job reports."""

    event_name: str
    repository: str
    head_ref: str | None = None
    head_repository: str | None = None
    action: str | None = None
    label: str | None = None
    partial: str | None = None


@dataclass(frozen=True)
class CheckRun:
    """A check run reported on a commit."""

    sha: str
    name: str
    conclusion: CheckState


@dataclass(frozen=True)
class CommitStatus:
    """A commit status posted on a commit."""

    sha: str
    context: str
    state: CheckState


@dataclass(frozen=True)
class GateScenario:
    """A pull request state and whether its required Verify must let it merge."""

    description: str
    event: WorkflowEvent
    verify_conclusion: CheckState
    statuses: tuple[CommitStatus, ...]
    head_sha: str
    expected_mergeable: bool
