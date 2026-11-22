"""Discovered items with `max_parallel` run side by side on both serial paths."""

from __future__ import annotations

import threading
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
    ItemPoolOrderTestCase,
    ItemPoolSerialTestCase,
    ItemPoolWriteTestCase,
)
from tests.unit.src.sqlbuild.executor.python_nodes.classes.helpers import (
    WAIT_SECONDS,
    ItemPoolTestAdapter,
    ItemProbe,
    fail_item,
    item_graph,
    meet_at,
    result_summary,
    run_ingress,
    run_read_side,
    run_writing_ingress,
    run_writing_read_side,
    writing_graph,
)

_PATHS: tuple[Callable[..., tuple[PythonNodeExecutionResult, ...]], ...] = (
    run_read_side,
    run_ingress,
)


@pytest.mark.parametrize("run_path", _PATHS, ids=("read side", "ingress"))
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
            items=tuple(
                (f"{asset}[{index}]", "plan", 2)
                for asset in ("bill", "fulfil")
                for index in range(3)
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
    run_path: Callable[..., tuple[PythonNodeExecutionResult, ...]],
) -> None:
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


@pytest.mark.parametrize("run_path", _PATHS, ids=("read side", "ingress"))
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
    run_path: Callable[..., tuple[PythonNodeExecutionResult, ...]],
) -> None:
    barrier: threading.Barrier = threading.Barrier(len(test_case.meeting))
    probe: ItemProbe = ItemProbe(
        actions={name: partial(meet_at, barrier) for name in test_case.meeting}
    )
    graph: PythonNodeGraph = item_graph(
        tuple((name, f"set-{limit}", limit, parents) for name, limit, parents in test_case.items)
    )

    _ = run_path(graph=graph, adapter=ItemPoolTestAdapter(), probe=probe)

    assert probe.seen_before == test_case.expected_seen_before


@pytest.mark.parametrize("run_path", _PATHS, ids=("read side", "ingress"))
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
    run_path: Callable[..., tuple[PythonNodeExecutionResult, ...]],
) -> None:
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


@pytest.mark.parametrize("run_path", _PATHS, ids=("read side", "ingress"))
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
    run_path: Callable[..., tuple[PythonNodeExecutionResult, ...]],
) -> None:
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
    serial, parallel = (
        result_summary(
            run_path(
                graph=item_graph(tuple((name, "plan", limit, up) for name, up in shape)),
                adapter=ItemPoolTestAdapter(),
                probe=ItemProbe(actions={"a": outcomes[test_case.outcome]}),
            )
        )
        for limit in (None, 3)
    )

    assert parallel == serial
    assert {row[1] for row in parallel if row[0] == "a"} == {test_case.expected_first_status}


@pytest.mark.parametrize("run_path", _PATHS, ids=("read side", "ingress"))
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
    run_path: Callable[..., tuple[PythonNodeExecutionResult, ...]],
) -> None:
    both_running: threading.Barrier = threading.Barrier(2)
    interrupted: threading.Event = threading.Event()

    def slow() -> object:
        _ = both_running.wait(timeout=WAIT_SECONDS)
        assert interrupted.wait(timeout=WAIT_SECONDS)
        return fail_item()

    def interrupt(name: str) -> None:
        probe.after_report = lambda reported: None
        interrupted.set()
        raise KeyboardInterrupt(name)

    probe: ItemProbe = ItemProbe(actions={"quick": partial(meet_at, both_running), "slow": slow})
    probe.after_report = interrupt
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
    "run_path", (run_writing_read_side, run_writing_ingress), ids=("read side", "ingress")
)
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
    run_path: Callable[..., tuple[PythonNodeExecutionResult, ...]],
    tmp_path: Path,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    config: dict[str, object] = {"database": str(tmp_path / "warehouse.duckdb")}
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
    table: str = adapter.query(
        connection=connection,
        sql="SELECT table_name FROM information_schema.columns "
        "WHERE table_schema = 'plan_state' AND column_name = 'payload_json_b64'",
        limit=None,
    ).rows[0][0]
    written: set[object] = {
        row[0]
        for row in adapter.query(
            connection=connection, sql=f"SELECT node_name FROM plan_state.{table}", limit=None
        ).rows
    }
    adapter.close(connection)
    assert written == set(test_case.steps)
