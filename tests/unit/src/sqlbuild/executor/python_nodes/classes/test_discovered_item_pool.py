"""Discovered items with `max_parallel` run side by side on both serial paths."""

from __future__ import annotations

import threading
import time
from collections.abc import Callable
from functools import partial
from pathlib import Path

import pytest

from sqlbuild._native import PythonNodeGraph
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.python_nodes.models import PythonNodeExecutionResult, PythonNodeRuntime
from tests.unit.src.sqlbuild.executor.python_nodes.classes._test_types import (
    ItemPoolCancelTestCase,
    ItemPoolCapTestCase,
    ItemPoolFailureTestCase,
    ItemPoolIgnoredCancelTestCase,
    ItemPoolMidSubmitTestCase,
    ItemPoolOrderTestCase,
    ItemPoolRecordedInterruptTestCase,
    ItemPoolRepeatedCancelTestCase,
    ItemPoolResultWriteTestCase,
    ItemPoolSerialTestCase,
    ItemPoolWriteTestCase,
)
from tests.unit.src.sqlbuild.executor.python_nodes.classes.helpers import (
    ItemPoolTestAdapter,
    ItemProbe,
    SlowResultAdapter,
    fail_item,
    finishing_probe,
    held_until,
    ignoring_probe,
    interrupt_after_next_record,
    interrupt_second_submit,
    interrupted_probe,
    item_graph,
    meet_at,
    run_ingress,
    run_persisting_read_side,
    run_read_side,
    run_shape,
    run_writing_ingress,
    run_writing_read_side,
    twice_cancelled_probe,
    writing_graph,
)

