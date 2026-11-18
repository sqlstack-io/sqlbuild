from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class CloneE2ETestCase:
    description: str
    repo_files: dict[str, str]
    clone_command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]


@dataclass(frozen=True)
class ClonePolicyErrorTestCase:
    description: str
    origin_allowed: bool
    destination_allowed: bool
    expected_error_code: str
    expected_policy_key: str


@dataclass(frozen=True)
class CloneFunctionGraphE2ETestCase:
    description: str
    expected_stdout_fragments: tuple[str, ...]
    expected_resource_order: tuple[str, ...]
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]


@dataclass(frozen=True)
class ClonePythonFunctionE2ETestCase:
    description: str
    expected_stdout_fragments: tuple[str, ...]
    expected_unregistered_function_match: str


@dataclass(frozen=True)
class CloneDeferredSourceFunctionE2ETestCase:
    description: str
    expected_stdout_fragments: tuple[str, ...]
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]


@dataclass(frozen=True)
class ClonePreparationE2ETestCase:
    description: str
    clone_arguments: tuple[str, ...]
    expected_assets: tuple[tuple[str, str, str, str | None, str], ...]
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]


@dataclass(frozen=True)
class CloneTerminalProgressE2ETestCase:
    description: str
    columns: int
    expected_screen_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ClonePreparationFailureE2ETestCase:
    description: str
    columns: int
    broken_model_path: str
    broken_model_sql: str
    expected_screen_fragments: tuple[str, ...]


@dataclass(frozen=True)
class CloneTimingsE2ETestCase:
    """The clone phases a debug run reports, in order."""

    description: str
    expected_phases: tuple[str, ...]


@dataclass(frozen=True)
class CloneChangedPlanE2ETestCase:
    """Commands run after building both targets, and what `plan --changed` says after them."""

    description: str
    commands: tuple[tuple[str, ...], ...]
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class CloneMissingUpstreamE2ETestCase:
    """A target missing a selected model's upstreams, and the clone command S301 prints for it."""

    description: str
    default_target: str
    target_args: tuple[str, ...]
    expected_clone_command: str


@dataclass(frozen=True)
class MissingUpstreamFixesE2ETestCase:
    """Whether the planned target may be cloned into, and what S301 then offers."""

    description: str
    destination_allowed: bool
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class PartialDeferCloneE2ETestCase:
    """A defer clone with one boundary missing in the origin, and what `plan --changed` says."""

    description: str
    dropped_origin_table: str
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class UnwritableConsumptionDeferCloneE2ETestCase:
    """A partly failed defer clone whose destination consumption table rejects writes."""

    description: str
    dropped_origin_table: str
    before_clone: tuple[str, ...]
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DeferToMissingUpstreamE2ETestCase:
    """Where an unselected upstream was built, and how a build deferring to prod plans it."""

    description: str
    upstream_sql: str
    expected_exit_code: int
    expected_fragments: tuple[str, ...]
    expected_report_ids: tuple[int, ...]
