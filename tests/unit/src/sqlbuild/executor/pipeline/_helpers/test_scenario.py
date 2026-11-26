"""Tests for scenario execution pipeline helpers."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    CompiledSqlScenario,
    CompilePipelineResult,
    ScenarioExecutionPlan,
    SqlLexicalSyntax,
)
from sqlbuild.executor.pipeline._helpers import scenario as scenario_pipeline
from sqlbuild.executor.scenario._helpers.local import execution as local_scenario
from sqlbuild.executor.scenario.constants import (
    SCENARIO_LOCAL_JSONL_INVALID,
    SCENARIO_LOCAL_SNAPSHOT_MISSING,
    SCENARIO_LOCAL_SNAPSHOT_STALE,
)
from sqlbuild.executor.scenario.models import (
    ScenarioCaptureSettings,
    ScenarioLocalReplaySource,
    ScenarioRunOptions,
    ScenarioRunResult,
    ScenarioSnapshotStateResult,
    ScenarioTestSettings,
)
from sqlbuild.executor.scenario.types import ScenarioLocalRunStatus, ScenarioSnapshotState
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.observability import EventDispatcher, LifecycleEvent, dispatcher_scope
from tests.unit.src.sqlbuild.executor.pipeline._helpers._test_types import (
    ScenarioConnectionLifecycleTestCase,
    ScenarioFailureHelpTestCase,
    ScenarioLocalPipelineTestCase,
    ScenarioPlanningLifecycleTestCase,
    ScenarioTestPipelineTestCase,
)
from tests.unit.src.sqlbuild.executor.pipeline._helpers.helpers import (
    ScenarioLocalPipelineTestAdapter,
    ScenarioPipelinePlanBuilder,
    ScenarioPipelineTestAdapter,
    build_scenario_pipeline_result,
    describe_lifecycle_events,
    local_snapshot_loader_for_test_case,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioTestPipelineTestCase(
            description="continues after one scenario planning failure",
            scenario_names=("passing_scenario", "planning_failure"),
            planning_failure_name="planning_failure",
            expected_statuses=("success", "failed"),
            expected_started_names=("passing_scenario", "planning_failure"),
            expected_completed_names=("passing_scenario", "planning_failure"),
            expected_completed_plan_names=("passing_scenario", None),
            expected_connection_events=("connect:pipeline.duckdb", "close"),
            expected_error_fragment="scenario planning failed",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selected_scenarios_when_running_scenario_test_pipeline_then_orchestrates_batch(
    monkeypatch: pytest.MonkeyPatch,
    test_case: ScenarioTestPipelineTestCase,
) -> None:
    pipeline_result: CompilePipelineResult = build_scenario_pipeline_result(
        scenario_names=test_case.scenario_names,
    )
    adapter: ScenarioPipelineTestAdapter = ScenarioPipelineTestAdapter()
    lifecycle_events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=lifecycle_events.append, accepts_opaque=False)
    started_names: list[str] = []
    completed_names: list[str] = []
    completed_plan_names: list[str | None] = []

    def execute_run(
        *,
        scenario_plan: ScenarioExecutionPlan,
        adapter: ScenarioPipelineTestAdapter,
        connection: Any,
        run_id: str,
        retain: bool,
        options: ScenarioRunOptions,
    ) -> ScenarioRunResult:
        del adapter, connection, run_id, retain, options
        return ScenarioRunResult(
            scenario_name=scenario_plan.name,
            status=ExecutionStatus.SUCCESS,
        )

    def complete_scenario(
        scenario: CompiledSqlScenario,
        scenario_plan: ScenarioExecutionPlan | None,
        _result: ScenarioRunResult,
    ) -> None:
        completed_names.append(scenario.name)
        completed_plan_names.append(getattr(scenario_plan, "name", None))

    monkeypatch.setattr(
        scenario_pipeline,
        "build_scenario_plan",
        ScenarioPipelinePlanBuilder(
            planning_failure_name=test_case.planning_failure_name,
            error_message=test_case.expected_error_fragment,
        ),
    )
    monkeypatch.setattr(scenario_pipeline, "execute_scenario_run", execute_run)

    with dispatcher_scope(dispatcher):
        results: tuple[ScenarioRunResult, ...] = scenario_pipeline.run_scenario_test_pipeline(
            pipeline_result=pipeline_result,
            scenarios=pipeline_result.project.sql_scenarios,
            connection_config={"database": "pipeline.duckdb"},
            adapter=adapter,
            project_name="waffle_shop",
            settings=ScenarioTestSettings(retain=False),
            on_scenario_start=lambda scenario: started_names.append(scenario.name),
            on_scenario_complete=complete_scenario,
        )

    assert tuple(result.status for result in results) == test_case.expected_statuses
    assert tuple(started_names) == test_case.expected_started_names
    assert tuple(completed_names) == test_case.expected_completed_names
    assert tuple(completed_plan_names) == test_case.expected_completed_plan_names
    assert tuple(adapter.events) == test_case.expected_connection_events
    assert results[-1].error_message is not None
    assert test_case.expected_error_fragment in results[-1].error_message
    resource_events: list[LifecycleEvent] = lifecycle_events[2:]
    assert tuple(event.event_type for event in lifecycle_events) == (
        "operation_started",
        "operation_completed",
        "resource_attempt_started",
        "resource_attempt_completed",
        "resource_attempt_started",
        "resource_attempt_failed",
    ), describe_lifecycle_events(lifecycle_events)
    assert tuple(event.resource_id for event in resource_events) == (
        "sql_scenario:passing_scenario",
        "sql_scenario:passing_scenario",
        "sql_scenario:planning_failure",
        "sql_scenario:planning_failure",
    )
    assert tuple(event.run_id for event in resource_events) == ("scenario-test-run",) * 4


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioConnectionLifecycleTestCase(
            description="warehouse test pipeline observes the target connection",
            expected_event_types=("operation_started", "operation_completed"),
            expected_operation_names=("scenario_target_connection",) * 2,
            expected_connection_events=("connect:pipeline.duckdb", "close"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_warehouse_test_pipeline_when_connecting_then_publishes_connection_operation(
    test_case: ScenarioConnectionLifecycleTestCase,
) -> None:
    pipeline_result: CompilePipelineResult = build_scenario_pipeline_result(scenario_names=())
    adapter: ScenarioPipelineTestAdapter = ScenarioPipelineTestAdapter()
    lifecycle_events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=lifecycle_events.append, accepts_opaque=False)

    with dispatcher_scope(dispatcher):
        _ = scenario_pipeline.run_scenario_test_pipeline(
            pipeline_result=pipeline_result,
            scenarios=(),
            connection_config={"database": "pipeline.duckdb"},
            adapter=adapter,
            project_name="waffle_shop",
            settings=ScenarioTestSettings(retain=False),
        )

    assert tuple(event.event_type for event in lifecycle_events) == test_case.expected_event_types
    assert (
        tuple(event.payload.get("operation_name") for event in lifecycle_events)
        == test_case.expected_operation_names
    )
    assert tuple(adapter.events) == test_case.expected_connection_events


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioConnectionLifecycleTestCase(
            description="capture pipeline observes the target connection",
            expected_event_types=("operation_started", "operation_completed"),
            expected_operation_names=("scenario_target_connection",) * 2,
            expected_connection_events=("connect:pipeline.duckdb", "close"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_capture_pipeline_when_connecting_then_publishes_connection_operation(
    tmp_path: Path,
    test_case: ScenarioConnectionLifecycleTestCase,
) -> None:
    pipeline_result: CompilePipelineResult = build_scenario_pipeline_result(scenario_names=())
    adapter: ScenarioPipelineTestAdapter = ScenarioPipelineTestAdapter()
    lifecycle_events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=lifecycle_events.append, accepts_opaque=False)

    with dispatcher_scope(dispatcher):
        _ = scenario_pipeline.run_scenario_capture_pipeline(
            project_dir=tmp_path,
            pipeline_result=pipeline_result,
            scenarios=(),
            connection_config={"database": "pipeline.duckdb"},
            adapter=adapter,
            project_name="waffle_shop",
            settings=ScenarioCaptureSettings(
                captured_at="2026-01-01T00:00:00Z",
                capture_adapter="duckdb",
                capture_dialect="duckdb",
                sqlbuild_version="0.0.0",
                retain=False,
            ),
        )

    assert tuple(event.event_type for event in lifecycle_events) == test_case.expected_event_types
    assert (
        tuple(event.payload.get("operation_name") for event in lifecycle_events)
        == test_case.expected_operation_names
    )
    assert tuple(adapter.events) == test_case.expected_connection_events


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioPlanningLifecycleTestCase(
            description="local planning failure has one failed scenario attempt",
            expected_resource_id="sql_scenario:local_planning_failure",
            expected_run_id="scenario-test-run",
            expected_terminal="resource_attempt_failed",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_local_scenario_planning_failure_when_running_then_canonical_attempt_fails(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    test_case: ScenarioPlanningLifecycleTestCase,
) -> None:
    pipeline_result: CompilePipelineResult = build_scenario_pipeline_result(
        scenario_names=("local_planning_failure",)
    )
    monkeypatch.setattr(
        scenario_pipeline,
        "build_scenario_plan",
        ScenarioPipelinePlanBuilder(
            planning_failure_name="local_planning_failure",
            error_message="local scenario planning failed",
        ),
    )
    lifecycle_events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=lifecycle_events.append, accepts_opaque=False)

    with dispatcher_scope(dispatcher):
        results: tuple[ScenarioRunResult, ...] = scenario_pipeline.run_scenario_local_test_pipeline(
            project_dir=tmp_path,
            pipeline_result=pipeline_result,
            scenarios=pipeline_result.project.sql_scenarios,
            adapter=ScenarioLocalPipelineTestAdapter(),
            project_name="waffle_shop",
            replay_source=ScenarioLocalReplaySource(lexical_syntax=SqlLexicalSyntax()),
        )

    assert results[0].status == ExecutionStatus.FAILED
    assert tuple(event.resource_id for event in lifecycle_events) == (
        test_case.expected_resource_id,
        test_case.expected_resource_id,
    )
    assert tuple(event.run_id for event in lifecycle_events) == (
        test_case.expected_run_id,
        test_case.expected_run_id,
    )
    assert lifecycle_events[-1].event_type == test_case.expected_terminal


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioLocalPipelineTestCase(
            description="missing snapshot is an error",
            snapshot_state="missing",
            load_error_message=None,
            expected_local_status="ERROR",
            expected_status="failed",
            expected_retained=False,
            expected_duckdb_exists=False,
            expected_error_code=SCENARIO_LOCAL_SNAPSHOT_MISSING,
        ),
        ScenarioLocalPipelineTestCase(
            description="stale snapshot is an error",
            snapshot_state="stale",
            load_error_message=None,
            expected_local_status="ERROR",
            expected_status="failed",
            expected_retained=False,
            expected_duckdb_exists=False,
            expected_error_code=SCENARIO_LOCAL_SNAPSHOT_STALE,
        ),
        ScenarioLocalPipelineTestCase(
            description="fresh snapshot pass keeps DuckDB by default",
            snapshot_state="fresh",
            load_error_message=None,
            expected_local_status="PASS",
            expected_status="success",
            expected_retained=True,
            expected_duckdb_exists=True,
            expected_error_code=None,
        ),
        ScenarioLocalPipelineTestCase(
            description="load error retains DuckDB",
            snapshot_state="fresh",
            load_error_message="bad jsonl",
            expected_local_status="ERROR",
            expected_status="failed",
            expected_retained=True,
            expected_duckdb_exists=True,
            expected_error_code=SCENARIO_LOCAL_JSONL_INVALID,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_selected_scenarios_when_running_local_scenario_pipeline_then_loads_or_errors(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    test_case: ScenarioLocalPipelineTestCase,
) -> None:
    pipeline_result: CompilePipelineResult = build_scenario_pipeline_result(
        scenario_names=("local_scenario",),
    )
    adapter: ScenarioLocalPipelineTestAdapter = ScenarioLocalPipelineTestAdapter()

    def classify_snapshot(
        *,
        project_dir: Path,
        scenario_plan: ScenarioExecutionPlan,
        capture_adapter: str | None = None,
        capture_dialect: str | None = None,
    ) -> ScenarioSnapshotStateResult:
        del capture_adapter, capture_dialect
        return ScenarioSnapshotStateResult(
            state=ScenarioSnapshotState(test_case.snapshot_state),
            manifest_path=project_dir
            / "tests"
            / "_scenario_snapshots"
            / scenario_plan.name
            / "scenario.json",
        )

    monkeypatch.setattr(local_scenario, "classify_scenario_snapshot_state", classify_snapshot)
    monkeypatch.setattr(
        local_scenario,
        "load_scenario_snapshot_into_duckdb",
        local_snapshot_loader_for_test_case(test_case),
    )
    monkeypatch.setattr(
        scenario_pipeline,
        "build_scenario_plan",
        ScenarioPipelinePlanBuilder(
            planning_failure_name="",
            error_message="scenario planning failed",
        ),
    )

    results: tuple[ScenarioRunResult, ...] = scenario_pipeline.run_scenario_local_test_pipeline(
        project_dir=tmp_path,
        pipeline_result=pipeline_result,
        scenarios=pipeline_result.project.sql_scenarios,
        adapter=adapter,
        project_name="waffle_shop",
        replay_source=ScenarioLocalReplaySource(lexical_syntax=SqlLexicalSyntax()),
    )

    result: ScenarioRunResult = results[0]
    duckdb_path: Path = (
        tmp_path / "target" / "run" / "scenarios" / "local_scenario" / "local.duckdb"
    )
    assert result.local_status == ScenarioLocalRunStatus(test_case.expected_local_status)
    assert result.status == ExecutionStatus(test_case.expected_status)
    assert result.retained is test_case.expected_retained
    assert duckdb_path.exists() is test_case.expected_duckdb_exists
    assert result.error_code == test_case.expected_error_code


class _CodedScenarioError(Exception):
    def __init__(self, *, code: str, message: str, help: str | None = None) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.help = help


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioFailureHelpTestCase(
            description="coded user error keeps no bug help when help is absent",
            error=_CodedScenarioError(code="S504", message="boom"),
            expected_help=None,
        ),
        ScenarioFailureHelpTestCase(
            description="coded user error preserves explicit help",
            error=_CodedScenarioError(code="S504", message="boom", help="Rename the source."),
            expected_help="Rename the source.",
        ),
        ScenarioFailureHelpTestCase(
            description="uncoded error keeps the internal bug help",
            error=RuntimeError("boom"),
            expected_help=(
                "This is likely a SQLBuild bug. Please file an issue with the scenario name."
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scenario_exception_when_resolving_help_then_avoids_bug_help_for_user_errors(
    test_case: ScenarioFailureHelpTestCase,
) -> None:
    assert scenario_pipeline._scenario_failure_help(test_case.error) == test_case.expected_help
