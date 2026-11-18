"""Test types for build scheduler class tests."""

from dataclasses import dataclass


@dataclass(frozen=True)
class MicrobatchBoundedFutureTestCase:
    """Expected future bounds for one large microbatch phase."""

    description: str
    batch_count: int
    global_concurrency: int
    model_concurrency: int
    expected_max_subworker_futures: int


@dataclass(frozen=True)
class SqlTestAuthoredOrderTestCase:
    description: str
    case_names: tuple[str, ...]
    expected_order: tuple[str, ...]


@dataclass(frozen=True)
class SchedulerDiagnosticsTestCase:
    description: str
    expected_state_count: int
    expected_running: int
    expected_ready: int
    expected_waiting: int
    expected_limit: int
    expected_aborted: int = 0


@dataclass(frozen=True)
class SchedulerIdentityTestCase:
    description: str
    expected_invocation_id: str
    expected_run_id: str


@dataclass(frozen=True)
class JobLimitDispatchTestCase:
    """Ready nodes, each node's limiting jobs and their limits, and the dispatch order expected."""

    description: str
    ready: tuple[str, ...]
    node_jobs: dict[str, tuple[str, ...]]
    job_limits: dict[str, int]
    blocked: frozenset[str]
    # Before each dispatch, the running node that finishes first, or "" for none.
    finished_before: tuple[str, ...]
    # Each dispatch's node, or None when no ready node has room.
    expected_dispatched: tuple[str | None, ...]
