"""Installed schedulers killed at random moments still tick every due minute exactly once."""

import json
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    SchedulerCrashTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    scheduler_crash_history,
)

NO_ANOMALIES: dict[str, int] = {
    "triggers_without_ticks": 0,
    "duplicate_ticks": 0,
    "missing_ticks": 0,
    "off_schedule_ticks": 0,
    "runs_without_exactly_one_tick": 0,
    "request_event_mismatch": 0,
    "accepted_duplicate_ticks": 0,
}


@pytest.mark.parametrize(
    "test_case",
    [
        SchedulerCrashTestCase(
            description="two installed SQLite schedulers SIGKILLed while draining a backlog",
            fixture="benchmarks/duckshop/edge_cases/orchestration/scheduler_crash_recovery.json",
            expected_anomalies=NO_ANOMALIES,
            expected_minimum_kills=32,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_schedulers_killed_mid_backlog_when_a_survivor_recovers_then_every_due_minute_ticks_once(
    test_case: SchedulerCrashTestCase, tmp_path: Path
) -> None:
    fixture: dict[str, Any] = json.loads(Path(test_case.fixture).read_text())
    observed: dict[str, int] = scheduler_crash_history(tmp_path, fixture)
    kills: int = observed.pop("kills")
    interrupted: int = observed.pop("ticks_before_survivor")
    ticks: int = observed.pop("ticks")
    print(f"{test_case.description}: {kills} kills, {interrupted} of {ticks} ticks before recovery")
    assert observed == test_case.expected_anomalies
    assert kills >= test_case.expected_minimum_kills
    assert 0 < interrupted < ticks
