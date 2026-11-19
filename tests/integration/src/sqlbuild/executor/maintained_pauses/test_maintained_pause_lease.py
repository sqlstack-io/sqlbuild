"""A build pauses the maintained models it touches, records its hold, and resumes them at the end."""

from __future__ import annotations

import time
from pathlib import Path
from typing import Any

import pytest

from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.executor.build.models import BuildExecutionResult, BuildRuntimeParams
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.maintained_pauses.classes.maintained_hold import MaintainedHold
from sqlbuild.executor.maintained_pauses.main.hold_maintained_models import (
    hold_maintained_models,
)
from tests.integration.src.sqlbuild.executor.maintained_pauses._test_types import (
    FirstBuildTestCase,
    HeartbeatTestCase,
    HoldRefusalTestCase,
    HoldTakeoverTestCase,
    PauseLeaseTestCase,
)
from tests.integration.src.sqlbuild.executor.maintained_pauses.helpers import (
    TOTALS_QUERY,
    LeaseProject,
    PausingDuckDbAdapter,
    build_lease_project,
    lease_project_case,
    pause_events,
    pause_table_names,
    plan_lease_project,
)

_ORDERS: str = "SELECT 1 AS order_id"
_MORE_ORDERS: str = "SELECT 1 AS order_id UNION ALL SELECT 2"
_REPORT: dict[str, str] = {"order_report": "staging"}
_FIRST_HOLD: tuple[tuple[str, str, str, str], ...] = (
    ("paused", "first_run", "first_run", "daily_totals"),
    ("resumed", "first_run", "first_run", "daily_totals"),
)
_SECOND_HOLD: tuple[tuple[str, str, str, str], ...] = (
    ("paused", "second_run", "second_run", "daily_totals"),
    ("resumed", "second_run", "second_run", "daily_totals"),
)
_TAKEN_OVER: tuple[tuple[str, str, str, str], ...] = (
    ("paused", "dead_run", "dead_run", "daily_totals"),
    ("paused", "first_run", "first_run", "daily_totals"),
    ("paused", "second_run", "second_run", "daily_totals"),
    ("resumed", "dead_run", "second_run", "daily_totals"),
    ("resumed", "first_run", "first_run", "daily_totals"),
    ("resumed", "second_run", "second_run", "daily_totals"),
)


