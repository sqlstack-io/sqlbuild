from __future__ import annotations

import sys
import threading
import time
from collections.abc import Callable, Mapping
from concurrent.futures import ThreadPoolExecutor
from datetime import UTC, datetime
from queue import SimpleQueue
from typing import cast

import pytest

from sqlbuild._native import (
    COMMAND_OUTPUT_LOSS_RECORD_TYPE,
    BoundEventExporter,
    EventExporterCounts,
    EventExporterDispatcher,
    EventExportSeverity,
    EventExportSummary,
)
from sqlbuild.observability import LifecycleEvent
from sqlbuild.runtime.event_exporting.exceptions import EventExporterInputError
from sqlbuild.runtime.event_exporting.main._invoke_lifecycle_event_sink import (
    invoke_lifecycle_event_sink,
)
from sqlbuild.runtime.output_capture.models import BoundCommandOutputSink
from sqlbuild.sinks import CommandOutputRecord, CommandOutputStream
from tests.unit.src.sqlbuild.runtime.event_exporting.classes._test_types import (
    EventExporterDispatcherTestCase,
    HealthIntervalTestCase,
    IdleDispatcherTestCase,
    OutcomeDeliveryTestCase,
)
from tests.unit.src.sqlbuild.runtime.event_exporting.classes.helpers import (
    correlated_event,
    lifecycle_event,
)


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("loss summary bypasses stream filter", 1),),
    ids=lambda case: case.description,
)
def test_given_stream_filter_when_output_loss_occurs_then_sink_receives_loss_summary(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    records: list[CommandOutputRecord] = []

    def publish(*, record: CommandOutputRecord) -> None:
        records.append(record)

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(exporters=())
    dispatcher.bind_command_output_sinks(
        (
            BoundCommandOutputSink(
                name="stdout_only",
                function=publish,
                provider_arguments={},
                streams=frozenset({CommandOutputStream.STDOUT}),
            ),
        )
    )
    dispatcher.export_output(
        (
            CommandOutputRecord(
                invocation_id="invocation",
                sequence=0,
                occurred_at=datetime.now(UTC),
                stream=CommandOutputStream.STDERR,
                message="filtered\n",
                external_context={},
            ),
            CommandOutputRecord(
                invocation_id="invocation",
                sequence=1,
                occurred_at=datetime.now(UTC),
                stream=CommandOutputStream.STDERR,
                message="SQLBuild output export dropped 2 record(s)",
                external_context={},
                record_type=COMMAND_OUTPUT_LOSS_RECORD_TYPE,
                dropped_records=2,
            ),
        )
    )
    _ = dispatcher.shutdown()

    assert len(records) == test_case.expected_delivered
    assert records[0].record_type == COMMAND_OUTPUT_LOSS_RECORD_TYPE


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("off-thread delivery", 1),),
    ids=lambda case: case.description,
)
def test_given_slow_exporter_when_enqueuing_then_execution_thread_is_not_blocked(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    release: threading.Event = threading.Event()
    entered: threading.Event = threading.Event()
    caller_thread: int = threading.get_ident()
    exporter_threads: list[int] = []

    def publish(*, event: LifecycleEvent) -> None:
        del event
        exporter_threads.append(threading.get_ident())
        entered.set()
        release.wait()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("publish", publish, {}),),
        invocation_timeout_seconds=0.2,
    )
    started: float = time.monotonic()
    dispatcher.enqueue(lifecycle_event())

    assert time.monotonic() - started < 0.05
    assert entered.wait(timeout=0.2)
    assert exporter_threads == [exporter_threads[0]]
    assert exporter_threads[0] != caller_thread
    release.set()
    assert dispatcher.shutdown().delivered == test_case.expected_delivered


