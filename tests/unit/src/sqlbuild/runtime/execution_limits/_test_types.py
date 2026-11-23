from collections.abc import Callable
from contextlib import AbstractContextManager
from dataclasses import dataclass

from sqlbuild.runtime.execution_limits.models import ItemTimeoutPolicy
from sqlbuild.runtime.execution_limits.types import CancellationOutcome, ItemKind
from tests.unit.src.sqlbuild.runtime.execution_limits.helpers import FakeCancellingAdapter


@dataclass(frozen=True)
class RunItemTimeoutsTestCase:
    description: str
    run_limit: str | None
    run_seconds: int | None
    overrides_items: bool
    declared: str | None
    declared_seconds: int | None
    expected_policy: ItemTimeoutPolicy | None


@dataclass(frozen=True)
class DeadlineCancellationTestCase:
    description: str
    interrupt_result: bool
    adapter_type: type[FakeCancellingAdapter]
    asynchronous: bool
    statement_query_id: str | None
    expected_outcome: CancellationOutcome


@dataclass(frozen=True)
class IdleExpiryTestCase:
    description: str
    kind: ItemKind
    statements_run: int
    expected_expired: bool
    expected_outcome: CancellationOutcome
    expected_interrupted: bool


@dataclass(frozen=True)
class SuspendedCleanupTestCase:
    description: str
    expected_enforced: bool


@dataclass(frozen=True)
class EarliestDeadlineTestCase:
    description: str
    build_seconds: int
    item_seconds: int
    expected_item_is_current: bool


@dataclass(frozen=True)
class NoPolicyTestCase:
    description: str
    expected_deadline: None


@dataclass(frozen=True)
class NestedDeadlineTestCase:
    description: str
    outer_seconds: int
    inner_seconds: int
    expected_expired_name: str


@dataclass(frozen=True)
class HeldClockTestCase:
    description: str
    held_seconds: float
    expected_maximum_running_seconds: float


@dataclass(frozen=True)
class SharedQueuedClockTestCase:
    description: str
    running_statements: int
    queued_statements: int
    waited_seconds: float
    expected_minimum_running_seconds: float
    expected_maximum_running_seconds: float


@dataclass(frozen=True)
class InterruptedConnectionReportsTestCase:
    description: str
    new_connection: Callable[[], object]
    expected_reports: tuple[bool, bool]


@dataclass(frozen=True)
class InterruptedIdentityReuseTestCase:
    description: str
    expected_reuse_and_report: tuple[bool, bool]


@dataclass(frozen=True)
class BookkeepingCancelTestCase:
    description: str
    marking: Callable[[], AbstractContextManager[None]]
    expected_cancelled: bool


@dataclass(frozen=True)
class BookkeepingDeadlineTestCase:
    description: str
    expected_expired: bool
