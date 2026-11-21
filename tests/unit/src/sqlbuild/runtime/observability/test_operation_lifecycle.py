from __future__ import annotations

from concurrent.futures import CancelledError, Future
from contextvars import Context, copy_context
from threading import Event, Thread
from typing import Any, cast

import pytest

from sqlbuild.observability import (
    EventDispatcher,
    ExecutionIdentity,
    LifecycleEvent,
    ObservabilityValidationError,
    OperationAttributes,
    OperationLifecycle,
    current_event_dispatcher,
    current_execution_identity,
    dispatcher_scope,
    invocation_scope,
    operation_scope,
    resource_attempt_scope,
    run_scope,
)
from sqlbuild.runtime.observability.classes.statement_lifecycle import StatementLifecycle
from tests.unit.src.sqlbuild.runtime.observability._test_types import OperationLifecycleCase


class _FailingSubscriber:
    def __call__(self, event: LifecycleEvent) -> None:
        del event
        raise SystemExit("subscriber secret")


class _ObservedContextManager:
    def __init__(self, events: list[LifecycleEvent]) -> None:
        self._events: list[LifecycleEvent] = events

    def __enter__(self) -> str:
        assert self._events[-1].event_type == "operation_started"
        return "context-value"

    def __exit__(self, *args: object) -> None:
        assert tuple(event.event_type for event in self._events) == ("operation_started",)


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="terminal strategy resolved after inspection",
            expected_event_types=("operation_started", "operation_completed"),
            operation_kind="warehouse",
            operation_name="relation_promotion",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_terminal_attributes_when_completing_then_start_remains_unchanged(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with dispatcher_scope(dispatcher):
        with OperationLifecycle(
            operation_kind=test_case.operation_kind,
            operation_name=test_case.operation_name,
            attributes=OperationAttributes(phase="promote", target_kind="relation"),
        ) as lifecycle:
            lifecycle.completed(attributes=OperationAttributes(strategy="rename"))

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert "strategy" not in events[0].payload
    assert events[1].payload["strategy"] == "rename"


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="clone physical transfer barrier",
            expected_event_types=("operation_started", "operation_completed"),
            operation_kind="clone",
            operation_name="clone_relation_transfer",
        ),
        OperationLifecycleCase(
            description="janitor physical cleanup barrier",
            expected_event_types=("operation_started", "operation_completed"),
            operation_kind="janitor",
            operation_name="janitor_cleanup_action",
        ),
        OperationLifecycleCase(
            description="scenario snapshot write barrier",
            expected_event_types=("operation_started", "operation_completed"),
            operation_kind="scenario",
            operation_name="scenario_snapshot_write",
        ),
        OperationLifecycleCase(
            description="discovery Python import barrier",
            expected_event_types=("operation_started", "operation_completed"),
            operation_kind="project",
            operation_name="discovery_python_import",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_blocking_callable_when_run_then_start_precedes_block_and_terminal_follows(
    test_case: OperationLifecycleCase,
) -> None:
    entered: Event = Event()
    release: Event = Event()
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    def block() -> str:
        entered.set()
        assert release.wait(timeout=5)
        return "exact-result"

    result: list[str] = []
    with invocation_scope("inv-callable"), dispatcher_scope(dispatcher):
        context: Context = copy_context()
        lifecycle: OperationLifecycle = OperationLifecycle(
            operation_kind=test_case.operation_kind,
            operation_name=test_case.operation_name,
        )
        thread: Thread = Thread(
            target=lambda: result.append(cast(str, context.run(lifecycle.run, block)))
        )
        thread.start()
        assert entered.wait(timeout=5)
        assert tuple(event.event_type for event in events) == ("operation_started",)
        release.set()
        thread.join(timeout=5)

    assert result == ["exact-result"]
    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert cast(float, events[-1].payload["duration_ms"]) >= 0


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="successful iterator consumption",
            expected_event_types=("operation_started", "operation_completed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_iterator_when_consumed_then_terminal_follows_exhaustion(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    def values() -> Any:
        assert events[-1].event_type == "operation_started"
        yield 1
        assert tuple(event.event_type for event in events) == ("operation_started",)
        yield 2

    with invocation_scope("inv-iterator"), dispatcher_scope(dispatcher):
        result: tuple[int, ...] = OperationLifecycle(
            operation_kind="scenario", operation_name="scenario_capture"
        ).consume(values())

    assert result == (1, 2)
    assert tuple(event.event_type for event in events) == test_case.expected_event_types


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="context manager acquisition and use",
            expected_event_types=("operation_started", "operation_completed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_context_manager_when_operation_owns_use_then_terminal_follows_exit(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv-context"), dispatcher_scope(dispatcher):
        with OperationLifecycle(operation_kind="project", operation_name="project_compile"):
            with _ObservedContextManager(events) as value:
                assert value == "context-value"

    assert tuple(event.event_type for event in events) == test_case.expected_event_types


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="successful future result",
            expected_event_types=("operation_started", "operation_completed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_pending_future_when_result_owned_then_terminal_follows_result(
    test_case: OperationLifecycleCase,
) -> None:
    future: Future[str] = Future()
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv-future"), dispatcher_scope(dispatcher):
        context: Context = copy_context()
        result: list[str] = []
        lifecycle: OperationLifecycle = OperationLifecycle(
            operation_kind="python_node", operation_name="python_asset"
        )
        thread: Thread = Thread(
            target=lambda: result.append(cast(str, context.run(lifecycle.result, future)))
        )
        thread.start()
        while not events:
            thread.join(timeout=0.001)
        assert tuple(event.event_type for event in events) == ("operation_started",)
        future.set_result("future-result")
        thread.join(timeout=5)

    assert result == ["future-result"]
    assert tuple(event.event_type for event in events) == test_case.expected_event_types


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="cancelled future result",
            expected_event_types=("operation_started", "operation_failed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_cancelled_future_when_result_owned_then_one_failure_is_published(
    test_case: OperationLifecycleCase,
) -> None:
    future: Future[str] = Future()
    _ = future.cancel()
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with (
        invocation_scope("inv-cancelled-future"),
        dispatcher_scope(dispatcher),
        pytest.raises(CancelledError),
    ):
        _ = OperationLifecycle(operation_kind="python_node", operation_name="python_asset").result(
            future
        )

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert events[-1].payload["error_type"] == "CancelledError"


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="base exception failure",
            expected_event_types=("operation_started", "operation_failed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_base_exception_when_operation_exits_then_one_safe_failure_is_published(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with (
        invocation_scope("inv-system-exit"),
        dispatcher_scope(dispatcher),
        pytest.raises(SystemExit, match="private exit message"),
    ):
        with OperationLifecycle(operation_kind="subprocess", operation_name="dbt_command"):
            raise SystemExit("private exit message")

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert events[-1].payload["error_type"] == "SystemExit"
    assert "private exit message" not in repr(events)


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="subscriber-isolated operation",
            expected_event_types=("operation_started", "operation_completed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_failing_subscriber_when_operation_runs_then_exact_result_is_preserved(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=_FailingSubscriber(), accepts_opaque=False)
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    expected: object = object()

    with invocation_scope("inv-subscriber"), dispatcher_scope(dispatcher):
        result: object = OperationLifecycle(
            operation_kind="loader", operation_name="managed_source_load"
        ).run(lambda: expected)

    assert result is expected
    assert tuple(event.event_type for event in events) == test_case.expected_event_types


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="nested operation restoration",
            expected_event_types=("operation_started", "operation_completed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_nested_operations_when_exiting_then_parent_and_siblings_do_not_leak(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with (
        invocation_scope("inv-nested"),
        run_scope("run-nested"),
        resource_attempt_scope(resource_id="resource", resource_attempt_id="attempt"),
        dispatcher_scope(dispatcher),
        operation_scope("parent-operation"),
    ):
        with OperationLifecycle(
            operation_kind="python_node",
            operation_name="python_asset",
            operation_id="child-one",
        ):
            with StatementLifecycle(adapter="duckdb", sql="SELECT 1", intent="execute"):
                pass
        identity: ExecutionIdentity | None = current_execution_identity()
        assert identity is not None
        assert identity.operation_id == "parent-operation"
        with OperationLifecycle(
            operation_kind="python_node",
            operation_name="python_asset",
            operation_id="child-two",
        ):
            sibling_identity: ExecutionIdentity | None = current_execution_identity()
            assert sibling_identity is not None
            assert sibling_identity.operation_id == "child-two"

    statement_events: tuple[LifecycleEvent, ...] = tuple(
        filter(lambda event: event.event_type.startswith("statement_"), events)
    )
    assert {event.operation_id for event in statement_events} == {"child-one"}
    assert tuple(event.event_type for event in events[-2:]) == test_case.expected_event_types
    assert all(event.run_id == "run-nested" for event in events)
    assert all(event.resource_attempt_id == "attempt" for event in events)
    assert current_execution_identity() is None
    assert current_event_dispatcher() is None


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="direct transient operation",
            expected_event_types=("operation_started", "operation_completed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_direct_programmatic_use_when_complete_then_transient_context_is_restored(
    test_case: OperationLifecycleCase,
) -> None:
    result: str = OperationLifecycle(
        operation_kind="project", operation_name="project_discovery"
    ).run(lambda: "direct-result")

    assert result == "direct-result"
    assert test_case.expected_event_types[-1] == "operation_completed"
    assert current_execution_identity() is None
    assert current_event_dispatcher() is None


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="start factory failure without ambient context",
            expected_event_types=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_start_factory_base_exception_when_entering_then_transient_scopes_are_restored(
    test_case: OperationLifecycleCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    lifecycle: OperationLifecycle = OperationLifecycle(
        operation_kind="project", operation_name="project_compile"
    )
    monkeypatch.setattr(
        "sqlbuild.runtime.observability.classes.operation_lifecycle.create_lifecycle_event",
        lambda **kwargs: (_ for _ in ()).throw(SystemExit("start factory failure")),
    )

    with pytest.raises(SystemExit, match="start factory failure"):
        _ = lifecycle.__enter__()

    assert test_case.expected_event_types == ()
    assert current_execution_identity() is None
    assert current_event_dispatcher() is None
    with pytest.raises(ObservabilityValidationError, match="cannot be entered more than once"):
        _ = lifecycle.__enter__()


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="start publication failure with ambient context",
            expected_event_types=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_start_publication_base_exception_when_nested_then_parent_context_is_restored(
    test_case: OperationLifecycleCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    dispatcher: EventDispatcher = EventDispatcher()

    with invocation_scope("inv-parent"), dispatcher_scope(dispatcher):
        parent_identity: ExecutionIdentity | None = current_execution_identity()
        monkeypatch.setattr(
            OperationLifecycle,
            "_publish",
            lambda *args, **kwargs: (_ for _ in ()).throw(SystemExit("start publication failure")),
        )
        with pytest.raises(SystemExit, match="start publication failure"):
            with OperationLifecycle(operation_kind="project", operation_name="project_compile"):
                pass
        assert test_case.expected_event_types == ()
        assert current_execution_identity() is parent_identity
        assert current_event_dispatcher() is dispatcher


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="terminal row and byte counts become immutable completion facts",
            expected_event_types=("operation_started", "operation_completed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_terminal_counts_when_operation_completes_then_counts_are_terminal_only(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv-counts"), dispatcher_scope(dispatcher):
        with OperationLifecycle(
            operation_kind="scenario", operation_name="scenario_snapshot_write"
        ) as lifecycle:
            lifecycle.completed(metadata={"row_count": 2, "byte_count": 17})

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert "metadata" not in events[0].payload
    assert events[1].payload["metadata"] == {"row_count": 2, "byte_count": 17}


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="observed subprocess signal metadata",
            expected_event_types=("operation_started", "operation_failed"),
            operation_kind="subprocess",
            operation_name="dbt_command",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_observed_signal_when_subprocess_fails_then_terminal_has_bounded_exit_metadata(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv-signal"), dispatcher_scope(dispatcher):
        with OperationLifecycle(
            operation_kind=test_case.operation_kind, operation_name=test_case.operation_name
        ) as lifecycle:
            lifecycle.failed(
                error_code="exit_-15",
                exit_code=-15,
                process_id=123,
                signal_number=15,
            )

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert events[1].payload["exit_code"] == -15
    assert events[1].payload["process_id"] == 123
    assert events[1].payload["signal_number"] == 15
    assert events[1].payload["error_code"] == "exit_-15"


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="unobserved subprocess interruption",
            expected_event_types=("operation_started",),
            operation_kind="subprocess",
            operation_name="dbt_command",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unobserved_base_exception_when_operation_opts_out_then_terminal_remains_missing(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with (
        invocation_scope("inv-interrupted-child"),
        dispatcher_scope(dispatcher),
        pytest.raises(KeyboardInterrupt),
    ):
        with OperationLifecycle(
            operation_kind=test_case.operation_kind,
            operation_name=test_case.operation_name,
            auto_fail_base_exceptions=False,
        ):
            raise KeyboardInterrupt

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