@pytest.mark.parametrize(
    "test_case",
    (
        IdleDispatcherTestCase(
            description="an idle dispatcher waits for work instead of polling",
            idle_seconds=0.5,
            polling_seconds=0.5,
            expected_max_waits=10,
            expected_max_delivery_seconds=0.5,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_idle_dispatcher_when_waiting_then_it_does_not_poll_and_still_delivers_promptly(
    test_case: IdleDispatcherTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    delivered: list[threading.Event] = [threading.Event(), threading.Event()]
    published: list[int] = []
    waits: list[tuple[int, float | None]] = []
    condition_wait: Callable[[threading.Condition, float | None], bool] = threading.Condition.wait

    def counted_wait(self: threading.Condition, timeout: float | None = None) -> bool:
        waits.append((threading.get_ident(), timeout))
        return condition_wait(self, timeout)

    monkeypatch.setattr(threading.Condition, "wait", counted_wait)

    def publish(*, event: LifecycleEvent) -> None:
        del event
        delivered[len(published)].set()
        published.append(1)

    existing_threads: set[int | None] = {thread.ident for thread in threading.enumerate()}
    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("publish", publish, {}),),
    )
    dispatcher.enqueue(lifecycle_event())
    assert delivered[0].wait(timeout=2.0)
    time.sleep(0.05)
    # Threads left by earlier tests wait too, so count only the threads this dispatcher started.
    dispatcher_threads: set[int | None] = {
        thread.ident for thread in threading.enumerate()
    } - existing_threads
    assert dispatcher_threads
    waits.clear()
    time.sleep(test_case.idle_seconds)
    # Only waits shorter than the idle safety timeout poll.
    idle_waits: int = sum(
        thread in dispatcher_threads and timeout is not None and timeout < test_case.polling_seconds
        for thread, timeout in waits
    )
    started: float = time.monotonic()
    dispatcher.enqueue(lifecycle_event())

    assert delivered[1].wait(timeout=2.0)
    assert time.monotonic() - started < test_case.expected_max_delivery_seconds
    assert idle_waits <= test_case.expected_max_waits
    assert dispatcher.shutdown().delivered == 2


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("failure isolation", 1),),
    ids=lambda case: case.description,
)
def test_given_multiple_exporters_when_one_fails_then_the_other_exporter_still_runs(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    calls: list[str] = []

    def failing(*, event: LifecycleEvent) -> None:
        del event
        calls.append("first")
        raise RuntimeError("secret destination detail")

    def succeeding(*, event: LifecycleEvent) -> None:
        del event
        calls.append("second")

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(
            BoundEventExporter("first", failing, {}),
            BoundEventExporter("second", succeeding, {}),
        )
    )
    dispatcher.enqueue(lifecycle_event())

    summary: EventExportSummary = dispatcher.shutdown()

    assert sorted(calls) == ["first", "second"]
    assert summary.delivered == test_case.expected_delivered
    assert summary.failed == 1
    assert summary.accepted == summary.delivered + summary.failed + summary.dropped
    assert tuple(item.exporter_name for item in summary.per_exporter) == ("first", "second")


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("immutable event", 1),),
    ids=lambda case: case.description,
)
def test_given_exporter_mutation_attempt_when_delivering_then_event_remains_immutable(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    observed_commands: list[object] = []

    def mutating(*, event: LifecycleEvent) -> None:
        event.payload["command"] = "changed"  # ty: ignore[invalid-assignment]

    def observing(*, event: LifecycleEvent) -> None:
        observed_commands.append(event.payload["command"])

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(
            BoundEventExporter("mutating", mutating, {}),
            BoundEventExporter("observing", observing, {}),
        )
    )
    dispatcher.enqueue(lifecycle_event())

    summary: EventExportSummary = dispatcher.shutdown()

    assert observed_commands == ["build"]
    assert summary.failed == 1
    assert summary.delivered == test_case.expected_delivered
    assert summary.flush_complete


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("bounded hung shutdown", 0),),
    ids=lambda case: case.description,
)
def test_given_hung_exporter_when_shutting_down_then_flush_is_bounded_and_threads_are_daemon(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    blocker: threading.Event = threading.Event()

    def publish(*, event: LifecycleEvent) -> None:
        del event
        blocker.wait()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("hung", publish, {}),),
        shutdown_timeout_seconds=0.05,
        invocation_timeout_seconds=10.0,
    )
    dispatcher.enqueue(lifecycle_event())
    started: float = time.monotonic()

    summary: EventExportSummary = dispatcher.shutdown()

    assert time.monotonic() - started < 0.2
    assert summary.failed == 1
    assert summary.delivered == test_case.expected_delivered
    assert not summary.flush_complete
    assert not dispatcher.thread.is_alive()
    exporter_threads_are_daemon: bool = all(
        map(
            lambda thread: thread.daemon,
            filter(
                lambda thread: thread.name.startswith("sqlbuild-event-exporter-"),
                threading.enumerate(),
            ),
        )
    )
    assert exporter_threads_are_daemon
    blocker.set()


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("bounded queue overflow", 1),),
    ids=lambda case: case.description,
)
def test_given_full_queue_when_enqueuing_then_overflow_is_counted_per_exporter(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    release: threading.Event = threading.Event()

    def publish(*, event: LifecycleEvent) -> None:
        del event
        release.wait()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("publish", publish, {}),),
        queue_capacity=1,
        invocation_timeout_seconds=0.2,
    )
    for index in range(100):
        dispatcher.enqueue(correlated_event(index, "statement_submitted"))
    time.sleep(0.02)
    release.set()

    summary: EventExportSummary = dispatcher.shutdown()

    assert summary.dropped > 0
    assert summary.accepted == summary.delivered + summary.failed + summary.dropped == 100
    assert summary.delivered >= test_case.expected_delivered


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("last live invocation finalizes once", 0),),
    ids=lambda case: case.description,
)
def test_given_multiple_timed_out_invocations_when_finalizing_then_runs_once_on_caller_after_last(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    first_entered: threading.Event = threading.Event()
    second_entered: threading.Event = threading.Event()
    first_release: threading.Event = threading.Event()
    second_release: threading.Event = threading.Event()
    calls: list[str] = []

    def first(*, event: LifecycleEvent) -> None:
        del event
        first_entered.set()
        first_release.wait()
        calls.append("first returned")

    def second(*, event: LifecycleEvent) -> None:
        del event
        second_entered.set()
        second_release.wait()
        calls.append("second returned")

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(
            BoundEventExporter("first", first, {}),
            BoundEventExporter("second", second, {}),
        ),
        shutdown_timeout_seconds=0.1,
        invocation_timeout_seconds=0.3,
    )
    dispatcher.enqueue(lifecycle_event())
    assert first_entered.wait(timeout=1.0)
    assert second_entered.wait(timeout=1.0)
    summary: EventExportSummary = dispatcher.shutdown()

    def finalize() -> None:
        calls.append(f"finalized on {threading.current_thread().name}")

    releases: tuple[threading.Timer, ...] = (
        threading.Timer(0.05, first_release.set),
        threading.Timer(0.1, second_release.set),
    )
    for release in releases:
        release.start()
    dispatcher.finalize_when_idle(finalize)

    assert calls == [
        "first returned",
        "second returned",
        f"finalized on {threading.current_thread().name}",
    ]
    assert summary.delivered == test_case.expected_delivered


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("blocked diagnostics do not block shutdown", 0),),
    ids=lambda case: case.description,
)
def test_given_blocked_failure_notification_when_shutting_down_then_summary_is_final_and_bounded(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    failure_started: threading.Event = threading.Event()
    release_failure: threading.Event = threading.Event()
    summary_received: threading.Event = threading.Event()
    callback_summaries: list[EventExportSummary] = []

    def failing(*, event: LifecycleEvent) -> None:
        del event
        raise RuntimeError("sanitized")

    def failure_callback(_failure: object) -> None:
        failure_started.set()
        release_failure.wait()

    def summary_callback(summary: EventExportSummary) -> None:
        callback_summaries.append(summary)
        summary_received.set()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("failing", failing, {}),),
        failure_callback=failure_callback,
        summary_callback=summary_callback,
        shutdown_timeout_seconds=0.05,
    )
    dispatcher.enqueue(lifecycle_event())
    assert failure_started.wait(timeout=0.2)
    started: float = time.monotonic()

    summary: EventExportSummary = dispatcher.shutdown()

    assert time.monotonic() - started < 0.2
    assert not dispatcher.thread.is_alive()
    assert summary.failed == 1
    assert summary.delivered == test_case.expected_delivered
    notifier: threading.Thread | None = dispatcher.notification_thread
    assert notifier is not None
    assert notifier.daemon
    assert notifier.is_alive()
    release_failure.set()
    assert summary_received.wait(timeout=0.2)
    assert callback_summaries == [summary]


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("concurrent shutdown shares final summary", 0),),
    ids=lambda case: case.description,
)
def test_given_concurrent_shutdown_callers_when_dispatcher_stops_then_summary_is_cached_once(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    callers_ready: threading.Barrier = threading.Barrier(2)
    summary_received: threading.Event = threading.Event()
    callback_summaries: list[EventExportSummary] = []

    def summary_callback(summary: EventExportSummary) -> None:
        callback_summaries.append(summary)
        summary_received.set()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(),
        summary_callback=summary_callback,
    )

    def shutdown() -> EventExportSummary:
        _ = callers_ready.wait()
        return dispatcher.shutdown()

    with ThreadPoolExecutor(max_workers=2) as executor:
        summaries: tuple[EventExportSummary, ...] = tuple(
            executor.map(lambda _: shutdown(), range(2))
        )

    assert summaries[0] is summaries[1]
    assert summaries[0].delivered == test_case.expected_delivered
    assert summary_received.wait(timeout=0.2)
    assert callback_summaries == [summaries[0]]


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("prequeue filtering", 0),),
    ids=lambda case: case.description,
)
def test_given_exporter_specific_filters_when_enqueuing_then_filtering_precedes_capacity(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    calls: list[str] = []

    def publish(*, event: LifecycleEvent) -> None:
        calls.append(event.event_type)

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(
            BoundEventExporter(
                "runs",
                publish,
                {},
                event_kinds=frozenset({"run"}),
                min_severity=EventExportSeverity.DEBUG,
            ),
            BoundEventExporter(
                "errors",
                publish,
                {},
                event_kinds=frozenset({"invocation"}),
                min_severity=EventExportSeverity.ERROR,
            ),
        ),
        queue_capacity=1,
    )

    dispatcher.enqueue(lifecycle_event())
    summary: EventExportSummary = dispatcher.shutdown()

    assert len(calls) == test_case.expected_delivered
    assert summary.accepted == 0
    assert summary.filtered == 2
    assert summary.dropped == 0
    assert summary.flush_complete


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("mixed eligibility", 0),),
    ids=lambda case: case.description,
)
def test_given_mixed_eligible_fanout_when_exporters_disagree_then_counts_are_independent(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    calls: list[str] = []

    def failing(*, event: LifecycleEvent) -> None:
        del event
        raise ValueError("must never reach diagnostics")

    def succeeding(*, event: LifecycleEvent) -> None:
        calls.append(event.event_type)

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(
            BoundEventExporter("failing", failing, {}),
            BoundEventExporter(
                "statement_only", succeeding, {}, event_kinds=frozenset({"statement"})
            ),
        )
    )
    dispatcher.enqueue(lifecycle_event())

    summary: EventExportSummary = dispatcher.shutdown()

    assert len(calls) == test_case.expected_delivered
    assert summary.accepted == 1
    assert summary.filtered == 1
    assert summary.failed == 1
    assert summary.per_exporter[0].counts.failed == 1
    assert summary.per_exporter[1].counts.filtered == 1


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("mixed displacement", 3),),
    ids=lambda case: case.description,
)
def test_given_mixed_eligible_queue_when_high_priority_displaces_then_attempts_count_once(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    entered: threading.Event = threading.Event()
    release: threading.Event = threading.Event()
    calls: list[tuple[str, str]] = []

    def first(*, event: LifecycleEvent) -> None:
        calls.append(("first", event.event_type))
        entered.set()
        release.wait()

    def second(*, event: LifecycleEvent) -> None:
        calls.append(("second", event.event_type))

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(
            BoundEventExporter("first", first, {}),
            BoundEventExporter("second", second, {}, min_severity=EventExportSeverity.INFO),
        ),
        queue_capacity=1,
        invocation_timeout_seconds=1,
    )
    dispatcher.enqueue(correlated_event(1, "statement_submitted"))
    assert entered.wait(timeout=0.2)
    dispatcher.enqueue(correlated_event(2, "statement_submitted"))
    dispatcher.enqueue(correlated_event(3, "operation_failed"))
    release.set()

    summary: EventExportSummary = dispatcher.shutdown()

    assert len(calls) == test_case.expected_delivered
    assert sorted(calls) == [
        ("first", "operation_failed"),
        ("first", "statement_submitted"),
        ("second", "operation_failed"),
    ]
    first_counts: EventExporterCounts = summary.per_exporter[0].counts
    second_counts: EventExporterCounts = summary.per_exporter[1].counts
    assert (first_counts.accepted, first_counts.delivered, first_counts.dropped) == (3, 2, 1)
    assert (second_counts.filtered, second_counts.accepted, second_counts.delivered) == (2, 1, 1)
    assert summary.accepted == summary.delivered + summary.failed + summary.dropped


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("producer stress", 400),),
    ids=lambda case: case.description,
)
def test_given_concurrent_producers_when_queue_overloads_then_final_counts_remain_bounded(
    test_case: EventExporterDispatcherTestCase,
) -> None:

    def publish(*, event: LifecycleEvent) -> None:
        del event

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("publish", publish, {}),), queue_capacity=8
    )
    with ThreadPoolExecutor(max_workers=8) as executor:
        tuple(
            executor.map(
                lambda index: dispatcher.enqueue(lifecycle_event(index)),
                range(test_case.expected_delivered),
            )
        )

    summary: EventExportSummary = dispatcher.shutdown()

    assert summary.accepted == test_case.expected_delivered
    assert summary.accepted == summary.delivered + summary.failed + summary.dropped
    assert summary.queue_depth == 0
    assert summary.queue_capacity == 8
    assert summary.flush_complete


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("safe health", 1),),
    ids=lambda case: case.description,
)
def test_given_sensitive_exporter_exception_when_reporting_then_only_safe_dimensions_escape(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    failures: list[object] = []
    reported: threading.Event = threading.Event()

    def failing(*, event: LifecycleEvent) -> None:
        del event
        raise RuntimeError("password=secret sql=select-sensitive destination=broker")

    def report(failure: object) -> None:
        failures.append(failure)
        reported.set()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("safe_name", failing, {}),),
        failure_callback=report,
    )
    dispatcher.enqueue(lifecycle_event())
    dispatcher.shutdown()

    assert reported.wait(timeout=1.0)
    assert len(failures) == test_case.expected_delivered
    rendered: str = repr(failures[0])
    assert "RuntimeError" in rendered
    assert "safe_name" in rendered
    assert "invocation" in rendered
    assert "debug" in rendered
    assert "secret" not in rendered
    assert "select-sensitive" not in rendered
    assert "broker" not in rendered


