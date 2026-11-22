"""Discovered items that meet at barriers, run on either serial path."""

from __future__ import annotations

import threading
from collections.abc import Callable
from pathlib import Path
from typing import Any, ClassVar, cast

import pytest

from sqlbuild._native import (
    CommandTerminated,
    DiscoveredAssetFunction,
    DiscoveredProjectInputs,
    LocalConfig,
    ProjectConfig,
    PythonNodeGraph,
    build_python_node_graph,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.assets import AssetContext
from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.executor.python_nodes._helpers.ingress_execution import (
    execute_ingress_python_loader_nodes,
)
from sqlbuild.executor.python_nodes.classes.read_side_python_execution_tracker import (
    ReadSidePythonExecutionTracker,
)
from sqlbuild.executor.python_nodes.models import (
    IngressCallbacks,
    PythonNodeExecutionResult,
    PythonNodeRuntime,
)
from sqlbuild.executor.scheduling.classes.lifecycle_progress import LifecycleProgress
from sqlbuild.python_nodes.models import DiscoveredItemRef

WAIT_SECONDS: float = 30


class ItemPoolTestAdapter(BaseAdapter):
    """Counts the connections a run opens."""

    adapter_name: ClassVar[str] = "item-pool-test"

    def __init__(self, *, adapter_name: str = "item-pool-test") -> None:
        self.__dict__["adapter_name"] = adapter_name
        self.connections: int = 0
        self.closed: int = 0

    def connect(self, config: dict[str, object]) -> object:
        del config
        self.connections += 1
        return object()

    def close(self, connection: object) -> None:
        del connection
        self.closed += 1

    def _execute(self, connection: Any, sql: str) -> object:
        del connection
        return sql


class ItemProbe:
    """What the items saw: who ran at once, on which threads, after whose reports."""

    def __init__(self, *, actions: dict[str, Callable[[], object]] | None = None) -> None:
        self.actions: dict[str, Callable[[], object]] = actions or {}
        self.reported: list[PythonNodeExecutionResult] = []
        self.report_threads: set[int] = set()
        self.started: list[str] = []
        self.start_threads: set[int] = set()
        self.started_before: dict[str, frozenset[str]] = {}
        self.cancelled: list[str] = []
        self.seen_before: dict[str, frozenset[str]] = {}
        self.threads: set[int] = set()
        self.most_at_once: int = 0
        self.after_report: Callable[[str], None] = lambda name: None
        self._active: int = 0
        self._lock: threading.Lock = threading.Lock()

    def step(self, name: str) -> object:
        with self._lock:
            self._active += 1
            self.most_at_once = max(self.most_at_once, self._active)
            self.threads.add(threading.get_ident())
            self.started_before[name] = frozenset(self.started)
            self.seen_before[name] = frozenset(result.node_name for result in self.reported)
        try:
            return self.actions.get(name, lambda: None)()
        finally:
            with self._lock:
                self._active -= 1

    def start(self, name: str) -> None:
        self.start_threads.add(threading.get_ident())
        self.started.append(name)

    def report(self, result: object) -> None:
        reported: PythonNodeExecutionResult = cast(PythonNodeExecutionResult, result)
        self.report_threads.add(threading.get_ident())
        self.reported.append(reported)
        self.after_report(reported.node_name)


def run_item(ctx: AssetContext) -> object:
    """Run one item as its probe says."""

    discovered: Any = cast(Any, ctx.discovered)
    probe: ItemProbe = cast(ItemProbe, ctx.vars["probe"])
    outcome: object = probe.step(str(discovered.item))
    skips: dict[object, Callable[[], object]] = {
        "soft": lambda: ctx.skip(reason="nothing to do", mode=SkipMode.SOFT),
        "hard": lambda: ctx.skip(reason="cannot run", mode=SkipMode.HARD),
    }
    return skips.get(outcome, lambda: None)()


def fail_item() -> object:
    """An item that raises."""

    raise ValueError("the step failed")


def meet_at(barrier: threading.Barrier) -> object:
    """An item that returns only once `barrier`'s other parties have arrived."""

    _ = barrier.wait(timeout=WAIT_SECONDS)
    return None


def item_refs(parents: tuple[str, ...]) -> Any:
    """An item's dependencies on the items named `parents`."""

    return tuple(map(_item_ref, parents))


def _item_ref(name: str) -> DiscoveredItemRef:
    return DiscoveredItemRef(name=name)


def item_graph(
    items: tuple[tuple[str, str, int | None, tuple[str, ...]], ...],
) -> PythonNodeGraph:
    """Nodes named by item, each `(name, set, max_parallel, parents)`."""

    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            asset_functions=tuple(
                DiscoveredAssetFunction(
                    file_path=Path("/project/python/plan.py"),
                    relative_path=Path("python/plan.py"),
                    name=name,
                    function=run_item,
                    depends_on=item_refs(parents),
                    discovered={
                        "asset": "fulfil",
                        "set": set_name,
                        "item": name,
                        "args": {},
                        "tags": {},
                        "max_parallel": max_parallel,
                        "placeholder": False,
                        "error": None,
                    },
                )
                for name, set_name, max_parallel, parents in items
            ),
        )
    )