@pytest.mark.parametrize(
    "test_case",
    [
        FirstBuildTestCase(
            description="a created model is paused for the build and resumed at its end",
            pause_maintained=False,
            expected_operations=(
                ("create", "daily_totals"),
                ("suspend", "daily_totals"),
                ("resume", "daily_totals"),
            ),
            expected_states={"daily_totals": "ACTIVE"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_first_build_when_creating_maintained_model_then_pauses_it_until_the_end(
    test_case: FirstBuildTestCase,
    adapter: PausingDuckDbAdapter,
    connection: Any,
    database_path: Path,
) -> None:
    result: BuildExecutionResult = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=lease_project_case(orders_query=_ORDERS, report_locations={}, refresh_on_build=False),
        runtime=BuildRuntimeParams(run_id="first_run", pause_maintained=test_case.pause_maintained),
        database_path=database_path,
    )

    assert result.status == BuildStatus.SUCCESS
    assert tuple(adapter.operations) == test_case.expected_operations
    assert adapter.states == test_case.expected_states
    assert pause_events(connection=connection) == sorted(_FIRST_HOLD)


@pytest.mark.parametrize(
    "test_case",
    [
        PauseLeaseTestCase(
            description="a build of the maintained model alone writes nothing it reads",
            second_select=("daily_totals",),
            second_orders_query=_ORDERS,
            second_totals_query=TOTALS_QUERY,
            report_locations={},
            refresh_on_build=False,
            states_before_second={},
            expected_operations=(),
            expected_events=_FIRST_HOLD,
        ),
        PauseLeaseTestCase(
            description="a rebuilt upstream pauses the model for the build",
            second_select=(),
            second_orders_query=_MORE_ORDERS,
            second_totals_query=TOTALS_QUERY,
            report_locations={},
            refresh_on_build=False,
            states_before_second={},
            expected_operations=(("suspend", "daily_totals"), ("resume", "daily_totals")),
            expected_events=(*_FIRST_HOLD, *_SECOND_HOLD),
        ),
        PauseLeaseTestCase(
            description="a model a person suspended is neither paused nor resumed",
            second_select=(),
            second_orders_query=_MORE_ORDERS,
            second_totals_query=TOTALS_QUERY,
            report_locations={},
            refresh_on_build=False,
            states_before_second={"daily_totals": "SUSPENDED"},
            expected_operations=(),
            expected_events=_FIRST_HOLD,
        ),
        PauseLeaseTestCase(
            description="a model a person suspended stays suspended when it is recreated",
            second_select=(),
            second_orders_query=_ORDERS,
            second_totals_query="SELECT count(*) + 0 AS orders FROM staging.orders",
            report_locations={},
            refresh_on_build=False,
            states_before_second={"daily_totals": "SUSPENDED"},
            expected_operations=(("create", "daily_totals"), ("suspend", "daily_totals")),
            expected_events=_FIRST_HOLD,
        ),
        PauseLeaseTestCase(
            description="a table built from the model refreshes it first",
            second_select=(),
            second_orders_query=_MORE_ORDERS,
            second_totals_query=TOTALS_QUERY,
            report_locations=_REPORT,
            refresh_on_build=False,
            states_before_second={},
            expected_operations=(
                ("suspend", "daily_totals"),
                ("refresh", "daily_totals"),
                ("resume", "daily_totals"),
            ),
            expected_events=(*_FIRST_HOLD, *_SECOND_HOLD),
        ),
        PauseLeaseTestCase(
            description="refresh_on_build refreshes a kept model",
            second_select=(),
            second_orders_query=_MORE_ORDERS,
            second_totals_query=TOTALS_QUERY,
            report_locations={},
            refresh_on_build=True,
            states_before_second={},
            expected_operations=(
                ("suspend", "daily_totals"),
                ("refresh", "daily_totals"),
                ("resume", "daily_totals"),
            ),
            expected_events=(*_FIRST_HOLD, *_SECOND_HOLD),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_maintained_model_when_building_again_then_pauses_only_what_it_touches(
    test_case: PauseLeaseTestCase,
    adapter: PausingDuckDbAdapter,
    connection: Any,
    database_path: Path,
) -> None:
    _ = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=lease_project_case(
            orders_query=_ORDERS,
            report_locations=test_case.report_locations,
            refresh_on_build=test_case.refresh_on_build,
        ),
        runtime=BuildRuntimeParams(run_id="first_run"),
        database_path=database_path,
    )
    adapter.operations.clear()
    adapter.states.update(test_case.states_before_second)

    result: BuildExecutionResult = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=lease_project_case(
            orders_query=test_case.second_orders_query,
            report_locations=test_case.report_locations,
            refresh_on_build=test_case.refresh_on_build,
            totals_query=test_case.second_totals_query,
        ),
        runtime=BuildRuntimeParams(run_id="second_run"),
        database_path=database_path,
        select=test_case.second_select,
    )

    assert result.status == BuildStatus.SUCCESS
    assert tuple(adapter.operations) == test_case.expected_operations
    assert pause_events(connection=connection) == sorted(test_case.expected_events)


@pytest.mark.parametrize(
    "test_case",
    [
        FirstBuildTestCase(
            description="a pause_maintained target leaves models suspended and records nothing",
            pause_maintained=True,
            expected_operations=(("refresh", "daily_totals"),),
            expected_states={"daily_totals": "SUSPENDED"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_pause_maintained_target_when_building_then_models_stay_suspended_and_refresh(
    test_case: FirstBuildTestCase,
    adapter: PausingDuckDbAdapter,
    connection: Any,
    database_path: Path,
) -> None:
    paused: BuildRuntimeParams = BuildRuntimeParams(
        run_id="first_run", pause_maintained=test_case.pause_maintained
    )
    _ = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=lease_project_case(orders_query=_ORDERS, report_locations={}, refresh_on_build=False),
        runtime=paused,
        database_path=database_path,
    )
    first: list[tuple[str, str]] = list(adapter.operations)
    adapter.operations.clear()

    result: BuildExecutionResult = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=lease_project_case(
            orders_query=_MORE_ORDERS, report_locations={}, refresh_on_build=False
        ),
        runtime=paused,
        database_path=database_path,
    )

    assert result.status == BuildStatus.SUCCESS
    assert first == [("create", "daily_totals"), ("suspend", "daily_totals")]
    assert tuple(adapter.operations) == test_case.expected_operations
    assert adapter.states == test_case.expected_states
    assert pause_table_names(connection=connection) == []


@pytest.mark.parametrize(
    "test_case",
    [
        HoldTakeoverTestCase(
            description="a hold silent past the grace is taken over",
            grace_seconds=0,
            ignore_grace=False,
            allow_ignore_grace=False,
            expected_operations=(("resume", "daily_totals"),),
            expected_events=_TAKEN_OVER,
        ),
        HoldTakeoverTestCase(
            description="--ignore-maintained-grace takes over a live hold when the project allows",
            grace_seconds=1800,
            ignore_grace=True,
            allow_ignore_grace=True,
            expected_operations=(("resume", "daily_totals"),),
            expected_events=_TAKEN_OVER,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_hold_of_a_stopped_build_when_building_then_takes_it_over_and_resumes_at_the_end(
    test_case: HoldTakeoverTestCase,
    adapter: PausingDuckDbAdapter,
    connection: Any,
    database_path: Path,
) -> None:
    _ = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=lease_project_case(orders_query=_ORDERS, report_locations={}, refresh_on_build=False),
        runtime=BuildRuntimeParams(run_id="first_run"),
        database_path=database_path,
    )
    changed: LeaseProject = lease_project_case(
        orders_query=_MORE_ORDERS, report_locations={}, refresh_on_build=False
    )
    dead: MaintainedHold | None = hold_maintained_models(
        plan=plan_lease_project(adapter=adapter, connection=connection, case=changed),
        adapter=adapter,
        runtime=BuildRuntimeParams(run_id="dead_run"),
        connection_config={"database": str(database_path)},
    )
    assert dead is not None
    dead.acquire(connection=connection, on_progress=None)
    adapter.operations.clear()

    result: BuildExecutionResult = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=changed,
        runtime=BuildRuntimeParams(
            run_id="second_run",
            maintained_heartbeat_grace_seconds=test_case.grace_seconds,
            ignore_maintained_grace=test_case.ignore_grace,
            allow_ignore_maintained_grace=test_case.allow_ignore_grace,
        ),
        database_path=database_path,
    )

    assert result.status == BuildStatus.SUCCESS
    assert tuple(adapter.operations) == test_case.expected_operations
    assert adapter.states == {"daily_totals": "ACTIVE"}
    assert pause_events(connection=connection) == sorted(test_case.expected_events)


@pytest.mark.parametrize(
    "test_case",
    [
        HoldRefusalTestCase(
            description="a live hold stops the build and names the run holding it",
            grace_seconds=1800,
            ignore_grace=False,
            allow_ignore_grace=False,
            expected_code="A718",
            expected_message_fragment="dead_run",
        ),
        HoldRefusalTestCase(
            description="--ignore-maintained-grace needs allow_ignore_grace",
            grace_seconds=1800,
            ignore_grace=True,
            allow_ignore_grace=False,
            expected_code="A719",
            expected_message_fragment="allow_ignore_grace",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_live_hold_when_building_then_stops_before_any_change(
    test_case: HoldRefusalTestCase,
    adapter: PausingDuckDbAdapter,
    connection: Any,
    database_path: Path,
) -> None:
    _ = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=lease_project_case(orders_query=_ORDERS, report_locations={}, refresh_on_build=False),
        runtime=BuildRuntimeParams(run_id="first_run"),
        database_path=database_path,
    )
    changed: LeaseProject = lease_project_case(
        orders_query=_MORE_ORDERS, report_locations={}, refresh_on_build=False
    )
    dead: MaintainedHold | None = hold_maintained_models(
        plan=plan_lease_project(adapter=adapter, connection=connection, case=changed),
        adapter=adapter,
        runtime=BuildRuntimeParams(run_id="dead_run"),
        connection_config={"database": str(database_path)},
    )
    assert dead is not None
    dead.acquire(connection=connection, on_progress=None)
    adapter.operations.clear()

    with pytest.raises(AdapterUserError) as raised:
        _ = build_lease_project(
            adapter=adapter,
            connection=connection,
            case=changed,
            runtime=BuildRuntimeParams(
                run_id="second_run",
                maintained_heartbeat_grace_seconds=test_case.grace_seconds,
                ignore_maintained_grace=test_case.ignore_grace,
                allow_ignore_maintained_grace=test_case.allow_ignore_grace,
            ),
            database_path=database_path,
        )

    assert raised.value.code == test_case.expected_code
    assert test_case.expected_message_fragment in str(raised.value)
    assert adapter.operations == []
    assert connection.execute("SELECT count(*) FROM staging.orders").fetchall() == [(1,)]


@pytest.mark.parametrize(
    "test_case",
    [
        HeartbeatTestCase(
            description="a hold beats on its own connection while the build runs",
            heartbeat_seconds=0.05,
            wait_seconds=0.4,
            expected_min_beats=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_held_models_when_build_runs_then_heartbeat_writes_on_its_own_connection(
    test_case: HeartbeatTestCase,
    adapter: PausingDuckDbAdapter,
    connection: Any,
    database_path: Path,
) -> None:
    _ = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=lease_project_case(orders_query=_ORDERS, report_locations={}, refresh_on_build=False),
        runtime=BuildRuntimeParams(run_id="first_run"),
        database_path=database_path,
    )
    changed: LeaseProject = lease_project_case(
        orders_query=_MORE_ORDERS, report_locations={}, refresh_on_build=False
    )
    hold: MaintainedHold | None = hold_maintained_models(
        plan=plan_lease_project(adapter=adapter, connection=connection, case=changed),
        adapter=adapter,
        runtime=BuildRuntimeParams(run_id="beating_run"),
        connection_config={"database": str(database_path)},
        heartbeat_seconds=test_case.heartbeat_seconds,
    )
    assert hold is not None

    hold.acquire(connection=connection, on_progress=None)
    time.sleep(test_case.wait_seconds)
    hold.release(connection=connection)

    beats: list[tuple[Any, ...]] = connection.execute(
        "SELECT count(*) FROM staging._sqlbuild_maintained_pauses "
        "WHERE event_type = 'heartbeat' AND run_id = 'beating_run'"
    ).fetchall()
    assert beats[0][0] >= test_case.expected_min_beats