@pytest.mark.parametrize(
    "test_case",
    (
        HealthIntervalTestCase("zero", 0.0, "positive and finite"),
        HealthIntervalTestCase("negative", -1.0, "positive and finite"),
        HealthIntervalTestCase("infinite", float("inf"), "positive and finite"),
        HealthIntervalTestCase("nan", float("nan"), "positive and finite"),
        HealthIntervalTestCase("boolean", True, "positive and finite"),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_health_interval_when_constructing_then_rejects(
    test_case: HealthIntervalTestCase,
) -> None:
    with pytest.raises(EventExporterInputError, match=test_case.expected_error):
        EventExporterDispatcher(health_interval_seconds=cast(float, test_case.interval))


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("live periodic filtering", 1),),
    ids=lambda case: case.description,
)
def test_given_live_accounting_change_when_interval_elapses_then_periodic_summary_is_published(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    received: threading.Event = threading.Event()
    summaries: list[EventExportSummary] = []

    def summary_callback(summary: EventExportSummary) -> None:
        summaries.append(summary)
        received.set()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(
            BoundEventExporter("runs", lambda *, event: None, {}, event_kinds=frozenset({"run"})),
        ),
        summary_callback=summary_callback,
        health_interval_seconds=0.01,
    )
    dispatcher.enqueue(lifecycle_event())

    assert received.wait(timeout=0.2)
    assert len(summaries) == test_case.expected_delivered
    assert summaries[0].filtered == 1
    assert not summaries[0].flush_complete
    dispatcher.shutdown()


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("coalesced periodic and retained final", 3),),
    ids=lambda case: case.description,
)
def test_given_blocked_notifier_when_health_changes_and_shutdown_then_newest_and_final_are_retained(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    callback_entered: threading.Event = threading.Event()
    callback_release: threading.Event = threading.Event()
    callbacks_completed: threading.Semaphore = threading.Semaphore(0)
    summaries: list[EventExportSummary] = []

    def summary_callback(summary: EventExportSummary) -> None:
        summaries.append(summary)
        callback_entered.set()
        callback_release.wait()
        callbacks_completed.release()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(
            BoundEventExporter("runs", lambda *, event: None, {}, event_kinds=frozenset({"run"})),
        ),
        summary_callback=summary_callback,
        health_interval_seconds=0.01,
    )
    dispatcher.enqueue(lifecycle_event(1))
    assert callback_entered.wait(timeout=0.2)
    dispatcher.enqueue(lifecycle_event(2))
    time.sleep(0.015)
    dispatcher.enqueue(lifecycle_event(3))
    time.sleep(0.03)

    final_summary: EventExportSummary = dispatcher.shutdown()
    assert summaries == [summaries[0]]
    callback_release.set()

    assert callbacks_completed.acquire(timeout=0.2)
    assert callbacks_completed.acquire(timeout=0.2)
    assert callbacks_completed.acquire(timeout=0.2)
    assert len(summaries) == test_case.expected_delivered
    assert [summary.filtered for summary in summaries] == [1, 3, 3]
    assert [summary.flush_complete for summary in summaries] == [False, False, True]
    assert summaries[-1] is final_summary


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("idle has final only", 1),),
    ids=lambda case: case.description,
)
def test_given_idle_dispatcher_when_intervals_repeat_then_no_duplicate_periodic_health_is_published(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    final_received: threading.Event = threading.Event()
    summaries: list[EventExportSummary] = []

    def summary_callback(summary: EventExportSummary) -> None:
        summaries.append(summary)
        final_received.set()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(), summary_callback=summary_callback, health_interval_seconds=0.01
    )
    time.sleep(0.04)
    assert summaries == []

    dispatcher.shutdown()

    assert final_received.wait(timeout=0.2)
    assert len(summaries) == test_case.expected_delivered
    assert summaries[0].flush_complete


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("health bypasses exporter", 1),),
    ids=lambda case: case.description,
)
def test_given_periodic_health_callback_when_published_then_it_never_reenters_exporter_queue(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    delivered: threading.Event = threading.Event()
    published: threading.Condition = threading.Condition()
    summaries: list[EventExportSummary] = []
    exported: list[LifecycleEvent] = []

    def export(*, event: LifecycleEvent) -> None:
        exported.append(event)
        delivered.set()

    def summary_callback(summary: EventExportSummary) -> None:
        with published:
            summaries.append(summary)
            published.notify_all()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("publish", export, {}),),
        summary_callback=summary_callback,
        health_interval_seconds=0.01,
    )
    dispatcher.enqueue(lifecycle_event())

    assert delivered.wait(timeout=10)
    with published:
        assert published.wait_for(
            lambda: any(summary.delivered == 1 for summary in summaries), timeout=10
        )
    final_summary: EventExportSummary = dispatcher.shutdown()

    assert len(exported) == test_case.expected_delivered
    assert (final_summary.accepted, final_summary.delivered) == (
        test_case.expected_delivered,
        test_case.expected_delivered,
    )
    assert summaries[0].accepted == 1


