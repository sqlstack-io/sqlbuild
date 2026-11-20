"""A location sync resumes dynamic models whose pause holds expired, and only those."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import PlanOutput
from sqlbuild.executor.build.models import BuildExecutionResult, BuildRuntimeParams
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.dynamic_pauses._helpers.resume import resume_holds
from sqlbuild.executor.dynamic_pauses._helpers.sql import read_open_holds, setup_statements
from sqlbuild.executor.dynamic_pauses.classes.dynamic_hold import DynamicHold
from sqlbuild.executor.dynamic_pauses.main.hold_dynamic_models import (
    hold_dynamic_models,
)
from sqlbuild.executor.dynamic_pauses.main.resume_expired_holds import resume_expired_holds
from sqlbuild.executor.dynamic_pauses.models import OpenHold, ResumeOutcome
from tests.integration.src.sqlbuild.executor.dynamic_pauses._test_types import (
    DoubleCloseTestCase,
    QuietTokenTestCase,
    RefusedResumeTestCase,
    SyncDuringTakeoverTestCase,
    SyncResumeTestCase,
)
from tests.integration.src.sqlbuild.executor.dynamic_pauses.helpers import (
    LeaseProject,
    PausingDuckDbAdapter,
    RefusingResumeAdapter,
    SyncDuringTakeoverAdapter,
    TokenPausingDuckDbAdapter,
    build_lease_project,
    lease_project_case,
    pause_event_ids,
    pause_events,
    pause_table_names,
    plan_lease_project,
    recorded_events,
    seed_hold,
)

_GRACE_SECONDS: float = 600.0
_SYNC: str = "location-sync:warehouse"


@pytest.mark.parametrize(
    "test_case",
    [
        SyncResumeTestCase(
            description="an expired hold is resumed and closed by the sync",
            holds=(("dead_run", "daily_totals", 900),),
            hand_suspended=(),
            pause_table=True,
            expected_resumed=(("dead_run", "daily_totals"),),
            expected_states={"daily_totals": "ACTIVE"},
            expected_events=(
                ("paused", "dead_run", "dead_run", "daily_totals"),
                ("resumed", "dead_run", _SYNC, "daily_totals"),
            ),
        ),
        SyncResumeTestCase(
            description="a live hold is left alone",
            holds=(("busy_run", "daily_totals", 30),),
            hand_suspended=(),
            pause_table=True,
            expected_resumed=(),
            expected_states={"daily_totals": "SUSPENDED"},
            expected_events=(("paused", "busy_run", "busy_run", "daily_totals"),),
        ),
        SyncResumeTestCase(
            description="a model suspended by hand stays suspended",
            holds=(("dead_run", "daily_totals", 900),),
            hand_suspended=("weekly_totals",),
            pause_table=True,
            expected_resumed=(("dead_run", "daily_totals"),),
            expected_states={"daily_totals": "ACTIVE", "weekly_totals": "SUSPENDED"},
            expected_events=(
                ("paused", "dead_run", "dead_run", "daily_totals"),
                ("resumed", "dead_run", _SYNC, "daily_totals"),
            ),
        ),
        SyncResumeTestCase(
            description="an expired hold on a model another run holds live is left alone",
            holds=(("dead_run", "daily_totals", 900), ("busy_run", "daily_totals", 30)),
            hand_suspended=(),
            pause_table=True,
            expected_resumed=(),
            expected_states={"daily_totals": "SUSPENDED"},
            expected_events=(
                ("paused", "busy_run", "busy_run", "daily_totals"),
                ("paused", "dead_run", "dead_run", "daily_totals"),
            ),
        ),
        SyncResumeTestCase(
            description="of two models only the one whose hold expired is resumed",
            holds=(("dead_run", "daily_totals", 900), ("busy_run", "weekly_totals", 30)),
            hand_suspended=(),
            pause_table=True,
            expected_resumed=(("dead_run", "daily_totals"),),
            expected_states={"daily_totals": "ACTIVE", "weekly_totals": "SUSPENDED"},
            expected_events=(
                ("paused", "busy_run", "busy_run", "weekly_totals"),
                ("paused", "dead_run", "dead_run", "daily_totals"),
                ("resumed", "dead_run", _SYNC, "daily_totals"),
            ),
        ),
        SyncResumeTestCase(
            description="without a pause table the sync writes nothing",
            holds=(),
            hand_suspended=("daily_totals",),
            pause_table=False,
            expected_resumed=(),
            expected_states={"daily_totals": "SUSPENDED"},
            expected_events=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_pause_holds_when_a_location_syncs_then_only_expired_holds_are_resumed(
    test_case: SyncResumeTestCase, adapter: PausingDuckDbAdapter, connection: Any
) -> None:
    for run_id, name, idle_seconds in test_case.holds:
        seed_hold(
            adapter=adapter,
            connection=connection,
            run_id=run_id,
            name=name,
            idle_seconds=idle_seconds,
        )
    adapter.states.update({name: "SUSPENDED" for name in test_case.hand_suspended})

    resumed: tuple[OpenHold, ...] = resume_expired_holds(
        adapter=adapter,
        connection=connection,
        location=(None, "staging"),
        grace_seconds=_GRACE_SECONDS,
        recorded_by=_SYNC,
        quiet_token=None,
    ).resumed

    assert tuple((hold.run_id, hold.relation.name) for hold in resumed) == (
        test_case.expected_resumed
    )
    assert adapter.states == test_case.expected_states
    assert bool(pause_table_names(connection=connection)) == test_case.pause_table
    assert recorded_events(connection=connection) == list(test_case.expected_events)


@pytest.mark.parametrize(
    "test_case",
    [
        QuietTokenTestCase(
            description="a pause table unchanged since a pass left it quiet is not read",
            holds=(("dead_run", "daily_totals", 900),),
            tokens=True,
            quiet=True,
            expected_resumed=(),
            expected_quiet=True,
        ),
        QuietTokenTestCase(
            description="a pass that leaves no hold open returns the table's token",
            holds=(),
            tokens=True,
            quiet=False,
            expected_resumed=(),
            expected_quiet=True,
        ),
        QuietTokenTestCase(
            description="a pass that resumes returns no token, so the next pass reads again",
            holds=(("dead_run", "daily_totals", 900),),
            tokens=True,
            quiet=False,
            expected_resumed=(("dead_run", "daily_totals"),),
            expected_quiet=False,
        ),
        QuietTokenTestCase(
            description="a live hold keeps the table from going quiet",
            holds=(("busy_run", "daily_totals", 30),),
            tokens=True,
            quiet=False,
            expected_resumed=(),
            expected_quiet=False,
        ),
        QuietTokenTestCase(
            description="an adapter without change tokens reads every pass",
            holds=(("dead_run", "daily_totals", 900),),
            tokens=False,
            quiet=True,
            expected_resumed=(("dead_run", "daily_totals"),),
            expected_quiet=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_quiet_token_when_a_location_syncs_then_only_a_changed_pause_table_is_read(
    test_case: QuietTokenTestCase, connection: Any
) -> None:
    adapter: PausingDuckDbAdapter = {
        True: TokenPausingDuckDbAdapter,
        False: PausingDuckDbAdapter,
    }[test_case.tokens]()
    for statement in setup_statements(adapter=adapter, location=(None, "staging")):
        connection.execute(statement)
    for run_id, name, idle_seconds in test_case.holds:
        seed_hold(
            adapter=adapter,
            connection=connection,
            run_id=run_id,
            name=name,
            idle_seconds=idle_seconds,
        )
    unchanged: str | None = TokenPausingDuckDbAdapter().read_relation_change_token(
        connection=connection, database=None, schema="staging", name="_sqlbuild_dynamic_pauses"
    )
    token: str | None = {True: unchanged, False: None}[test_case.quiet]

    outcome: ResumeOutcome = resume_expired_holds(
        adapter=adapter,
        connection=connection,
        location=(None, "staging"),
        grace_seconds=_GRACE_SECONDS,
        recorded_by=_SYNC,
        quiet_token=token,
    )

    current: str | None = TokenPausingDuckDbAdapter().read_relation_change_token(
        connection=connection, database=None, schema="staging", name="_sqlbuild_dynamic_pauses"
    )
    assert tuple((hold.run_id, hold.relation.name) for hold in outcome.resumed) == (
        test_case.expected_resumed
    )
    assert outcome.quiet_token == {True: current, False: None}[test_case.expected_quiet]


@pytest.mark.parametrize(
    "test_case",
    [
        DoubleCloseTestCase(
            description="a build and a sync closing one expired hold write the same resume",
            closers=("second_run", _SYNC),
            expected_events=(
                ("paused", "dead_run", "dead_run", "daily_totals"),
                ("resumed", "dead_run", _SYNC, "daily_totals"),
                ("resumed", "dead_run", "second_run", "daily_totals"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_one_expired_hold_when_two_closers_resume_it_then_it_closes_once(
    test_case: DoubleCloseTestCase, adapter: PausingDuckDbAdapter, connection: Any
) -> None:
    seed_hold(
        adapter=adapter,
        connection=connection,
        run_id="dead_run",
        name="daily_totals",
        idle_seconds=900,
    )
    read: tuple[OpenHold, ...] = read_open_holds(
        adapter=adapter, connection=connection, location=(None, "staging")
    )
    for closer in test_case.closers:
        resume_holds(
            adapter=adapter,
            connection=connection,
            location=(None, "staging"),
            holds=read,
            recorded_by=closer,
        )

    assert pause_events(connection=connection) == list(test_case.expected_events)
    assert len({event_id for event_id, _, _ in pause_event_ids(connection=connection)}) == 1
    assert read_open_holds(adapter=adapter, connection=connection, location=(None, "staging")) == ()
    assert adapter.operations == [("resume", "daily_totals")]
    assert adapter.states == {"daily_totals": "ACTIVE"}


@pytest.mark.parametrize(
    "test_case",
    [
        SyncDuringTakeoverTestCase(
            description="a model a sync resumes while a build takes its hold over is paused again",
            expected_operations=(("suspend", "daily_totals"), ("resume", "daily_totals")),
            expected_states={"daily_totals": "ACTIVE"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_sync_resuming_mid_takeover_when_the_build_holds_then_it_pauses_the_model(
    test_case: SyncDuringTakeoverTestCase,
    database_path: Path,
) -> None:
    adapter: SyncDuringTakeoverAdapter = SyncDuringTakeoverAdapter(
        takeover_run="second_run", resumed={"daily_totals": "ACTIVE"}
    )
    connection: Any = adapter.connect({"database": str(database_path)})
    connection.execute("CREATE SCHEMA staging")
    case: LeaseProject = lease_project_case(
        orders_query="SELECT 1 AS order_id", report_locations={}, refresh_on_build=False
    )
    _ = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=case,
        runtime=BuildRuntimeParams(run_id="first_run"),
        database_path=database_path,
    )
    changed: LeaseProject = lease_project_case(
        orders_query="SELECT 1 AS order_id UNION ALL SELECT 2",
        report_locations={},
        refresh_on_build=False,
    )
    dead: DynamicHold | None = hold_dynamic_models(
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
        runtime=BuildRuntimeParams(run_id="second_run", dynamic_heartbeat_grace_seconds=0),
        database_path=database_path,
    )

    adapter.close(connection)
    assert result.status == BuildStatus.SUCCESS
    assert tuple(adapter.operations) == test_case.expected_operations
    assert adapter.states == test_case.expected_states


@pytest.mark.parametrize(
    "test_case",
    [
        RefusedResumeTestCase(
            description="a refused resume leaves that hold open and the others still close",
            refused=frozenset({"daily_totals"}),
            expected_resumed=(("dead_weekly", "weekly_totals"),),
            expected_failures=(
                "staging.daily_totals: insufficient privileges to resume daily_totals",
            ),
            expected_states={"daily_totals": "SUSPENDED", "weekly_totals": "ACTIVE"},
            expected_events=(
                ("paused", "dead_daily", "dead_daily", "daily_totals"),
                ("paused", "dead_weekly", "dead_weekly", "weekly_totals"),
                ("resumed", "dead_weekly", _SYNC, "weekly_totals"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_one_refused_resume_when_a_location_syncs_then_the_other_holds_still_close(
    test_case: RefusedResumeTestCase, connection: Any
) -> None:
    adapter: RefusingResumeAdapter = RefusingResumeAdapter(refused=test_case.refused)
    for run_id, name in (("dead_daily", "daily_totals"), ("dead_weekly", "weekly_totals")):
        seed_hold(
            adapter=adapter, connection=connection, run_id=run_id, name=name, idle_seconds=900
        )

    outcome: ResumeOutcome = resume_expired_holds(
        adapter=adapter,
        connection=connection,
        location=(None, "staging"),
        grace_seconds=_GRACE_SECONDS,
        recorded_by=_SYNC,
        quiet_token=None,
    )

    assert tuple((hold.run_id, hold.relation.name) for hold in outcome.resumed) == (
        test_case.expected_resumed
    )
    assert outcome.failures == test_case.expected_failures
    assert adapter.states == test_case.expected_states
    assert pause_events(connection=connection) == list(test_case.expected_events)


@pytest.mark.xfail(
    strict=True,
    reason=(
        "A sync that read the holds before a build took one over still resumes the model after "
        "the build's state check. Whether to close this gap, and how, waits on the owner's "
        "decision on the sync/takeover race (CHI-1569 review, finding 1)."
    ),
)
@pytest.mark.parametrize(
    "test_case",
    [
        SyncDuringTakeoverTestCase(
            description="a sync resuming from a stale read after the build's state check",
            expected_operations=(("resume", "daily_totals"),),
            expected_states={"daily_totals": "SUSPENDED"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_sync_resuming_after_the_takeover_state_check_when_the_build_holds_then_the_model_stays_paused(
    test_case: SyncDuringTakeoverTestCase,
    adapter: PausingDuckDbAdapter,
    connection: Any,
    database_path: Path,
) -> None:
    case: LeaseProject = lease_project_case(
        orders_query="SELECT 1 AS order_id", report_locations={}, refresh_on_build=False
    )
    _ = build_lease_project(
        adapter=adapter,
        connection=connection,
        case=case,
        runtime=BuildRuntimeParams(run_id="first_run"),
        database_path=database_path,
    )
    plan: PlanOutput = plan_lease_project(
        adapter=adapter,
        connection=connection,
        case=lease_project_case(
            orders_query="SELECT 1 AS order_id UNION ALL SELECT 2",
            report_locations={},
            refresh_on_build=False,
        ),
    )
    dead: DynamicHold | None = hold_dynamic_models(
        plan=plan,
        adapter=adapter,
        runtime=BuildRuntimeParams(run_id="dead_run"),
        connection_config={"database": str(database_path)},
    )
    assert dead is not None
    dead.acquire(connection=connection, on_progress=None)
    stale: tuple[OpenHold, ...] = read_open_holds(
        adapter=adapter, connection=connection, location=(None, "staging")
    )
    taking_over: DynamicHold | None = hold_dynamic_models(
        plan=plan,
        adapter=adapter,
        runtime=BuildRuntimeParams(run_id="second_run", dynamic_heartbeat_grace_seconds=0),
        connection_config={"database": str(database_path)},
    )
    assert taking_over is not None
    taking_over.acquire(connection=connection, on_progress=None)
    adapter.operations.clear()

    _ = resume_holds(
        adapter=adapter,
        connection=connection,
        location=(None, "staging"),
        holds=stale,
        recorded_by=_SYNC,
    )
    states: dict[str, str] = dict(adapter.states)
    operations: tuple[tuple[str, str], ...] = tuple(adapter.operations)
    taking_over.release(connection=connection)

    assert operations == test_case.expected_operations
    assert states == test_case.expected_states
