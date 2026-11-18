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
class JobLimitedBuildTestCase:
    """A small build graph run through the concurrent dispatch loop with per-job limits."""

    description: str
    # Each node and the nodes it waits for.
    upstream: dict[str, tuple[str, ...]]
    node_jobs: dict[str, tuple[str, ...]]
    job_limits: dict[str, int]
    max_concurrency: int
    failing: frozenset[str]
    # The most nodes of each named job, or of the whole build under "*", seen running at once.
    expected_peaks: dict[str, int]
    expected_skipped: tuple[str, ...]


@dataclass(frozen=True)
class SubworkerReservationTestCase:
    """Running and queued microbatch coordinators, and the sub-worker slots one of them gets."""

    description: str
    job_limits: dict[str, int]
    # Nodes of each job already running, the coordinator's included.
    job_running: dict[str, int]
    coordinator_jobs: tuple[str, ...]
    queued_coordinator_jobs: tuple[tuple[str, ...], ...]
    max_concurrency: int
    expected_demand: int
    expected_reservations: tuple[bool, ...]