@pytest.mark.parametrize(
    "test_case",
    (
        OutcomeDeliveryTestCase(
            "saturated queue still admits outcomes",
            queue_capacity=4,
            backlog=100,
            sink_seconds=0.0,
            shutdown_timeout_seconds=5.0,
            expected_outcomes=("run_completed", "invocation_completed"),
        ),
        OutcomeDeliveryTestCase(
            "tight budget drops backlog for outcomes",
            queue_capacity=1024,
            backlog=300,
            sink_seconds=0.02,
            shutdown_timeout_seconds=0.4,
            expected_outcomes=("run_completed", "invocation_completed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_backlog_when_shutting_down_then_outcome_events_arrive_and_backlog_is_dropped(
    test_case: OutcomeDeliveryTestCase,
) -> None:
    held: threading.Event = threading.Event()
    release: threading.Event = threading.Event()
    delivered: list[str] = []

    def record(*, event: LifecycleEvent) -> None:
        delivered.append(event.event_type)
        held.set()
        _ = release.wait()
        time.sleep(test_case.sink_seconds)

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("recorder", record, {}),),
        queue_capacity=test_case.queue_capacity,
        shutdown_timeout_seconds=test_case.shutdown_timeout_seconds,
        invocation_timeout_seconds=5.0,
    )
    dispatcher.enqueue(correlated_event(0, "run_started"))
    assert held.wait(timeout=1.0)
    for index in range(test_case.backlog):
        dispatcher.enqueue(correlated_event(index + 1, "statement_completed"))
    for offset, event_type in enumerate(test_case.expected_outcomes):
        dispatcher.enqueue(correlated_event(test_case.backlog + 1 + offset, event_type))
    release.set()

    summary: EventExportSummary = dispatcher.shutdown()

    assert (
        tuple(filter(test_case.expected_outcomes.__contains__, delivered))
        == test_case.expected_outcomes
    )
    assert delivered[-1] == "invocation_completed"
    assert summary.dropped > 0
    assert summary.delivered + summary.dropped + summary.failed == summary.accepted


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("second call started before the reserve", 4),),
    ids=lambda case: case.description,
)
def test_given_lane_running_its_second_call_when_reserve_begins_then_it_counts_delivered(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    event_ids: tuple[str, ...] = ("event-1", "event-2", "event-3", "event-4")
    started: dict[str, threading.Event] = {event_id: threading.Event() for event_id in event_ids}
    gates: dict[str, threading.Event] = {event_id: threading.Event() for event_id in event_ids}
    for event_id in ("event-1", "event-3", "event-4"):
        gates[event_id].set()
    release_second: threading.Event = gates["event-2"]
    second_started: threading.Event = started["event-2"]
    delivered: list[str] = []

    def record(*, event: LifecycleEvent) -> None:
        delivered.append(event.event_id)
        started[event.event_id].set()
        _ = gates[event.event_id].wait()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("recorder", record, {}),),
        shutdown_timeout_seconds=2.0,
        invocation_timeout_seconds=5.0,
    )
    dispatcher.enqueue(correlated_event(1, "statement_completed"))
    dispatcher.enqueue(correlated_event(2, "statement_completed"))
    dispatcher.enqueue(correlated_event(3, "run_completed"))
    dispatcher.enqueue(lifecycle_event(4, "invocation_completed"))
    assert second_started.wait(timeout=1.0)
    release: threading.Timer = threading.Timer(1.7, release_second.set)
    release.start()

    summary: EventExportSummary = dispatcher.shutdown()

    release.cancel()
    assert tuple(delivered) == event_ids
    assert summary.delivered == test_case.expected_delivered
    assert (summary.dropped, summary.failed) == (0, 0)


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("claimed call runs, cancelled call is skipped", 1),),
    ids=lambda case: case.description,
)
def test_given_claims_when_delivery_thread_runs_calls_then_only_unclaimed_cancellations_skip(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    calls: SimpleQueue[
        tuple[Callable[..., object], Mapping[str, object], LifecycleEvent, int] | None
    ] = SimpleQueue()
    results: SimpleQueue[tuple[int, BaseException | None]] = SimpleQueue()
    claims: dict[int, bool] = {2: False}
    seen: list[str] = []

    def record(*, event: LifecycleEvent) -> None:
        seen.append(event.event_id)
        assert claims.setdefault(1, False)

    calls.put((record, {}, correlated_event(1, "statement_completed"), 1))
    calls.put((record, {}, correlated_event(2, "statement_completed"), 2))
    calls.put(None)

    invoke_lifecycle_event_sink(calls=calls, results=results, claims=claims)

    assert seen == ["event-1"]
    assert results.qsize() == test_case.expected_delivered
    assert results.get_nowait() == (1, None)
    assert claims == {1: True}


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("delivery resumes after the slow call returns", 5),),
    ids=lambda case: case.description,
)
def test_given_sink_slow_once_when_call_returns_then_later_and_outcome_events_arrive(
    test_case: EventExporterDispatcherTestCase,
) -> None:
    event_ids: tuple[str, ...] = tuple(f"event-{index}" for index in range(1, 8))
    started: dict[str, threading.Event] = {event_id: threading.Event() for event_id in event_ids}
    gates: dict[str, threading.Event] = {event_id: threading.Event() for event_id in event_ids}
    for event_id in event_ids[:1] + event_ids[2:]:
        gates[event_id].set()
    first_delivered: threading.Event = started["event-1"]
    slow_started: threading.Event = started["event-2"]
    release: threading.Event = gates["event-2"]
    delivered: list[str] = []

    def record(*, event: LifecycleEvent) -> None:
        delivered.append(event.event_id)
        started[event.event_id].set()
        _ = gates[event.event_id].wait()

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("recorder", record, {}),),
        shutdown_timeout_seconds=5.0,
        invocation_timeout_seconds=0.1,
    )
    dispatcher.enqueue(correlated_event(1, "run_started"))
    assert first_delivered.wait(timeout=1.0)
    dispatcher.enqueue(correlated_event(2, "statement_completed"))
    assert slow_started.wait(timeout=1.0)
    time.sleep(0.3)
    dispatcher.enqueue(correlated_event(3, "statement_completed"))
    dispatcher.enqueue(correlated_event(4, "run_completed"))
    time.sleep(0.05)
    release.set()
    time.sleep(0.1)
    dispatcher.enqueue(correlated_event(5, "statement_completed"))
    dispatcher.enqueue(correlated_event(6, "statement_completed"))
    dispatcher.enqueue(lifecycle_event(7, "invocation_completed"))

    summary: EventExportSummary = dispatcher.shutdown()

    assert delivered == ["event-1", "event-2", "event-4", "event-5", "event-6", "event-7"]
    assert summary.delivered == test_case.expected_delivered
    assert (summary.failed, summary.dropped) == (1, 1)
    assert summary.delivered + summary.failed + summary.dropped == summary.accepted