def ignore_identity(*, identity: object, _target_name: str | None) -> None:
    """Record no identity."""

    del identity, _target_name


def item_runtime(*, adapter: ItemPoolTestAdapter, probe: ItemProbe) -> PythonNodeRuntime:
    """A runtime that persists nothing."""

    return PythonNodeRuntime(
        adapter=adapter,
        connection_config={},
        connection=object(),
        run_id="test_run",
        target="dev",
        vars={"probe": probe},
        is_reload=False,
        persist_node_results=False,
    )


def run_read_side(
    *, graph: PythonNodeGraph, adapter: ItemPoolTestAdapter, probe: ItemProbe
) -> tuple[PythonNodeExecutionResult, ...]:
    """Every node through the read-side tracker."""

    tracker: ReadSidePythonExecutionTracker = ReadSidePythonExecutionTracker(
        python_graph=graph,
        selected_python_names=frozenset(graph.nodes_by_name),
        scheduled_sql_names=frozenset(),
        runtime=item_runtime(adapter=adapter, probe=probe),
        identity_recorder=ignore_identity,
        on_node_complete=probe.report,
        on_asset_start=probe.start,
        on_node_cancelled=probe.cancelled.append,
    )
    tracker.dispatch_ready_python_nodes()
    return tracker.results


def run_ingress(
    *, graph: PythonNodeGraph, adapter: ItemPoolTestAdapter, probe: ItemProbe
) -> tuple[PythonNodeExecutionResult, ...]:
    """Every node through the ingress lifecycle scheduler."""

    return execute_ingress_python_loader_nodes(
        python_graph=graph,
        selected_python_names=frozenset(graph.nodes_by_name),
        loader_functions=(),
        source_map={},
        runtime=item_runtime(adapter=adapter, probe=probe),
        callbacks=IngressCallbacks(
            on_node_complete=probe.report,
            on_asset_start=probe.start,
            on_node_cancelled=probe.cancelled.append,
            identity_recorder=ignore_identity,
        ),
    ).python_results


def result_summary(
    results: tuple[PythonNodeExecutionResult, ...],
) -> frozenset[tuple[str, str, str | None, str | None]]:
    """Each node's status and why, whatever order they finished in."""

    return frozenset(
        (result.node_name, result.status.value, result.error_message, result.skip_reason)
        for result in results
    )


def write_step(ctx: AssetContext) -> object:
    """Write a table and a result while every other step does."""

    discovered: Any = cast(Any, ctx.discovered)
    step: str = str(discovered.item)
    barrier: threading.Barrier = cast(threading.Barrier, ctx.vars["barrier"])
    _ = barrier.wait(timeout=WAIT_SECONDS)
    ctx.execute_sql(f"CREATE OR REPLACE TABLE main.step_{step} AS SELECT 1 AS written")
    _ = barrier.wait(timeout=WAIT_SECONDS)
    return ctx.result(payload={"step": step})


