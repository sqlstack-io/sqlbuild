"""Concurrent explicit SQLite subscriptions to CLI lifecycle publication."""

from __future__ import annotations

import logging
from collections.abc import Iterator
from concurrent.futures import Future, ThreadPoolExecutor
from contextvars import Context, copy_context
from pathlib import Path
from threading import Event, Thread
from typing import Any, cast

import pytest

from sqlbuild._native import CliNamespace, cli_observability_scope
from sqlbuild.execution_history import (
    CanonicalLifecycleEvent,
    EventFilter,
    EventPage,
    RunRecord,
    RunStatus,
)
from sqlbuild.observability import (
    LifecycleEvent,
    ResourceAttemptLifecycle,
    Unsubscribe,
    create_lifecycle_event,
    invocation_scope,
    run_scope,
)
from sqlbuild.runtime.execution_history.exceptions import ExecutionHistoryStorageError
from sqlbuild.sqlite_history import SQLiteExecutionHistory
from tests.integration.src.sqlbuild.runtime.execution_history._test_types import (
    ConcurrentSQLiteHistoryCase,
)
from tests.integration.src.sqlbuild.runtime.execution_history.helpers import lifecycle_event


@pytest.mark.parametrize(
    "test_case",
    (
        ConcurrentSQLiteHistoryCase(
            description="two copied worker contexts persist complete resource attempts",
            expected_resource_start_count=2,
            expected_resource_terminal_count=2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_explicit_cli_history_when_two_workers_publish_then_events_and_projection_persist(
    tmp_path: Path,
    caplog: pytest.LogCaptureFixture,
    test_case: ConcurrentSQLiteHistoryCase,
) -> None:
    caplog.set_level(logging.DEBUG)
    args: CliNamespace = CliNamespace()
    history: SQLiteExecutionHistory = SQLiteExecutionHistory(project_dir=tmp_path)

    def execute_attempt(resource_name: str) -> None:
        with ResourceAttemptLifecycle(
            resource_id=f"asset:{resource_name}",
            resource_kind="task",
            resource_name=resource_name,
            run_id="concurrent-run",
        ):
            pass

    def persist_event(event: CanonicalLifecycleEvent) -> None:
        _ = history.append_and_project((event,))

    with invocation_scope("concurrent-invocation"):
        with cli_observability_scope(args=args, project_dir=tmp_path) as dispatcher:
            unsubscribe: Unsubscribe = dispatcher.subscribe_lifecycle(
                subscriber=persist_event,
                accepts_opaque=True,
            )
            try:
                with run_scope("concurrent-run"):
                    dispatcher.publish_lifecycle(
                        create_lifecycle_event(
                            event_type="run_started",
                            payload={"run_kind": "build", "selected_count": 2},
                        )
                    )
                    contexts: tuple[Context, Context] = (copy_context(), copy_context())
                    with ThreadPoolExecutor(max_workers=2) as pool:
                        first: Future[Any] = pool.submit(contexts[0].run, execute_attempt, "first")
                        second: Future[Any] = pool.submit(
                            contexts[1].run, execute_attempt, "second"
                        )
                        first.result()
                        second.result()
                    dispatcher.publish_lifecycle(
                        create_lifecycle_event(
                            event_type="run_completed",
                            payload={
                                "run_kind": "build",
                                "succeeded_count": 2,
                                "failed_count": 0,
                                "skipped_count": 0,
                                "duration_ms": 1,
                            },
                        )
                    )
            finally:
                unsubscribe()

    page: EventPage = history.get_events(event_filter=EventFilter(run_id="concurrent-run"))
    run: RunRecord | None = history.get_run("concurrent-run")
    history.close()
    events: tuple[LifecycleEvent, ...] = tuple(
        cast(LifecycleEvent, record.event) for record in page.records
    )
    resource_events: tuple[LifecycleEvent, ...] = tuple(
        filter(lambda event: event.event_type.startswith("resource_attempt_"), events)
    )

    assert sum(event.event_type.endswith("started") for event in resource_events) == (
        test_case.expected_resource_start_count
    )
    assert sum(event.event_type.endswith(("completed", "failed")) for event in resource_events) == (
        test_case.expected_resource_terminal_count
    )
    assert tuple(event.run_id for event in resource_events) == ("concurrent-run",) * 4
    assert run is not None
    assert run.status == RunStatus.COMPLETED
    assert not tuple(
        filter(lambda record: "Lifecycle event subscriber failed" in record.message, caplog.records)
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ConcurrentSQLiteHistoryCase(
            description="close waits for in-flight append transaction",
            expected_resource_start_count=1,
            expected_resource_terminal_count=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_shared_history_when_append_and_close_overlap_then_close_waits_and_is_idempotent(
    tmp_path: Path,
    test_case: ConcurrentSQLiteHistoryCase,
) -> None:
    history: SQLiteExecutionHistory = SQLiteExecutionHistory(project_dir=tmp_path)
    append_entered: Event = Event()
    append_release: Event = Event()
    close_done: Event = Event()
    event: LifecycleEvent = lifecycle_event("concurrent-close", run_id="close-run")

    def blocking_events() -> Iterator[LifecycleEvent]:
        append_entered.set()
        assert append_release.wait(timeout=5)
        yield event

    append_thread: Thread = Thread(target=lambda: history.append_events(blocking_events()))
    close_thread: Thread = Thread(target=lambda: (history.close(), close_done.set()))
    append_thread.start()
    assert append_entered.wait(timeout=5)
    close_thread.start()
    assert not close_done.wait(timeout=0.05)
    append_release.set()
    append_thread.join(timeout=5)
    close_thread.join(timeout=5)
    history.close()

    assert close_done.is_set()
    assert test_case.expected_resource_start_count == 1
    assert test_case.expected_resource_terminal_count == 0
    with pytest.raises(ExecutionHistoryStorageError, match="closed"):
        history.get_events(event_filter=EventFilter())
