"""Tests for runtime SQL statement recording."""

from __future__ import annotations

import pytest

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import LifeCycleEvent
from sqlbuild.adapter.contract.types import LifeCycleEventKind
from tests.unit.src.sqlbuild.executor.shared._helpers._test_types import (
    StatementRecorderTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        StatementRecorderTestCase(
            description="records SQL and log events in order",
            statements=("CREATE TABLE x AS SELECT 1", "DROP TABLE y"),
            log_message="building partition 2024-01-01",
            expected_snapshot=(
                LifeCycleEvent(kind=LifeCycleEventKind.SQL, content="CREATE TABLE x AS SELECT 1"),
                LifeCycleEvent(
                    kind=LifeCycleEventKind.LOG,
                    content="building partition 2024-01-01",
                ),
                LifeCycleEvent(kind=LifeCycleEventKind.SQL, content="DROP TABLE y"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_recorded_events_when_snapshotting_then_returns_expected_tuple(
    test_case: StatementRecorderTestCase,
) -> None:
    recorder: StatementRecorder = StatementRecorder()

    recorder.record(test_case.statements[0])
    recorder.log(message=test_case.log_message)
    recorder.record_many(test_case.statements[1:])
    result: tuple[LifeCycleEvent, ...] = recorder.snapshot()

    assert result == test_case.expected_snapshot