def writing_graph(steps: tuple[str, ...]) -> PythonNodeGraph:
    """Every step at once on one set."""

    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            asset_functions=tuple(
                DiscoveredAssetFunction(
                    file_path=Path("/project/python/plan.py"),
                    relative_path=Path("python/plan.py"),
                    name=step,
                    function=write_step,
                    discovered={
                        "asset": "fulfil",
                        "set": "plan",
                        "item": step,
                        "args": {},
                        "tags": {},
                        "max_parallel": len(steps),
                        "placeholder": False,
                        "error": None,
                    },
                )
                for step in steps
            ),
        )
    )


def run_writing_read_side(
    *, graph: PythonNodeGraph, runtime: PythonNodeRuntime
) -> tuple[PythonNodeExecutionResult, ...]:
    """Every writing step through the read-side tracker."""

    tracker: ReadSidePythonExecutionTracker = ReadSidePythonExecutionTracker(
        python_graph=graph,
        selected_python_names=frozenset(graph.nodes_by_name),
        scheduled_sql_names=frozenset(),
        runtime=runtime,
    )
    tracker.dispatch_ready_python_nodes()
    return tracker.results


def run_writing_ingress(
    *, graph: PythonNodeGraph, runtime: PythonNodeRuntime
) -> tuple[PythonNodeExecutionResult, ...]:
    """Every writing step through the ingress lifecycle scheduler."""

    return execute_ingress_python_loader_nodes(
        python_graph=graph,
        selected_python_names=frozenset(graph.nodes_by_name),
        loader_functions=(),
        source_map={},
        runtime=runtime,
    ).python_results


def run_shape(
    *,
    run_path: Callable[..., tuple[PythonNodeExecutionResult, ...]],
    shape: tuple[tuple[str, tuple[str, ...]], ...],
    limit: int | None,
    action: Callable[[], object],
) -> frozenset[tuple[str, str, str | None, str | None]]:
    """How `shape`'s nodes end at `limit` when its first node takes `action`."""

    return result_summary(
        run_path(
            graph=item_graph(tuple((name, "plan", limit, up) for name, up in shape)),
            adapter=ItemPoolTestAdapter(),
            probe=ItemProbe(actions={"a": action}),
        )
    )


def interrupted_probe() -> ItemProbe:
    """`quick` finishes and its report raises an interrupt while `slow` is still running."""

    both_running: threading.Barrier = threading.Barrier(2)
    interrupted: threading.Event = threading.Event()

    def quick() -> object:
        return meet_at(both_running)

    def slow() -> object:
        _ = both_running.wait(timeout=WAIT_SECONDS)
        assert interrupted.wait(timeout=WAIT_SECONDS)
        return fail_item()

    probe: ItemProbe = ItemProbe(actions={"quick": quick, "slow": slow})

    def interrupt(name: str) -> None:
        probe.after_report = lambda reported: None
        interrupted.set()
        raise KeyboardInterrupt(name)

    probe.after_report = interrupt
    return probe


def interrupt_after_next_record(monkeypatch: pytest.MonkeyPatch) -> None:
    """Raise an interrupt once the next ingress result is recorded, as a signal can."""

    record: Callable[..., None] = LifecycleProgress.complete

    def record_then_interrupt(progress: LifecycleProgress, **recorded: Any) -> None:
        monkeypatch.setattr(LifecycleProgress, "complete", record)
        record(progress, **recorded)
        raise KeyboardInterrupt

    monkeypatch.setattr(LifecycleProgress, "complete", record_then_interrupt)


