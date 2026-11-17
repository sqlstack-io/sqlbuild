from __future__ import annotations

import threading

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledResourceType,
    Dialect,
    PlannedSqlTests,
    PlannerInputError,
    SqlTestPlanning,
    SqlTestSelection,
    start_sql_test_planning,
)
from tests.unit.src.sqlbuild.compiler.planner.classes._test_types import (
    BackgroundSqlTestPlanningTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner.classes.helpers import SqlTestsProject

_KEYS: frozenset[CompiledObjectKey] = frozenset(
    {CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders")}
)


@pytest.mark.parametrize(
    "test_case",
    [
        BackgroundSqlTestPlanningTestCase(
            description="enabled planning reads the project's SQL tests once",
            enabled=True,
            expected_planner_calls=1,
        ),
        BackgroundSqlTestPlanningTestCase(
            description="disabled planning never plans and returns no tests",
            enabled=False,
            expected_planner_calls=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_planning_when_joining_then_returns_planned_tests(
    test_case: BackgroundSqlTestPlanningTestCase,
) -> None:
    reads: list[int] = []

    def read_tests() -> tuple[object, ...]:
        reads.append(1)
        return ()

    planning: SqlTestPlanning = start_sql_test_planning(
        sql_test_selection=SqlTestSelection(),
        project=SqlTestsProject(read_tests),  # ty: ignore[invalid-argument-type]
        dialect=Dialect(object()),  # ty: ignore[invalid-argument-type]
        selected_keys=_KEYS,
        enabled=test_case.enabled,
    )
    result: PlannedSqlTests = planning.result()

    assert result == PlannedSqlTests(selected_keys=_KEYS)
    assert len(reads) == test_case.expected_planner_calls


@pytest.mark.parametrize(
    "test_case",
    [
        BackgroundSqlTestPlanningTestCase(
            description="planning errors surface when joining",
            enabled=True,
            expected_planner_calls=1,
            expected_error="Invalid SQL test fixtures",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_planning_when_joining_then_raises_original_error(
    test_case: BackgroundSqlTestPlanningTestCase,
) -> None:
    def read_tests() -> tuple[object, ...]:
        raise PlannerInputError(str(test_case.expected_error))

    planning: SqlTestPlanning = start_sql_test_planning(
        sql_test_selection=SqlTestSelection(),
        project=SqlTestsProject(read_tests),  # ty: ignore[invalid-argument-type]
        dialect=Dialect(object()),  # ty: ignore[invalid-argument-type]
        selected_keys=_KEYS,
        enabled=test_case.enabled,
    )
    with pytest.raises(PlannerInputError, match=str(test_case.expected_error)):
        planning.result()


@pytest.mark.parametrize(
    "test_case",
    [
        BackgroundSqlTestPlanningTestCase(
            description="running planning leaves no Python thread to keep the process alive",
            enabled=True,
            expected_planner_calls=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_running_planning_when_listing_threads_then_no_python_worker_blocks_exit(
    test_case: BackgroundSqlTestPlanningTestCase,
) -> None:
    started: threading.Event = threading.Event()
    release: threading.Event = threading.Event()
    reads: list[int] = []

    def read_tests() -> tuple[object, ...]:
        reads.append(1)
        started.set()
        release.wait(timeout=5)
        return ()

    planning: SqlTestPlanning = start_sql_test_planning(
        sql_test_selection=SqlTestSelection(),
        project=SqlTestsProject(read_tests),  # ty: ignore[invalid-argument-type]
        dialect=Dialect(object()),  # ty: ignore[invalid-argument-type]
        selected_keys=_KEYS,
        enabled=test_case.enabled,
    )
    assert started.wait(timeout=5)
    daemon_by_name: dict[str, bool] = {
        thread.name: thread.daemon for thread in threading.enumerate()
    }
    release.set()
    planning.result()

    assert daemon_by_name.get("sqlbuild-test-planning", True) is True
    assert len(reads) == test_case.expected_planner_calls
