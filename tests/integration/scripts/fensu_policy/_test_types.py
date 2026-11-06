"""Native boundary metadata cases for the real Fensu CLI."""

from dataclasses import dataclass


@dataclass(frozen=True)
class NativeBoundaryTestCase:
    description: str
    metadata: str
    expected_fault_count: int