def ignoring_probe(release: threading.Event) -> ItemProbe:
    """`quick` finishes and its report raises an interrupt; `slow` ignores it until released."""

    both_running: threading.Barrier = threading.Barrier(2)

    def quick() -> object:
        return meet_at(both_running)

    def slow() -> object:
        _ = both_running.wait(timeout=WAIT_SECONDS)
        _ = release.wait(timeout=WAIT_SECONDS)
        return None

    probe: ItemProbe = ItemProbe(actions={"quick": quick, "slow": slow})

    def interrupt(name: str) -> None:
        probe.after_report = lambda reported: None
        raise KeyboardInterrupt(name)

    probe.after_report = interrupt
    return probe


def twice_cancelled_probe(items: tuple[str, ...]) -> ItemProbe:
    """Items finish together; the cancel lands at the first report and its signal at the next."""

    barrier: threading.Barrier = threading.Barrier(len(items))
    probe: ItemProbe = ItemProbe(actions={name: lambda: meet_at(barrier) for name in items})

    def signalled(name: str) -> None:
        probe.after_report = lambda reported: None
        raise CommandTerminated(name)

    def cancelled(name: str) -> None:
        probe.after_report = signalled
        raise KeyboardInterrupt(name)

    probe.after_report = cancelled
    return probe


class SlowResultAdapter(DuckDbAdapter):
    """Writes `slow`'s result slowly, and fails that write when it is cancelled."""

    def __init__(self, *, write_seconds: float) -> None:
        super().__init__()
        self.write_seconds: float = write_seconds
        self.writing: threading.Event = threading.Event()
        self.killed: threading.Event = threading.Event()

    def _execute(self, connection: Any, sql: str) -> object:
        writes: dict[bool, Callable[[], None]] = {True: self._write_slowly, False: lambda: None}
        writes["INSERT" in sql and "node_results" in sql and "'slow'" in sql]()
        return super()._execute(connection=connection, sql=sql)

    def _write_slowly(self) -> None:
        self.writing.set()
        killed: bool = self.killed.wait(timeout=self.write_seconds)
        self.writing.clear()
        outcomes: dict[bool, Callable[[], object]] = {True: fail_item, False: lambda: None}
        _ = outcomes[killed]()

    def interrupt_connection(self, connection: Any) -> bool:
        del connection
        marks: dict[bool, Callable[[], None]] = {True: self.killed.set, False: lambda: None}
        marks[self.writing.is_set()]()
        return True


def run_persisting_read_side(
    *, graph: PythonNodeGraph, adapter: DuckDbAdapter, probe: ItemProbe, database: Path
) -> None:
    """Every node through the read-side tracker, each result written to `database`."""

    config: dict[str, object] = {"database": str(database)}
    ReadSidePythonExecutionTracker(
        python_graph=graph,
        selected_python_names=frozenset(graph.nodes_by_name),
        scheduled_sql_names=frozenset(),
        runtime=PythonNodeRuntime(
            adapter=adapter,
            connection_config=config,
            connection=adapter.connect(config),
            run_id="test_run",
            target="dev",
            vars={"probe": probe},
            is_reload=False,
            default_schema="main",
        ),
        identity_recorder=ignore_identity,
        on_node_complete=probe.report,
        on_node_cancelled=probe.cancelled.append,
    ).dispatch_ready_python_nodes()


def finishing_probe() -> ItemProbe:
    """`quick` finishes and its report raises an interrupt; `slow` finishes only after it."""

    both_running: threading.Barrier = threading.Barrier(2)
    interrupted: threading.Event = threading.Event()

    def quick() -> object:
        return meet_at(both_running)

    def slow() -> object:
        _ = both_running.wait(timeout=WAIT_SECONDS)
        _ = interrupted.wait(timeout=WAIT_SECONDS)
        return None

    probe: ItemProbe = ItemProbe(actions={"quick": quick, "slow": slow})

    def interrupt(name: str) -> None:
        probe.after_report = lambda reported: None
        interrupted.set()
        raise KeyboardInterrupt(name)

    probe.after_report = interrupt
    return probe
