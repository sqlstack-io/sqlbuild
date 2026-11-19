from __future__ import annotations

import time
from dataclasses import replace
from io import StringIO
from threading import Event, Thread
from typing import cast

import pytest

from sqlbuild._native import NativeProgressProjector, shared_transient_line_coordinator
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    OperationLifecycle,
    ResourceAttemptLifecycle,
    dispatcher_scope,
    invocation_scope,
    run_scope,
)
from sqlbuild.runtime.observability.classes.operation_lifecycle import publish_retry_scheduled
from tests.unit.src.sqlbuild.cli.progress.classes._test_types import (
    CursorCleanupCase,
    FreshnessMetadataProjectionCase,
    LockOrderCase,
    NativeProjectionCase,
    RetryProjectionCase,
    StartFlushCase,
)
from tests.unit.src.sqlbuild.runtime.observability.helpers import lifecycle_event


class _FlushRecordingStream(StringIO):
    def __init__(self, *, tty: bool = False) -> None:
        super().__init__()
        self.flush_count: int = 0
        self._tty: bool = tty

    def flush(self) -> None:
        self.flush_count += 1
        super().flush()

    def isatty(self) -> bool:
        return self._tty


@pytest.mark.parametrize(
    "test_case",
    (
        StartFlushCase(
            description="logical skip renders one terminal",
            expected_event_types=("resource_attempt_started", "resource_attempt_skipped"),
            expected_flush_count_before_block=2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_skipped_resource_when_projected_then_skip_terminal_renders_once(
    test_case: StartFlushCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream()
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    dispatcher.subscribe_lifecycle(subscriber=projector.consume, accepts_opaque=False)

    with (
        invocation_scope("inv-skip-projection"),
        run_scope("run-skip-projection"),
        dispatcher_scope(dispatcher),
        ResourceAttemptLifecycle(
            resource_id="asset:skipped",
            resource_kind="task",
            resource_name="skipped",
        ) as lifecycle,
    ):
        lifecycle.skipped(skip_code="scheduler")

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert stream.flush_count == test_case.expected_flush_count_before_block
    lines: tuple[str, ...] = tuple(stream.getvalue().splitlines())
    assert lines[0] == "  task      skipped START"
    assert lines[1].startswith("  task      skipped SKIP 0.")


@pytest.mark.parametrize(
    "test_case",
    (
        RetryProjectionCase(
            description="retry attempts flush before each blocking boundary",
            expected_lines=(
                "  task      flaky START",
                "    Python task attempt 1  START",
                "    Python task attempt 1  FAIL  (0.00s)",
                "    retry 1->2 in 0.25s",
                "    Python task attempt 2  START",
                "    Python task attempt 2  OK  (0.00s)",
                "  task      flaky OK 0.00s",
            ),
            expected_unrelated_duration_ms=0.0,
            expected_final_duration_ms=0.0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_retrying_child_operation_when_projected_then_each_line_flushes_before_next_block(
    test_case: RetryProjectionCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream()
    projector: NativeProgressProjector = NativeProgressProjector(
        stream=stream, use_color=False, debug=True
    )
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=projector.consume, accepts_opaque=False)

    with (
        invocation_scope("inv-retry-projection"),
        run_scope("run-retry-projection"),
        dispatcher_scope(dispatcher),
        ResourceAttemptLifecycle(
            resource_id="asset:flaky",
            resource_kind="task",
            resource_name="flaky",
        ),
    ):
        with OperationLifecycle(
            operation_kind="python_node",
            operation_name="python_task",
            metadata={"attempt_number": 1},
        ) as first_attempt:
            assert stream.getvalue().endswith("Python task attempt 1  START\n")
            first_attempt.failed(error=TimeoutError())
        publish_retry_scheduled(
            failed_attempt_number=1,
            next_attempt_number=2,
            delay_ms=250,
            error=TimeoutError(),
        )
        assert stream.getvalue().endswith("    retry 1->2 in 0.25s\n")
        with OperationLifecycle(
            operation_kind="python_node",
            operation_name="python_task",
            metadata={"attempt_number": 2},
        ):
            assert stream.getvalue().endswith("Python task attempt 2  START\n")

    assert stream.flush_count == len(stream.getvalue().splitlines())
    rendered_lines: tuple[str, ...] = tuple(stream.getvalue().splitlines())
    assert rendered_lines[0:2] == test_case.expected_lines[0:2]
    assert rendered_lines[2].startswith("    Python task attempt 1  FAIL  (")
    assert rendered_lines[3:5] == test_case.expected_lines[3:5]
    assert rendered_lines[5].startswith("    Python task attempt 2  OK  (")
    assert rendered_lines[-1].startswith("  task      flaky OK 0.")


@pytest.mark.parametrize(
    "test_case",
    (
        NativeProjectionCase(
            description="interleaved resource attempts and replayed facts",
            expected_lines=(
                "  2/2  loader    second START",
                "  1/2  table     first START",
                "Project compile  START",
                "Project compile  OK  (0.01s)",
            ),
            expected_first_duration_ms=1250.0,
            expected_second_duration_ms=2500.0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_interleaved_and_duplicate_events_when_projected_then_ids_remain_independent(
    test_case: NativeProjectionCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream()
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    projector.configure_resources(ordinals={"first": 1, "second": 2}, total=2)
    second_start: LifecycleEvent = replace(
        lifecycle_event(
            "resource_attempt_started",
            run_id="run",
            resource_id="source:second",
            resource_attempt_id="attempt-second",
            payload={
                "resource_kind": "loader",
                "resource_name": "second",
                "attempt_number": 1,
            },
        ),
        event_id="second-start",
    )
    first_start: LifecycleEvent = replace(
        second_start,
        event_id="first-start",
        resource_id="model:first",
        resource_attempt_id="attempt-first",
        payload={
            "resource_kind": "table",
            "resource_name": "first",
            "attempt_number": 1,
        },
    )
    first_terminal: LifecycleEvent = replace(
        first_start,
        event_id="first-terminal",
        event_type="resource_attempt_completed",
        payload={**first_start.payload, "duration_ms": test_case.expected_first_duration_ms},
    )
    second_terminal: LifecycleEvent = replace(
        second_start,
        event_id="second-terminal",
        event_type="resource_attempt_completed",
        payload={**second_start.payload, "duration_ms": test_case.expected_second_duration_ms},
    )
    operation_start: LifecycleEvent = replace(
        lifecycle_event(
            "operation_started",
            operation_id="operation",
            payload={"operation_kind": "project", "operation_name": "project_compile"},
        ),
        event_id="operation-start",
    )
    operation_terminal: LifecycleEvent = replace(
        operation_start,
        event_id="operation-terminal",
        event_type="operation_completed",
        payload={**operation_start.payload, "duration_ms": 10.0},
    )
    statement: LifecycleEvent = replace(
        lifecycle_event(
            "statement_started",
            statement_id="statement",
            payload={"adapter": "duckdb", "statement_kind": "select"},
        ),
        event_id="statement-start",
    )

    for event in (
        second_start,
        first_start,
        first_start,
        first_terminal,
        second_terminal,
        statement,
        operation_start,
        operation_terminal,
    ):
        projector.consume(event)

    assert tuple(stream.getvalue().splitlines()) == test_case.expected_lines
    assert (
        projector.consume_resource_terminal(resource_name="first")
        == test_case.expected_first_duration_ms
    )
    assert (
        projector.consume_resource_terminal(resource_name="second")
        == test_case.expected_second_duration_ms
    )
    assert projector.consume_resource_terminal(resource_name="first") is None


@pytest.mark.parametrize(
    "test_case",
    (
        RetryProjectionCase(
            description="failed retry followed by successful final attempt",
            expected_lines=(
                "  1/2  task      orders START",
                "  2/2  task      inventory START",
                "  task      orders FAIL 0.10s",
                "  1/2  task      orders START",
                "  task      inventory OK 0.30s  rich",
                "  task      orders OK 0.25s  rich",
            ),
            expected_unrelated_duration_ms=300.0,
            expected_final_duration_ms=250.0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_retried_resource_when_final_result_claims_then_prior_attempt_renders_first(
    test_case: RetryProjectionCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream()
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    projector.configure_resources(ordinals={"orders": 1, "inventory": 2}, total=2)
    first_start: LifecycleEvent = replace(
        lifecycle_event(
            "resource_attempt_started",
            run_id="run",
            resource_id="asset:orders",
            resource_attempt_id="orders-attempt-1",
            payload={
                "resource_kind": "task",
                "resource_name": "orders",
                "attempt_number": 1,
            },
        ),
        event_id="orders-start-1",
    )
    first_failed: LifecycleEvent = replace(
        first_start,
        event_id="orders-failed-1",
        event_type="resource_attempt_failed",
        payload={**first_start.payload, "duration_ms": 100.0, "error_type": "TimeoutError"},
    )
    unrelated_start: LifecycleEvent = replace(
        first_start,
        event_id="inventory-start",
        resource_id="asset:inventory",
        resource_attempt_id="inventory-attempt",
        payload={
            "resource_kind": "task",
            "resource_name": "inventory",
            "attempt_number": 1,
        },
    )
    unrelated_terminal: LifecycleEvent = replace(
        unrelated_start,
        event_id="inventory-terminal",
        event_type="resource_attempt_completed",
        payload={
            **unrelated_start.payload,
            "duration_ms": test_case.expected_unrelated_duration_ms,
        },
    )
    final_start: LifecycleEvent = replace(
        first_start,
        event_id="orders-start-2",
        resource_attempt_id="orders-attempt-2",
        payload={**first_start.payload, "attempt_number": 2},
    )
    final_terminal: LifecycleEvent = replace(
        final_start,
        event_id="orders-terminal-2",
        event_type="resource_attempt_completed",
        payload={**final_start.payload, "duration_ms": test_case.expected_final_duration_ms},
    )

    for event in (first_start, unrelated_start, first_failed, final_start, unrelated_terminal):
        projector.consume(event)
    unrelated_duration: float | None = projector.consume_resource_terminal(
        resource_name="inventory"
    )
    stream.write(
        f"  task      inventory OK {cast(float, unrelated_duration) / 1000.0:.2f}s  rich\n"
    )
    projector.consume(final_terminal)
    final_duration: float | None = projector.consume_resource_terminal(resource_name="orders")
    stream.write(f"  task      orders OK {cast(float, final_duration) / 1000.0:.2f}s  rich\n")
    projector.consume(first_failed)
    projector.consume(final_terminal)
    projector.close()

    assert unrelated_duration == test_case.expected_unrelated_duration_ms
    assert final_duration == test_case.expected_final_duration_ms
    assert tuple(stream.getvalue().splitlines()) == test_case.expected_lines


@pytest.mark.parametrize(
    "test_case",
    (
        RetryProjectionCase(
            description="same generic audit column on distinct targets",
            expected_lines=(
                "  1/1  audit     not_null (orders.id) START",
                "  1/1  audit     not_null (customers.id) START",
                "  orders audit OK 0.10s  rich",
                "  customers audit OK 0.20s  rich",
            ),
            expected_unrelated_duration_ms=100.0,
            expected_final_duration_ms=200.0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_same_name_distinct_resources_when_claimed_by_id_then_terminals_do_not_cross(
    test_case: RetryProjectionCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream()
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    projector.configure_resources(ordinals={"not_null": 1}, total=1)
    first_start: LifecycleEvent = replace(
        lifecycle_event(
            "resource_attempt_started",
            run_id="run",
            resource_id="audit:not_null:model:orders:id",
            resource_attempt_id="first-attempt",
            payload={
                "resource_kind": "audit",
                "resource_name": "not_null",
                "attempt_number": 1,
            },
        ),
        event_id="first-start",
    )
    second_start: LifecycleEvent = replace(
        first_start,
        event_id="second-start",
        resource_id="audit:not_null:model:customers:id",
        resource_attempt_id="second-attempt",
    )
    first_terminal: LifecycleEvent = replace(
        first_start,
        event_id="first-terminal",
        event_type="resource_attempt_completed",
        payload={**first_start.payload, "duration_ms": test_case.expected_unrelated_duration_ms},
    )
    second_terminal: LifecycleEvent = replace(
        second_start,
        event_id="second-terminal",
        event_type="resource_attempt_completed",
        payload={**second_start.payload, "duration_ms": test_case.expected_final_duration_ms},
    )

    for event in (first_start, second_start, second_terminal, first_terminal):
        projector.consume(event)
    first_duration: float | None = projector.consume_resource_terminal(
        resource_name="not_null", resource_id="audit:not_null:model:orders:id"
    )
    stream.write(f"  orders audit OK {cast(float, first_duration) / 1000.0:.2f}s  rich\n")
    second_duration: float | None = projector.consume_resource_terminal(
        resource_name="not_null", resource_id="audit:not_null:model:customers:id"
    )
    stream.write(f"  customers audit OK {cast(float, second_duration) / 1000.0:.2f}s  rich\n")
    projector.close()

    assert first_duration == test_case.expected_unrelated_duration_ms
    assert second_duration == test_case.expected_final_duration_ms
    assert tuple(stream.getvalue().splitlines()) == test_case.expected_lines


@pytest.mark.parametrize(
    "test_case",
    (
        StartFlushCase(
            description="blocking resource starts after durable flushed row",
            expected_event_types=("resource_attempt_started", "resource_attempt_completed"),
            expected_flush_count_before_block=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_blocking_resource_when_entered_then_start_is_flushed_before_work(
    test_case: StartFlushCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream()
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    dispatcher: EventDispatcher = EventDispatcher()
    events: list[LifecycleEvent] = []
    dispatcher.subscribe_lifecycle(subscriber=projector.consume, accepts_opaque=False)
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    entered: Event = Event()
    release: Event = Event()

    def block() -> None:
        assert stream.flush_count >= test_case.expected_flush_count_before_block
        assert stream.getvalue().endswith("source    orders START\n")
        entered.set()
        assert release.wait(timeout=5)

    def execute() -> None:
        with invocation_scope("inv"), run_scope("run"), dispatcher_scope(dispatcher):
            with ResourceAttemptLifecycle(
                resource_id="source:orders",
                resource_kind="source",
                resource_name="orders",
            ):
                block()

    thread: Thread = Thread(target=execute)
    thread.start()
    assert entered.wait(timeout=5)
    release.set()
    thread.join(timeout=5)

    assert tuple(event.event_type for event in events) == test_case.expected_event_types


@pytest.mark.parametrize(
    "test_case",
    (
        CursorCleanupCase(
            description="tty cleanup with missing terminal",
            expected_output="  table     abandoned START\n",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_missing_terminal_on_tty_when_closed_then_only_cursor_is_restored(
    test_case: CursorCleanupCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream(tty=True)
    projector: NativeProgressProjector = NativeProgressProjector(
        stream=stream, use_color=False, debug=True
    )
    projector.consume(
        lifecycle_event(
            "resource_attempt_started",
            run_id="run",
            resource_id="model:abandoned",
            resource_attempt_id="attempt",
            payload={
                "resource_kind": "table",
                "resource_name": "abandoned",
                "attempt_number": 1,
            },
        )
    )

    projector.close()

    assert stream.getvalue() == test_case.expected_output


@pytest.mark.parametrize(
    "test_case",
    (
        CursorCleanupCase(
            description="tty standalone operations remain owned by existing spinners",
            expected_output="",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_visible_operation_on_tty_when_projected_then_rich_progress_is_not_duplicated(
    test_case: CursorCleanupCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream(tty=True)
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    start: LifecycleEvent = lifecycle_event(
        "operation_started",
        operation_id="walk",
        payload={"operation_kind": "project", "operation_name": "discovery_filesystem_walk"},
    )
    terminal: LifecycleEvent = replace(
        start,
        event_id="walk-terminal",
        event_type="operation_completed",
        payload={**start.payload, "duration_ms": 10.0},
    )

    projector.consume(start)
    projector.consume(terminal)

    assert stream.getvalue() == test_case.expected_output


@pytest.mark.parametrize(
    "test_case",
    (CursorCleanupCase(description="enriched capture owns tty", expected_output=""),),
    ids=lambda case: case.description,
)
def test_given_enriched_resource_on_tty_when_projected_then_spinner_output_is_not_corrupted(
    test_case: CursorCleanupCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream(tty=True)
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    projector.expect_resource_enrichment(resource_name="capture_orders")
    start: LifecycleEvent = lifecycle_event(
        "resource_attempt_started",
        run_id="run",
        resource_id="sql_scenario:capture_orders",
        resource_attempt_id="attempt",
        payload={
            "resource_kind": "scenario",
            "resource_name": "capture_orders",
            "attempt_number": 1,
        },
    )
    terminal: LifecycleEvent = replace(
        start,
        event_id="capture-terminal",
        event_type="resource_attempt_completed",
        payload={**start.payload, "duration_ms": 10.0},
    )

    projector.consume(start)
    projector.consume(terminal)
    duration: float | None = projector.consume_resource_terminal(
        resource_name="capture_orders",
        resource_id="sql_scenario:capture_orders",
        resource_attempt_id="attempt",
    )

    assert duration == 10.0
    assert stream.getvalue() == test_case.expected_output


@pytest.mark.parametrize(
    "test_case",
    (
        CursorCleanupCase(
            description="standalone warehouse operation is visible on tty",
            expected_output=("Inspect retention  START\nInspect retention  OK  (0.01s)\n"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_standalone_warehouse_operation_on_tty_when_projected_then_progress_is_visible(
    test_case: CursorCleanupCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream(tty=True)
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    start: LifecycleEvent = lifecycle_event(
        "operation_started",
        operation_id="retention",
        payload={
            "operation_kind": "warehouse",
            "operation_name": "retention_inspection",
            "phase": "inspect",
            "adapter": "snowflake",
            "target_kind": "relation",
        },
    )
    terminal: LifecycleEvent = replace(
        start,
        event_id="retention-terminal",
        event_type="operation_completed",
        payload={**start.payload, "duration_ms": 10.0},
    )

    projector.consume(start)
    projector.consume(terminal)

    assert stream.getvalue() == test_case.expected_output


@pytest.mark.parametrize(
    "test_case",
    (
        CursorCleanupCase(
            description="clone finalization phase is visible",
            expected_output="Clone finalization  START\nClone finalization  OK  (0.01s)\n",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_clone_finalization_lifecycle_when_projected_then_phase_is_visible(
    test_case: CursorCleanupCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream(tty=False)
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    start: LifecycleEvent = lifecycle_event(
        "operation_started",
        operation_id="clone-finalization",
        payload={
            "operation_kind": "clone",
            "operation_name": "clone_finalization",
        },
    )
    terminal: LifecycleEvent = replace(
        start,
        event_id="clone-finalization-terminal",
        event_type="operation_completed",
        payload={**start.payload, "duration_ms": 10.0},
    )

    projector.consume(start)
    projector.consume(terminal)

    assert stream.getvalue() == test_case.expected_output


@pytest.mark.parametrize(
    "test_case",
    (
        FreshnessMetadataProjectionCase(
            description="completed batch reports source and unknown counts",
            terminal_event_type="operation_completed",
            terminal_payload={"metadata": {"item_count": 257, "unknown_count": 3}},
            expected_terminal_line="Freshness metadata  OK (257 sources, 3 unknown)  (0.01s)",
        ),
        FreshnessMetadataProjectionCase(
            description="failed batch explains that every source is unknown",
            terminal_event_type="operation_failed",
            terminal_payload={"metadata": {"item_count": 2}, "error_type": "AdapterUserError"},
            expected_terminal_line=(
                "Freshness metadata  FAIL (2 sources unknown, metadata lookup failed: "
                "AdapterUserError)  (0.01s)"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_freshness_metadata_terminal_when_projected_then_line_explains_outcome(
    test_case: FreshnessMetadataProjectionCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream()
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    start: LifecycleEvent = lifecycle_event(
        "operation_started",
        operation_id="freshness-metadata",
        payload={
            "operation_kind": "freshness",
            "operation_name": "source_freshness_metadata_observation",
        },
    )
    terminal: LifecycleEvent = replace(
        start,
        event_id="freshness-metadata-terminal",
        event_type=test_case.terminal_event_type,
        payload={**start.payload, **test_case.terminal_payload, "duration_ms": 10.0},
    )

    projector.consume(start)
    projector.consume(terminal)

    assert stream.getvalue().splitlines() == [
        "Freshness metadata  START",
        test_case.expected_terminal_line,
    ]


@pytest.mark.parametrize(
    "test_case",
    (
        CursorCleanupCase(
            description="standalone warehouse operation is visible without tty",
            expected_output="Promote relation  START\nPromote relation  OK  (0.01s)\n",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_standalone_warehouse_operation_without_tty_when_projected_then_progress_is_visible(
    test_case: CursorCleanupCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream()
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    start: LifecycleEvent = lifecycle_event(
        "operation_started",
        operation_id="promotion",
        payload={"operation_kind": "warehouse", "operation_name": "relation_promotion"},
    )
    terminal: LifecycleEvent = replace(
        start,
        event_id="promotion-terminal",
        event_type="operation_completed",
        payload={**start.payload, "duration_ms": 10.0},
    )

    projector.consume(start)
    projector.consume(terminal)

    assert stream.getvalue() == test_case.expected_output


@pytest.mark.parametrize(
    "test_case",
    (CursorCleanupCase(description="runtime schema events stay silent", expected_output=""),),
    ids=lambda case: case.description,
)
def test_given_many_runtime_schema_events_when_projected_then_output_does_not_grow(
    test_case: CursorCleanupCase,
) -> None:
    stream: _FlushRecordingStream = _FlushRecordingStream(tty=True)
    projector: NativeProgressProjector = NativeProgressProjector(stream=stream, use_color=False)
    first_start: LifecycleEvent = replace(
        lifecycle_event(
            "operation_started",
            operation_id="schema-1",
            payload={
                "operation_kind": "warehouse",
                "operation_name": "runtime_schema_inspection",
            },
        ),
        event_id="schema-start-1",
    )
    first_terminal: LifecycleEvent = replace(
        first_start,
        event_id="schema-terminal-1",
        event_type="operation_completed",
        payload={**first_start.payload, "duration_ms": 10.0},
    )
    second_start: LifecycleEvent = replace(
        first_start,
        event_id="schema-start-2",
        operation_id="schema-2",
    )
    second_terminal: LifecycleEvent = replace(
        first_terminal,
        event_id="schema-terminal-2",
        operation_id="schema-2",
    )

    projector.consume(first_start)
    projector.consume(first_terminal)
    projector.consume(second_start)
    projector.consume(second_terminal)

    assert stream.getvalue() == test_case.expected_output


@pytest.mark.parametrize(
    "test_case",
    (
        LockOrderCase(
            description="reporter holding the line lock claims a terminal",
            timeout_seconds=5.0,
            expected_alive_threads=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_line_lock_held_when_projector_writes_concurrently_then_no_deadlock(
    test_case: LockOrderCase,
) -> None:
    projector: NativeProgressProjector = NativeProgressProjector(stream=StringIO(), use_color=False)
    line_lock_held: Event = Event()

    def project_resource() -> None:
        dispatcher: EventDispatcher = EventDispatcher()
        dispatcher.subscribe_lifecycle(subscriber=projector.consume, accepts_opaque=False)
        line_lock_held.wait()
        with (
            invocation_scope("inv-lock-order"),
            run_scope("run-lock-order"),
            dispatcher_scope(dispatcher),
            ResourceAttemptLifecycle(
                resource_id="asset:writer", resource_kind="task", resource_name="writer"
            ) as lifecycle,
        ):
            lifecycle.skipped(skip_code="scheduler")

    def claim_terminal_under_line_lock() -> None:
        with shared_transient_line_coordinator().lock:
            line_lock_held.set()
            time.sleep(0.2)
            projector.consume_resource_terminal(resource_name="writer")

    threads: tuple[Thread, ...] = (
        Thread(target=project_resource, daemon=True),
        Thread(target=claim_terminal_under_line_lock, daemon=True),
    )
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join(timeout=test_case.timeout_seconds)

    alive: int = sum(thread.is_alive() for thread in threads)
    assert alive == test_case.expected_alive_threads
