"""Test case declarations for custom-rule host partitioning."""

from dataclasses import dataclass


@dataclass(frozen=True)
class HostPartitionTestCase:
    """One partitioned custom-rule evaluation compared with a single host."""

    description: str
    model_count: int
    hosts: int
    expected_host_runs: int
    records_module_state: bool = False
    detects_duplicates: bool = False
    cache_enabled: bool = True


@dataclass(frozen=True)
class HostFailureTestCase:
    """Failing subjects whose first single-host failure must be reported."""

    description: str
    model_count: int
    failing_models: tuple[str, ...]
    expected_failing_model: str


@dataclass(frozen=True)
class HostCancellationTestCase:
    """A fast failure on one host while the other hosts are still busy."""

    description: str
    model_count: int
    hosts: int
    delay_seconds: float
    expected_error_fragment: str
    expected_max_seconds: float


@dataclass(frozen=True)
class HostInterruptTestCase:
    """An interrupt sent once `interrupt_at_hosts` host processes are busy with slow invocations."""

    description: str
    model_count: int
    hosts: int
    delay_seconds: float
    interrupt_at_hosts: int
    expected_host_launches: int
    failing: tuple[str, ...] = ()


@dataclass(frozen=True)
class SingleHostInterruptTestCase:
    """An interrupt sent once `interrupt_after` model invocations have started on one host."""

    description: str
    model_count: int
    hosts: int
    delay_seconds: float
    interrupt_after: int
    expected_host_launches: int
    expected_max_started_after_interrupt: int
    failing: tuple[str, ...] = ()


@dataclass(frozen=True)
class ModuleStateCase:
    """Custom-rule modules whose state changes both detectors must classify identically."""

    description: str
    files: tuple[tuple[str, str], ...]
    model_count: int
    expected_stateful: frozenset[str]


@dataclass(frozen=True)
class RandomModuleStateCase:
    """Seeded random rule mutations whose classification must match full re-fingerprinting."""

    description: str
    seed: int
    model_count: int
    expected_matches_reference: bool = True


@dataclass(frozen=True)
class StateDetectorCostCase:
    """Stateless rules sharing helper tables, checked by both detectors after every call."""

    description: str
    rule_count: int
    model_count: int
    expected_min_speedup: float


@dataclass(frozen=True)
class HostInputsTestCase:
    """Hosts started before their project payload is published, or abandoned without one."""

    description: str
    model_count: int
    hosts: int
    write_delay_seconds: float
    expected_max_seconds: float


@dataclass(frozen=True)
class AwaitInputsTestCase:
    """One wait for a host payload with a given parent, cancel signal, and payload state."""

    description: str
    reported_parent_pid: int
    existing_files: tuple[str, ...]
    expected_error: type[Exception]
    expected_max_seconds: float


@dataclass(frozen=True)
class AwaitPublishedInputsTestCase:
    """One wait for a payload that is already published while the parent still runs."""

    description: str
    reported_parent_pid: int
    existing_files: tuple[str, ...]
    expected_max_seconds: float