_PATHS: tuple[Callable[..., tuple[PythonNodeExecutionResult, ...]], ...] = (
    run_read_side,
    run_ingress,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolCapTestCase(
            description="six items capped at two",
            items=tuple((f"fulfil[{index}]", "plan", 2) for index in range(6)),
            meet=2,
            expected_connections=2,
            expected_most_at_once=2,
        ),
        ItemPoolCapTestCase(
            description="six items capped at three",
            items=tuple((f"fulfil[{index}]", "plan", 3) for index in range(6)),
            meet=3,
            expected_connections=3,
            expected_most_at_once=3,
        ),
        ItemPoolCapTestCase(
            description="two assets on one set share the cap",
            items=(
                ("bill[0]", "plan", 2),
                ("bill[1]", "plan", 2),
                ("bill[2]", "plan", 2),
                ("fulfil[0]", "plan", 2),
                ("fulfil[1]", "plan", 2),
                ("fulfil[2]", "plan", 2),
            ),
            meet=2,
            expected_connections=2,
            expected_most_at_once=2,
        ),
        ItemPoolCapTestCase(
            description="two sets each have their own cap",
            items=(
                ("bill[0]", "billing", 2),
                ("bill[1]", "billing", 2),
                ("fulfil[0]", "plan", 3),
                ("fulfil[1]", "plan", 3),
                ("fulfil[2]", "plan", 3),
            ),
            meet=5,
            expected_connections=5,
            expected_most_at_once=5,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_ready_items_when_running_then_the_cap_is_reached_and_never_exceeded(
    test_case: ItemPoolCapTestCase,
) -> None:
    for run_path in _PATHS:
        barrier: threading.Barrier = threading.Barrier(test_case.meet)
        probe: ItemProbe = ItemProbe(
            actions={name: partial(meet_at, barrier) for name, _, _ in test_case.items}
        )
        adapter: ItemPoolTestAdapter = ItemPoolTestAdapter()
        graph: PythonNodeGraph = item_graph(
            tuple((name, set_name, limit, ()) for name, set_name, limit in test_case.items)
        )

        results: tuple[PythonNodeExecutionResult, ...] = run_path(
            graph=graph, adapter=adapter, probe=probe
        )

        assert [result.error_message for result in results] == [None] * len(test_case.items)
        assert (adapter.connections, probe.most_at_once) == (
            test_case.expected_connections,
            test_case.expected_most_at_once,
        )
        assert adapter.closed == adapter.connections
        assert probe.report_threads == {threading.get_ident()}
        assert probe.start_threads == {threading.get_ident()}


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolOrderTestCase(
            description="a diamond runs its middle together and its ends alone",
            items=(
                ("a", 4, ()),
                ("b", 4, ("a",)),
                ("c", 4, ("a",)),
                ("d", 4, ("b", "c")),
            ),
            meeting=("b", "c"),
            expected_seen_before={
                "a": frozenset(),
                "b": frozenset({"a"}),
                "c": frozenset({"a"}),
                "d": frozenset({"a", "b", "c"}),
            },
        ),
        ItemPoolOrderTestCase(
            description="an unset set runs serially beside a parallel one's dependencies",
            items=(("a", None, ()), ("b", 2, ("a",)), ("c", 2, ("a",)), ("d", None, ("b", "c"))),
            meeting=("b", "c"),
            expected_seen_before={
                "a": frozenset(),
                "b": frozenset({"a"}),
                "c": frozenset({"a"}),
                "d": frozenset({"a", "b", "c"}),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dependencies_when_running_then_an_item_starts_after_its_parents_are_reported(
    test_case: ItemPoolOrderTestCase,
) -> None:
    for run_path in _PATHS:
        barrier: threading.Barrier = threading.Barrier(len(test_case.meeting))
        probe: ItemProbe = ItemProbe(
            actions={name: partial(meet_at, barrier) for name in test_case.meeting}
        )
        graph: PythonNodeGraph = item_graph(
            tuple(
                (name, f"set-{limit}", limit, parents) for name, limit, parents in test_case.items
            )
        )

        _ = run_path(graph=graph, adapter=ItemPoolTestAdapter(), probe=probe)

        assert probe.seen_before == test_case.expected_seen_before
        assert {name: name in before for name, before in probe.started_before.items()} == (
            dict.fromkeys(test_case.expected_seen_before, True)
        )


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolSerialTestCase(
            description="unset is serial",
            max_parallel=None,
            adapter_name="item-pool-test",
            expected_threads=1,
            expected_connections=0,
        ),
        ItemPoolSerialTestCase(
            description="one is serial",
            max_parallel=1,
            adapter_name="item-pool-test",
            expected_threads=1,
            expected_connections=0,
        ),
        ItemPoolSerialTestCase(
            description="a database private to its connection stays serial",
            max_parallel=4,
            adapter_name="duckdb",
            expected_threads=1,
            expected_connections=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_parallelism_when_running_then_items_run_one_by_one_on_the_calling_thread(
    test_case: ItemPoolSerialTestCase,
) -> None:
    for run_path in _PATHS:
        probe: ItemProbe = ItemProbe()
        adapter: ItemPoolTestAdapter = ItemPoolTestAdapter(adapter_name=test_case.adapter_name)
        graph: PythonNodeGraph = item_graph(
            tuple((name, "plan", test_case.max_parallel, ()) for name in ("a", "b", "c"))
        )

        _ = run_path(graph=graph, adapter=adapter, probe=probe)

        assert probe.threads == {threading.get_ident()}
        assert (len(probe.threads), adapter.connections, probe.most_at_once) == (
            test_case.expected_threads,
            test_case.expected_connections,
            1,
        )
        assert probe.seen_before["c"] == frozenset({"a", "b"})


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolFailureTestCase("a failure", "fail", "failed"),
        ItemPoolFailureTestCase("a hard skip", "hard", "skipped"),
        ItemPoolFailureTestCase("a soft skip", "soft", "skipped"),
    ],
    ids=lambda case: case.description,
)
def test_given_an_item_that_does_not_succeed_when_running_then_dependants_end_as_in_serial(
    test_case: ItemPoolFailureTestCase,
) -> None:
    for run_path in _PATHS:
        outcomes: dict[str, Callable[[], object]] = {
            "fail": fail_item,
            "hard": lambda: "hard",
            "soft": lambda: "soft",
        }
        shape: tuple[tuple[str, tuple[str, ...]], ...] = (
            ("a", ()),
            ("b", ("a",)),
            ("c", ("b",)),
            ("d", ()),
            ("e", ("a", "d")),
        )
        action: Callable[[], object] = outcomes[test_case.outcome]
        serial: frozenset[tuple[str, str, str | None, str | None]] = run_shape(
            run_path=run_path, shape=shape, limit=None, action=action
        )
        parallel: frozenset[tuple[str, str, str | None, str | None]] = run_shape(
            run_path=run_path, shape=shape, limit=3, action=action
        )

        assert parallel == serial
        assert {row[0]: row[1] for row in parallel}["a"] == test_case.expected_first_status


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolCancelTestCase(
            description="an interrupt with one step finished and one running",
            expected_reported=frozenset({("quick", "success")}),
            expected_cancelled=("slow",),
            expected_never_started="waiting",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_steps_in_flight_when_interrupted_then_finished_stay_succeeded_and_nothing_starts(
    test_case: ItemPoolCancelTestCase,
) -> None:
    for run_path in _PATHS:
        probe: ItemProbe = interrupted_probe()
        adapter: ItemPoolTestAdapter = ItemPoolTestAdapter()
        graph: PythonNodeGraph = item_graph(
            (("quick", "plan", 2, ()), ("slow", "plan", 2, ()), ("waiting", "plan", 2, ("quick",)))
        )

        with pytest.raises(KeyboardInterrupt):
            _ = run_path(graph=graph, adapter=adapter, probe=probe)

        reported: frozenset[tuple[str, str]] = frozenset(
            (result.node_name, result.status.value) for result in probe.reported
        )
        assert (reported, tuple(probe.cancelled)) == (
            test_case.expected_reported,
            test_case.expected_cancelled,
        )
        assert test_case.expected_never_started not in probe.seen_before
        assert adapter.closed == adapter.connections


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolWriteTestCase(
            description="four steps create the result table and write at once",
            steps=("label", "pack", "pick", "ship"),
            expected_status="success",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_steps_using_sql_when_they_write_results_then_they_do_not_collide(
    test_case: ItemPoolWriteTestCase,
    tmp_path: Path,
) -> None:
    for run_path in (run_writing_read_side, run_writing_ingress):
        adapter: DuckDbAdapter = DuckDbAdapter()
        config: dict[str, object] = {"database": str(tmp_path / f"{run_path.__name__}.duckdb")}
        connection: object = adapter.connect(config)
        runtime: PythonNodeRuntime = PythonNodeRuntime(
            adapter=adapter,
            connection_config=config,
            connection=connection,
            run_id="test_run",
            target="dev",
            vars={"barrier": threading.Barrier(len(test_case.steps))},
            is_reload=False,
            default_schema="plan_state",
        )

        results: tuple[PythonNodeExecutionResult, ...] = run_path(
            graph=writing_graph(test_case.steps), runtime=runtime
        )

        assert {
            (result.node_name, result.status.value, result.error_message) for result in results
        } == {(step, test_case.expected_status, None) for step in test_case.steps}
        found: object = adapter.query(
            connection=connection,
            sql="SELECT table_name FROM information_schema.columns "
            "WHERE table_schema = 'plan_state' AND column_name = 'payload_json_b64'",
            limit=None,
        ).rows[0][0]
        table: str = str(found)
        written: set[object] = {
            row[0]
            for row in adapter.query(
                connection=connection, sql=f"SELECT node_name FROM plan_state.{table}", limit=None
            ).rows
        }
        adapter.close(connection)
        assert written == set(test_case.steps)


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolRecordedInterruptTestCase(
            description="two steps running when the interrupt follows the first's record",
            items=("bill", "pack"),
            expected_reported=frozenset({("bill", "success"), ("pack", "success")}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_an_interrupt_just_after_a_record_when_running_then_it_still_ends_as_a_cancel(
    test_case: ItemPoolRecordedInterruptTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    barrier: threading.Barrier = threading.Barrier(len(test_case.items))
    probe: ItemProbe = ItemProbe(
        actions={name: partial(meet_at, barrier) for name in test_case.items}
    )
    graph: PythonNodeGraph = item_graph(
        tuple((name, "plan", len(test_case.items), ()) for name in test_case.items)
    )
    interrupt_after_next_record(monkeypatch)

    with pytest.raises(KeyboardInterrupt):
        _ = run_ingress(graph=graph, adapter=ItemPoolTestAdapter(), probe=probe)

    reported: frozenset[tuple[str, str]] = frozenset(
        (result.node_name, result.status.value) for result in probe.reported
    )
    assert reported == test_case.expected_reported


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolIgnoredCancelTestCase(
            description="one step ignores the cancel",
            expected_reported=frozenset({("quick", "success")}),
            expected_cancelled=("slow",),
            expected_left_open=1,
            expected_within_seconds=9,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_step_ignoring_cancel_when_interrupted_then_it_is_reported_cancelled_and_not_joined(
    test_case: ItemPoolIgnoredCancelTestCase,
) -> None:
    for run_path in _PATHS:
        release: threading.Event = threading.Event()
        probe: ItemProbe = ignoring_probe(release)
        adapter: ItemPoolTestAdapter = ItemPoolTestAdapter()
        graph: PythonNodeGraph = item_graph((("quick", "plan", 2, ()), ("slow", "plan", 2, ())))

        interrupted: float = time.monotonic()
        with pytest.raises(KeyboardInterrupt):
            _ = run_path(graph=graph, adapter=adapter, probe=probe)
        waited: float = time.monotonic() - interrupted
        release.set()

        reported: frozenset[tuple[str, str]] = frozenset(
            (result.node_name, result.status.value) for result in probe.reported
        )
        assert (reported, tuple(probe.cancelled)) == (
            test_case.expected_reported,
            test_case.expected_cancelled,
        )
        assert waited < test_case.expected_within_seconds
        assert adapter.connections - adapter.closed == test_case.expected_left_open


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolRepeatedCancelTestCase(
            description="the cancel's signal arrives while finished steps are reported",
            items=("bill", "pack", "pick"),
            expected_reported=frozenset(
                {("bill", "success"), ("pack", "success"), ("pick", "success")}
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_cancel_arriving_again_when_reporting_then_every_finished_step_is_still_reported(
    test_case: ItemPoolRepeatedCancelTestCase,
) -> None:
    for run_path in _PATHS:
        probe: ItemProbe = twice_cancelled_probe(test_case.items)
        graph: PythonNodeGraph = item_graph(
            tuple((name, "plan", len(test_case.items), ()) for name in test_case.items)
        )

        with pytest.raises(KeyboardInterrupt):
            _ = run_path(graph=graph, adapter=ItemPoolTestAdapter(), probe=probe)

        reported: frozenset[tuple[str, str]] = frozenset(
            (result.node_name, result.status.value) for result in probe.reported
        )
        assert (reported, probe.cancelled) == (test_case.expected_reported, [])


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolResultWriteTestCase(
            description="a finished step's result write spans a cancel round",
            write_seconds=2.5,
            expected_reported=frozenset({("quick", "success"), ("slow", "success")}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_step_writing_its_result_when_a_cancel_round_passes_then_the_write_is_not_cancelled(
    test_case: ItemPoolResultWriteTestCase, tmp_path: Path
) -> None:
    probe: ItemProbe = finishing_probe()
    graph: PythonNodeGraph = item_graph((("quick", "plan", 2, ()), ("slow", "plan", 2, ())))
    adapter: SlowResultAdapter = SlowResultAdapter(write_seconds=test_case.write_seconds)

    with pytest.raises(KeyboardInterrupt):
        run_persisting_read_side(
            graph=graph, adapter=adapter, probe=probe, database=tmp_path / "plan.duckdb"
        )

    reported: frozenset[tuple[str, str]] = frozenset(
        (result.node_name, result.status.value) for result in probe.reported
    )
    assert (reported, probe.cancelled) == (test_case.expected_reported, [])


@pytest.mark.parametrize(
    "test_case",
    [
        ItemPoolMidSubmitTestCase(
            description="the second step started before the interrupt left submit",
            start_second_thread=True,
            expected_cancelled=("pack", "pick"),
            expected_left_open=2,
        ),
        ItemPoolMidSubmitTestCase(
            description="the second step never started",
            start_second_thread=False,
            expected_cancelled=("pick", "pack"),
            expected_left_open=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_interrupt_inside_a_submit_when_cancelling_then_the_step_is_tracked_and_reported(
    test_case: ItemPoolMidSubmitTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    for run_path in _PATHS:
        release: threading.Event = threading.Event()
        started: threading.Event = threading.Event()
        probe: ItemProbe = ItemProbe(
            actions={
                "pack": partial(held_until, release, None),
                "pick": partial(held_until, release, started),
            }
        )
        adapter: ItemPoolTestAdapter = ItemPoolTestAdapter()
        graph: PythonNodeGraph = item_graph((("pack", "plan", 2, ()), ("pick", "plan", 2, ())))
        interrupt_second_submit(
            monkeypatch, start_thread=test_case.start_second_thread, started=started
        )

        began: float = time.monotonic()
        with pytest.raises(KeyboardInterrupt):
            _ = run_path(graph=graph, adapter=adapter, probe=probe)
        waited: float = time.monotonic() - began
        release.set()

        assert (tuple(probe.cancelled), probe.reported) == (test_case.expected_cancelled, [])
        assert waited < 9
        assert adapter.connections - adapter.closed == test_case.expected_left_open
