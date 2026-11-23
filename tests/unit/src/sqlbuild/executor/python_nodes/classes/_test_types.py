"""Test case types for the run-time relation guard."""

from dataclasses import dataclass, field

from sqlbuild._native import HardCodedRelationOwnerKind
from sqlbuild.python_nodes.models import SqlResourceRef


@dataclass(frozen=True)
class RuntimeRelationGuardTestCase:
    """SQL a Python node sends, and the hard-coded-name warnings it produces."""

    description: str
    statements: tuple[str, ...]
    expected_warning_fragments: tuple[str, ...]
    expected_warning_count: int
    resolved_refs: tuple[SqlResourceRef, ...] = field(default_factory=tuple)
    own_refs: frozenset[SqlResourceRef] = frozenset()
    owner_label: str = "asset 'export_orders'"
    owner_kind: HardCodedRelationOwnerKind = HardCodedRelationOwnerKind.NODE
    upstream_loader_by_source: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ItemPoolCapTestCase:
    description: str
    items: tuple[tuple[str, str, int | None], ...]
    meet: int
    expected_connections: int
    expected_most_at_once: int


@dataclass(frozen=True)
class ItemPoolOrderTestCase:
    description: str
    items: tuple[tuple[str, int | None, tuple[str, ...]], ...]
    meeting: tuple[str, ...]
    expected_seen_before: dict[str, frozenset[str]]


@dataclass(frozen=True)
class ItemPoolSerialTestCase:
    description: str
    max_parallel: int | None
    adapter_name: str
    expected_threads: int
    expected_connections: int


@dataclass(frozen=True)
class ItemPoolFailureTestCase:
    description: str
    outcome: str
    expected_first_status: str


@dataclass(frozen=True)
class ItemPoolCancelTestCase:
    description: str
    expected_reported: frozenset[tuple[str, str]]
    expected_cancelled: tuple[str, ...]
    expected_never_started: str


@dataclass(frozen=True)
class ItemPoolWriteTestCase:
    description: str
    steps: tuple[str, ...]
    expected_status: str


@dataclass(frozen=True)
class ItemPoolRecordedInterruptTestCase:
    description: str
    items: tuple[str, ...]
    expected_reported: frozenset[tuple[str, str]]


@dataclass(frozen=True)
class ItemPoolIgnoredCancelTestCase:
    description: str
    expected_reported: frozenset[tuple[str, str]]
    expected_cancelled: tuple[str, ...]
    expected_left_open: int
    expected_within_seconds: float


@dataclass(frozen=True)
class ItemPoolRepeatedCancelTestCase:
    description: str
    items: tuple[str, ...]
    expected_reported: frozenset[tuple[str, str]]


@dataclass(frozen=True)
class ItemPoolResultWriteTestCase:
    description: str
    write_seconds: float
    expected_reported: frozenset[tuple[str, str]]


@dataclass(frozen=True)
class ItemPoolMidSubmitTestCase:
    description: str
    start_second_thread: bool
    expected_cancelled: tuple[str, ...]
    expected_left_open: int
