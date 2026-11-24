"""Tests for standalone source loader pipeline execution."""

from __future__ import annotations

from contextvars import Token
from io import StringIO
from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredLoaderFunction,
    LoadProgressReporter,
    NativeProgressProjector,
    SourceEntry,
)
from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.executor.load.main.run import run_load_pipeline
from sqlbuild.executor.load.models import (
    LoadCallbacks,
    LoaderContext,
    LoadExecutionResult,
    LoadRuntimeParams,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from tests.unit.src.sqlbuild.executor.load._test_types import (
    ConcurrentLoadProgressTestCase,
    LoadPipelineSkipFanInTestCase,
    LoadStartOrderingTestCase,
)
from tests.unit.src.sqlbuild.executor.load.helpers import CountingLoaderContextTestAdapter


@pytest.mark.parametrize(
    "test_case",
    (
        LoadStartOrderingTestCase(
            description="sequential load start callback follows canonical start",
            expected_connection_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_standalone_load_mode_when_loader_enters_then_start_order_wraps_blocking_work(
    test_case: LoadStartOrderingTestCase,
) -> None:
    adapter: CountingLoaderContextTestAdapter = CountingLoaderContextTestAdapter()
    order: list[str] = []
    dispatcher: EventDispatcher = EventDispatcher()

    def record_event(event: LifecycleEvent) -> None:
        order.append(event.event_type)

    def loader(_ctx: LoaderContext) -> None:
        assert order.index("resource_attempt_started") < order.index("callback_start")
        order.append("blocking_work")

    dispatcher.subscribe_lifecycle(subscriber=record_event, accepts_opaque=False)
    with invocation_scope("load-order-invocation"), dispatcher_scope(dispatcher):
        results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
            sources=(SourceEntry(name="ordered", loader="ordered_loader"),),
            loader_functions=(
                DiscoveredLoaderFunction(
                    file_path=Path("python/nodes/loaders/ordered.py"),
                    relative_path=Path("python/nodes/loaders/ordered.py"),
                    name="ordered_loader",
                    function=loader,
                ),
            ),
            connection_config={},
            adapter=adapter,
            runtime=LoadRuntimeParams(
                run_id="load-order-run",
                target=None,
                vars={},
                is_reload=False,
            ),
            callbacks=LoadCallbacks(
                on_load_start=lambda _source: order.append("callback_start"),
                on_load_complete=lambda _result: order.append("callback_complete"),
            ),
        )

    assert results[0].status == ExecutionStatus.SUCCESS
    assert adapter.connection_count == test_case.expected_connection_count
    assert order.index("resource_attempt_started") < order.index("callback_start")
    assert order.index("callback_start") < order.index("blocking_work")
    assert order.index("blocking_work") < order.index("resource_attempt_completed")
    assert order.index("resource_attempt_completed") < order.index("callback_complete")


@pytest.mark.parametrize(
    "test_case",
    (
        LoadPipelineSkipFanInTestCase(
            description="runs downstream when soft-skipped loader branch has successful sibling",
            skip_mode=SkipMode.SOFT,
            expected_statuses=(
                ExecutionStatus.SKIPPED,
                ExecutionStatus.SUCCESS,
                ExecutionStatus.SKIPPED,
                ExecutionStatus.SUCCESS,
            ),
            expected_skip_modes=("soft", None, "soft", None),
            expected_skip_reasons=(
                "no new orders",
                None,
                "All upstream loaders were soft-skipped",
                None,
            ),
        ),
        LoadPipelineSkipFanInTestCase(
            description="skips downstream when hard-skipped loader branch has successful sibling",
            skip_mode=SkipMode.HARD,
            expected_statuses=(
                ExecutionStatus.SKIPPED,
                ExecutionStatus.SUCCESS,
                ExecutionStatus.SKIPPED,
                ExecutionStatus.SKIPPED,
            ),
            expected_skip_modes=("hard", None, "hard", "hard"),
            expected_skip_reasons=(
                "no new orders",
                None,
                "Upstream loader hard-skipped",
                "Upstream loader hard-skipped",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_mixed_loader_skips_when_running_pipeline_then_fan_in_matches_mode(
    test_case: LoadPipelineSkipFanInTestCase,
) -> None:
    adapter: CountingLoaderContextTestAdapter = CountingLoaderContextTestAdapter()

    def a_loader(ctx: LoaderContext) -> object:
        return ctx.skip(reason="no new orders", mode=test_case.skip_mode)

    def x_loader(_ctx: LoaderContext) -> None:
        return None

    def b_loader(_ctx: LoaderContext) -> None:
        return None

    def c_loader(_ctx: LoaderContext) -> None:
        return None

    results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=(
            SourceEntry(name="a", loader="a_loader"),
            SourceEntry(name="x", loader="x_loader"),
            SourceEntry(name="b", loader="b_loader"),
            SourceEntry(name="c", loader="c_loader"),
        ),
        loader_functions=(
            DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/a.py"),
                relative_path=Path("python/nodes/loaders/a.py"),
                name="a_loader",
                function=a_loader,
            ),
            DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/x.py"),
                relative_path=Path("python/nodes/loaders/x.py"),
                name="x_loader",
                function=x_loader,
            ),
            DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/b.py"),
                relative_path=Path("python/nodes/loaders/b.py"),
                name="b_loader",
                function=b_loader,
                depends_on=(a_loader,),
            ),
            DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/c.py"),
                relative_path=Path("python/nodes/loaders/c.py"),
                name="c_loader",
                function=c_loader,
                depends_on=(b_loader, x_loader),
            ),
        ),
        connection_config={},
        adapter=adapter,
        runtime=LoadRuntimeParams(
            run_id="run-1",
            target=None,
            vars={},
            is_reload=False,
        ),
    )

    assert tuple(result.source_name for result in results) == ("a", "x", "b", "c")
    assert tuple(result.status for result in results) == test_case.expected_statuses
    assert (
        tuple(getattr(result.skip_mode, "value", None) for result in results)
        == test_case.expected_skip_modes
    )
    assert tuple(result.skip_reason for result in results) == test_case.expected_skip_reasons


@pytest.mark.parametrize(
    "test_case",
    (
        ConcurrentLoadProgressTestCase(
            description="two concurrent loaders retain canonical and rich progress contexts",
            expected_start_count=2,
            expected_terminal_count=2,
            expected_rich_row_count=2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_two_independent_loaders_when_run_concurrently_then_each_projects_once(
    test_case: ConcurrentLoadProgressTestCase,
) -> None:
    adapter: CountingLoaderContextTestAdapter = CountingLoaderContextTestAdapter()
    sources: tuple[SourceEntry, ...] = (
        SourceEntry(name="first", loader="first_loader"),
        SourceEntry(name="second", loader="second_loader"),
    )
    stream: StringIO = StringIO()
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    projector_token: Token[NativeProgressProjector | None] = projector.install()
    reporter: LoadProgressReporter = LoadProgressReporter(
        stream=stream,
        use_color=False,
        source_order={"first": 1, "second": 2},
        total_count=2,
    )
    dispatcher: EventDispatcher = EventDispatcher()
    events: list[LifecycleEvent] = []
    dispatcher.subscribe_lifecycle(subscriber=projector.consume, accepts_opaque=False)
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    def first_loader(_ctx: LoaderContext) -> None:
        assert "first START" in stream.getvalue()
        return None

    def second_loader(_ctx: LoaderContext) -> None:
        assert "second START" in stream.getvalue()
        return None

    try:
        with invocation_scope("load-invocation"), dispatcher_scope(dispatcher):
            results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
                sources=sources,
                loader_functions=(
                    DiscoveredLoaderFunction(
                        file_path=Path("python/nodes/loaders/first.py"),
                        relative_path=Path("python/nodes/loaders/first.py"),
                        name="first_loader",
                        function=first_loader,
                    ),
                    DiscoveredLoaderFunction(
                        file_path=Path("python/nodes/loaders/second.py"),
                        relative_path=Path("python/nodes/loaders/second.py"),
                        name="second_loader",
                        function=second_loader,
                    ),
                ),
                connection_config={},
                adapter=adapter,
                runtime=LoadRuntimeParams(
                    run_id="concurrent-run",
                    target=None,
                    vars={},
                    is_reload=False,
                ),
                callbacks=LoadCallbacks(
                    on_load_start=reporter.on_start,
                    on_load_complete=reporter.on_complete,
                ),
                max_concurrency=2,
            )
    finally:
        projector.restore(projector_token)

    resource_events: tuple[LifecycleEvent, ...] = tuple(
        filter(lambda event: event.event_type.startswith("resource_attempt_"), events)
    )
    output_lines: tuple[str, ...] = tuple(stream.getvalue().splitlines())
    assert len(results) == 2
    assert sum(event.event_type.endswith("started") for event in resource_events) == (
        test_case.expected_start_count
    )
    assert (
        sum(event.event_type.endswith(("completed", "failed")) for event in resource_events)
        == test_case.expected_terminal_count
    )
    assert sum("rows=0" in line and "OK" in line for line in output_lines) == (
        test_case.expected_rich_row_count
    ), stream.getvalue()
