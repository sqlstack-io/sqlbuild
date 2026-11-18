"""Native boundary metadata cases for the real Fensu CLI."""

from dataclasses import dataclass


@dataclass(frozen=True)
class NativeBoundaryTestCase:
    description: str
    metadata: str
    expected_fault_count: int


@dataclass(frozen=True)
class ObservationBoundaryTestCase:
    description: str
    rule: str
    allowed: tuple[str, ...]
    planner: str
    expected_returncode: int
    expected_faults: tuple[str, ...]


@dataclass(frozen=True)
class PlannerReadsTestCase:
    description: str
    files: dict[str, str]
    planner_paths: tuple[str, ...]
    expected_returncode: int
    expected_faults: tuple[str, ...]
