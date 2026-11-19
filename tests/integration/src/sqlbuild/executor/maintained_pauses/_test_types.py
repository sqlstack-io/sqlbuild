from __future__ import annotations

from dataclasses import dataclass


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
    pause_maintained: bool
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