class _BrokenStream:
    def write(self, text: str) -> int:
        raise OSError(f"stderr is closed: {len(text)}")

    def flush(self) -> None:
        raise OSError("stderr is closed")


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDispatcherTestCase("a failing stderr does not stop delivery", 2),),
    ids=lambda case: case.description,
)
def test_given_failing_stderr_when_sink_stalls_then_delivery_and_counts_continue(
    test_case: EventExporterDispatcherTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    event_ids: tuple[str, ...] = tuple(f"event-{index}" for index in range(1, 5))
    started: dict[str, threading.Event] = {event_id: threading.Event() for event_id in event_ids}
    gates: dict[str, threading.Event] = {event_id: threading.Event() for event_id in event_ids}
    for event_id in event_ids[1:]:
        gates[event_id].set()
    slow_started: threading.Event = started["event-1"]
    release: threading.Event = gates["event-1"]
    delivered: list[str] = []

    def record(*, event: LifecycleEvent) -> None:
        delivered.append(event.event_id)
        started[event.event_id].set()
        _ = gates[event.event_id].wait()

    monkeypatch.setattr(sys, "stderr", _BrokenStream())
    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        exporters=(BoundEventExporter("recorder", record, {}),),
        shutdown_timeout_seconds=5.0,
        invocation_timeout_seconds=0.1,
    )
    dispatcher.enqueue(correlated_event(1, "statement_completed"))
    assert slow_started.wait(timeout=1.0)
    time.sleep(0.3)
    dispatcher.enqueue(correlated_event(2, "statement_completed"))
    time.sleep(0.05)
    release.set()
    time.sleep(0.1)
    dispatcher.enqueue(correlated_event(3, "statement_completed"))
    dispatcher.enqueue(lifecycle_event(4, "invocation_completed"))

    summary: EventExportSummary = dispatcher.shutdown()

    assert delivered == ["event-1", "event-3", "event-4"]
    assert summary.delivered == test_case.expected_delivered
    assert summary.delivered + summary.failed + summary.dropped == summary.accepted
