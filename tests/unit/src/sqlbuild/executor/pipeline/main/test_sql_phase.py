"""A build whose SQL phase has nothing to run announces its start and opens no warehouse connections."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import Mock

import pytest

import sqlbuild.executor.pipeline.main.run as run_module
from sqlbuild._native import PlanOutput, SettingsConfig
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.build.models import BuildExecutionResult, BuildRuntimeParams
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from tests.unit.src.sqlbuild.executor.pipeline.main._test_types import EmptySqlPhaseTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        EmptySqlPhaseTestCase(
            description="a Python-only plan starts execution without connection details",
            execution_announced=False,
            expected_event_types=("run_started", "invocation_phase", "run_completed"),
            expected_phase={"phase": "started_execution", "count": 1},
            expected_connects=0,
        ),
        EmptySqlPhaseTestCase(
            description="a caller that announced the start first is not announced twice",
            execution_announced=True,
            expected_event_types=("run_started", "run_completed"),
            expected_phase=None,
            expected_connects=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_plan_without_sql_work_when_building_then_no_connection_opens(
    test_case: EmptySqlPhaseTestCase,
) -> None:
    adapter: Mock = Mock(spec=BaseAdapter)
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    _ = dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with dispatcher_scope(dispatcher):
        with invocation_scope("inv-python-only"):
            result: BuildExecutionResult = run_module.run_build_pipeline(
                plan=PlanOutput(),
                connection_config={},
                adapter=adapter,
                settings=Mock(spec=SettingsConfig),
                runtime=BuildRuntimeParams(
                    run_id="run-python-only", runtime_dir=Path("target"), target="dev"
                ),
                execution_announced=test_case.execution_announced,
            )

    assert (
        result.status,
        tuple(event.event_type for event in events),
        {event.event_type: dict(event.payload) for event in events}.get("invocation_phase"),
        adapter.connect.call_count,
    ) == (
        BuildStatus.SUCCESS,
        test_case.expected_event_types,
        test_case.expected_phase,
        test_case.expected_connects,
    ), test_case.description
