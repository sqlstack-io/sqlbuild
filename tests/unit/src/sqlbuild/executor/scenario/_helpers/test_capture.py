from __future__ import annotations

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import ScenarioArtifactKind, ScenarioExecutionPlan, SeedPlanEntry
from sqlbuild.executor.scenario._helpers.capture import core as capture_core
from sqlbuild.executor.scenario._helpers.capture.core import execute_scenario_snapshot_capture_steps
from sqlbuild.executor.scenario.models import (
    ScenarioCaptureSettings,
    ScenarioFixtureExecutionResult,
    ScenarioSnapshotCaptureRunResult,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.observability import EventDispatcher, LifecycleEvent, dispatcher_scope
from tests.unit.src.sqlbuild.executor.scenario._helpers._test_types import (
    ExecuteScenarioSnapshotCaptureStepsTestCase,
    ScenarioCaptureMissingFixtureMessageTestCase,
    ScenarioCaptureRunIdentityTestCase,
)
from tests.unit.src.sqlbuild.executor.scenario._helpers.helpers import (
    assert_capture_steps_error,
    resource_attempt_events,
)
from tests.unit.src.sqlbuild.executor.scenario.main.helpers import (
    ScenarioSnapshotCaptureStepsTestAdapter,
    build_scenario_cleanup_test_plan_with_project_seed,
)

SCENARIO_PLAN: ScenarioExecutionPlan = build_scenario_cleanup_test_plan_with_project_seed()


@pytest.mark.parametrize(
    "test_case",
    [
        ExecuteScenarioSnapshotCaptureStepsTestCase(
            description="captures inputs and cleans up by default",
            retain=False,
            fail_on_create_target=None,
            fail_on_seed=False,
            fail_on_query_target=None,
            expected_status=ExecutionStatus.SUCCESS,
            expected_retained=False,
            expected_fixture_result_count=2,
            expected_seed_result_count=1,
            expected_has_capture_result=True,
            expected_has_cleanup_result=True,
        ),
        ExecuteScenarioSnapshotCaptureStepsTestCase(
            description="retain skips final cleanup",
            retain=True,
            fail_on_create_target=None,
            fail_on_seed=False,
            fail_on_query_target=None,
            expected_status=ExecutionStatus.SUCCESS,
            expected_retained=True,
            expected_fixture_result_count=2,
            expected_seed_result_count=1,
            expected_has_capture_result=True,
            expected_has_cleanup_result=False,
        ),
        ExecuteScenarioSnapshotCaptureStepsTestCase(
            description="fixture failure skips capture and cleans up",
            retain=False,
            fail_on_create_target="raw__orders",
            fail_on_seed=False,
            fail_on_query_target=None,
            expected_status=ExecutionStatus.FAILED,
            expected_retained=False,
            expected_fixture_result_count=1,
            expected_seed_result_count=0,
            expected_has_capture_result=False,
            expected_has_cleanup_result=True,
            expected_error_fragment="fixture create failed",
        ),
        ExecuteScenarioSnapshotCaptureStepsTestCase(
            description="seed failure skips capture and cleans up",
            retain=False,
            fail_on_create_target=None,
            fail_on_seed=True,
            fail_on_query_target=None,
            expected_status=ExecutionStatus.FAILED,
            expected_retained=False,
            expected_fixture_result_count=2,
            expected_seed_result_count=1,
            expected_has_capture_result=False,
            expected_has_cleanup_result=True,
            expected_error_fragment="seed load failed",
        ),
        ExecuteScenarioSnapshotCaptureStepsTestCase(
            description="capture failure returns failed capture result and cleans up",
            retain=False,
            fail_on_create_target=None,
            fail_on_seed=False,
            fail_on_query_target="stg_customers",
            expected_status=ExecutionStatus.FAILED,
            expected_retained=False,
            expected_fixture_result_count=2,
            expected_seed_result_count=1,
            expected_has_capture_result=True,
            expected_has_cleanup_result=True,
            expected_error_fragment="warehouse read failed",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_plan_when_executing_snapshot_capture_steps_then_returns_expected_result(
    tmp_path: Path,
    test_case: ExecuteScenarioSnapshotCaptureStepsTestCase,
) -> None:
    adapter: ScenarioSnapshotCaptureStepsTestAdapter = ScenarioSnapshotCaptureStepsTestAdapter(
        fail_on_create_target=test_case.fail_on_create_target,
        fail_on_seed=test_case.fail_on_seed,
        fail_on_query_target=test_case.fail_on_query_target,
    )

    result: ScenarioSnapshotCaptureRunResult = execute_scenario_snapshot_capture_steps(
        project_dir=tmp_path,
        scenario_plan=SCENARIO_PLAN,
        adapter=adapter,
        connection=object(),
        run_id="capture-test-run",
        settings=ScenarioCaptureSettings(
            captured_at="2026-05-09T00:00:00Z",
            capture_adapter="duckdb",
            capture_dialect="duckdb",
            sqlbuild_version="0.1.0",
            retain=test_case.retain,
        ),
    )

    assert result.status == test_case.expected_status
    assert result.retained is test_case.expected_retained
    assert len(result.fixture_results) == test_case.expected_fixture_result_count
    assert len(result.seed_results) == test_case.expected_seed_result_count
    assert (result.capture_result is not None) is test_case.expected_has_capture_result
    assert (result.cleanup_result is not None) is test_case.expected_has_cleanup_result
    assert_capture_steps_error(result=result, test_case=test_case)


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioCaptureRunIdentityTestCase(
            description="capture operation and multiple seeds share project run identity",
            expected_run_id="capture-project-run",
            expected_seed_resource_ids=(
                "scenario:revenue__customer_refund:seed:country_codes",
                "scenario:revenue__customer_refund:seed:regions",
            ),
            expected_operation_name="scenario_capture",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_multiple_capture_seeds_when_executing_then_operation_and_resources_share_run_id(
    tmp_path: Path,
    test_case: ScenarioCaptureRunIdentityTestCase,
) -> None:
    first_seed: SeedPlanEntry = SCENARIO_PLAN.seed_entries[0]
    second_seed: SeedPlanEntry = replace(
        first_seed,
        key=replace(first_seed.key, name="regions"),
        name="regions",
        destination=replace(
            first_seed.destination,
            name="__sqb_51b385aebe20__seed__country_codes_regions",
            qualified_name="scenario_schema.__sqb_51b385aebe20__seed__country_codes_regions",
        ),
        file_path=Path("sql/seeds/regions.csv"),
    )
    scenario_plan: ScenarioExecutionPlan = replace(
        SCENARIO_PLAN,
        seed_entries=(first_seed, second_seed),
    )
    lifecycle_events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=lifecycle_events.append, accepts_opaque=False)

    with dispatcher_scope(dispatcher):
        result: ScenarioSnapshotCaptureRunResult = execute_scenario_snapshot_capture_steps(
            project_dir=tmp_path,
            scenario_plan=scenario_plan,
            adapter=ScenarioSnapshotCaptureStepsTestAdapter(),
            connection=object(),
            run_id=test_case.expected_run_id,
            settings=ScenarioCaptureSettings(
                captured_at="2026-05-09T00:00:00Z",
                capture_adapter="duckdb",
                capture_dialect="duckdb",
                sqlbuild_version="0.1.0",
                retain=True,
            ),
        )

    assert result.status == ExecutionStatus.SUCCESS
    assert len(result.seed_results) == 2
    assert tuple(event.run_id for event in lifecycle_events) == (test_case.expected_run_id,) * len(
        lifecycle_events
    )
    assert lifecycle_events[0].payload["operation_name"] == test_case.expected_operation_name
    resource_events: tuple[LifecycleEvent, ...] = resource_attempt_events(lifecycle_events)
    assert tuple(event.resource_id for event in resource_events) == (
        test_case.expected_seed_resource_ids[0],
        test_case.expected_seed_resource_ids[0],
        test_case.expected_seed_resource_ids[1],
        test_case.expected_seed_resource_ids[1],
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioCaptureMissingFixtureMessageTestCase(
            description="failed fixture without a message uses the capture fallback message",
            fixture_error_message=None,
            expected_error_message="scenario snapshot capture failed",
        ),
        ScenarioCaptureMissingFixtureMessageTestCase(
            description="failed fixture with an empty message uses the capture fallback message",
            fixture_error_message="",
            expected_error_message="scenario snapshot capture failed",
        ),
        ScenarioCaptureMissingFixtureMessageTestCase(
            description="failed fixture message is reported unchanged",
            fixture_error_message="orders fixture failed",
            expected_error_message="orders fixture failed",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_failed_fixture_message_when_capturing_then_reports_message_or_fallback(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    test_case: ScenarioCaptureMissingFixtureMessageTestCase,
) -> None:
    failed_fixture: ScenarioFixtureExecutionResult = ScenarioFixtureExecutionResult(
        scenario_name=SCENARIO_PLAN.name,
        kind=ScenarioArtifactKind.SOURCE,
        logical_name="raw__orders",
        target_relation="scenario_schema.raw__orders",
        status=ExecutionStatus.FAILED,
        error_message=test_case.fixture_error_message,
    )
    monkeypatch.setattr(
        capture_core, "execute_scenario_fixtures", lambda **_kwargs: (failed_fixture,)
    )

    result: ScenarioSnapshotCaptureRunResult = execute_scenario_snapshot_capture_steps(
        project_dir=tmp_path,
        scenario_plan=SCENARIO_PLAN,
        adapter=ScenarioSnapshotCaptureStepsTestAdapter(),
        connection=object(),
        run_id="capture-test-run",
        settings=ScenarioCaptureSettings(
            captured_at="2026-05-09T00:00:00Z",
            capture_adapter="duckdb",
            capture_dialect="duckdb",
            sqlbuild_version="0.1.0",
            retain=False,
        ),
    )

    assert result.status == ExecutionStatus.FAILED
    assert result.error_message == test_case.expected_error_message
