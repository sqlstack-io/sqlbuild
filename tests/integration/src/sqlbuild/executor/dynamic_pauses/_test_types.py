from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass

from sqlbuild.executor.dynamic_pauses.types import DynamicPauseEventType
from tests.integration.src.sqlbuild.executor.dynamic_pauses.helpers import (
    PausingDuckDbAdapter,
)


@dataclass(frozen=True)
class PauseLeaseTestCase:
    description: str
    second_select: tuple[str, ...]
    second_orders_query: str
    second_totals_query: str
    report_locations: dict[str, str]
    refresh_on_build: bool
    states_before_second: dict[str, str]
    expected_operations: tuple[tuple[str, str], ...]
    expected_events: tuple[tuple[str, str, str, str], ...]


@dataclass(frozen=True)
class HoldTakeoverTestCase:
    description: str
    grace_seconds: int
    ignore_grace: bool
    allow_ignore_grace: bool
    expected_operations: tuple[tuple[str, str], ...]
    expected_events: tuple[tuple[str, str, str, str], ...]


@dataclass(frozen=True)
class HoldRefusalTestCase:
    description: str
    grace_seconds: int
    ignore_grace: bool
    allow_ignore_grace: bool
    expected_code: str
    expected_message_fragment: str


@dataclass(frozen=True)
class FirstBuildTestCase:
    description: str
    pause_dynamic: bool
    expected_operations: tuple[tuple[str, str], ...]
    expected_states: dict[str, str]


@dataclass(frozen=True)
class TakenOverReleaseTestCase:
    description: str
    expected_release_operations: tuple[tuple[str, str], ...]
    expected_states: dict[str, str]
    expected_events: tuple[tuple[str, str, str, str], ...]


@dataclass(frozen=True)
class ConcurrentHoldTestCase:
    description: str
    expected_code: str
    expected_winner: str
    expected_states: dict[str, str]
    expected_events: tuple[tuple[str, str, str, str], ...]


@dataclass(frozen=True)
class LifecycleLineTestCase:
    description: str
    states_before_second: dict[str, str]
    expected_line: str


@dataclass(frozen=True)
class HeartbeatTestCase:
    description: str
    heartbeat_seconds: float
    wait_seconds: float
    expected_min_beats: int


@dataclass(frozen=True)
class CreatedModelHoldTestCase:
    description: str
    refused_events: tuple[DynamicPauseEventType, ...]
    rival_closes: tuple[str, ...]
    expected_release_operations: tuple[tuple[str, str], ...]
    expected_code: str
    expected_message_fragments: tuple[str, ...]
    expected_states: dict[str, str]
    expected_events: tuple[tuple[str, str, str, str], ...]


@dataclass(frozen=True)
class LateCommitTestCase:
    description: str
    expected_code: str
    expected_rival: str
    expected_states: dict[str, str]
    expected_events: tuple[tuple[str, str, str, str], ...]


@dataclass(frozen=True)
class TakenOverPauseTestCase:
    description: str
    expected_code: str
    expected_rival: str
    expected_states: dict[str, str]
    expected_events: tuple[tuple[str, str, str, str], ...]


@dataclass(frozen=True)
class SyncResumeTestCase:
    description: str
    holds: tuple[tuple[str, str, int], ...]
    hand_suspended: tuple[str, ...]
    expected_resumed: tuple[tuple[str, str], ...]
    expected_states: dict[str, str]
    expected_events: tuple[tuple[str, str, str, str], ...]


@dataclass(frozen=True)
class NoPauseTableTestCase:
    description: str
    suspended: str
    expected_states: dict[str, str]


@dataclass(frozen=True)
class QuietTokenTestCase:
    description: str
    holds: tuple[tuple[str, str, int], ...]
    adapter: Callable[[], PausingDuckDbAdapter]
    sent_token: Callable[[str], str | None]
    expected_resumed: tuple[tuple[str, str], ...]
    expected_token: Callable[[str], str | None]


@dataclass(frozen=True)
class DoubleCloseTestCase:
    description: str
    closers: tuple[str, ...]
    expected_events: tuple[tuple[str, str, str, str], ...]


@dataclass(frozen=True)
class SyncDuringTakeoverTestCase:
    description: str
    expected_operations: tuple[tuple[str, str], ...]
    expected_states: dict[str, str]
