from __future__ import annotations

from typing import cast

import pytest

from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    OperationLifecycle,
    ResourceAttemptLifecycle,
    current_event_dispatcher,
    current_execution_identity,
    dispatcher_scope,
    invocation_scope,
    resource_attempt_scope,
    run_scope,
)
from sqlbuild.runtime.observability.exceptions import ObservabilityValidationError
from sqlbuild.runtime.observability.main.is_terminal_event import is_terminal_event
from sqlbuild.runtime.observability.models import ExecutionIdentity
from tests.unit.src.sqlbuild.runtime.observability._test_types import OperationLifecycleCase


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="failed returned resource result",
            expected_event_types=("resource_attempt_started", "resource_attempt_failed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_returned_failure_when_attempt_marked_failed_then_exact_terminal_is_published(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv"), run_scope("run"), dispatcher_scope(dispatcher):
        with ResourceAttemptLifecycle(
            resource_id="model:orders",
            resource_kind="table",
            resource_name="orders",
        ) as lifecycle:
            lifecycle.failed(error_code="SQB-101")

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert events[0].resource_attempt_id == events[1].resource_attempt_id
    assert events[1].payload["error_code"] == "SQB-101"
    assert cast(float, events[1].payload["duration_ms"]) >= 0


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="explicit hard skip terminal",
            expected_event_types=("resource_attempt_started", "resource_attempt_skipped"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_bounded_skip_when_attempt_marked_skipped_then_reason_is_not_canonical(
    test_case: OperationLifecycleCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv-skip"), run_scope("run-skip"), dispatcher_scope(dispatcher):
        with ResourceAttemptLifecycle(
            resource_id="asset:orders",
            resource_kind="asset",
            resource_name="orders",
        ) as lifecycle:
            lifecycle.skipped(skip_code="explicit", skip_mode="hard")

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert events[1].payload["skip_code"] == "explicit"
    assert events[1].payload["skip_mode"] == "hard"
    assert "reason" not in events[1].payload
    assert is_terminal_event(events[1])


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="direct use installs and restores transient contexts",
            expected_event_types=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_no_context_when_attempt_runs_then_transient_identity_and_dispatcher_are_restored(
    test_case: OperationLifecycleCase,
) -> None:
    assert test_case.expected_event_types == ()
    assert current_execution_identity() is None
    assert current_event_dispatcher() is None

    with ResourceAttemptLifecycle(
        resource_id="asset:direct",
        resource_kind="asset",
        resource_name="direct",
        run_id="direct-run",
    ):
        identity: ExecutionIdentity | None = current_execution_identity()
        assert identity is not None
        assert identity.run_id == "direct-run"
        assert identity.resource_id == "asset:direct"
        assert current_event_dispatcher() is not None
        with OperationLifecycle(operation_kind="python_node", operation_name="python_asset"):
            nested: ExecutionIdentity | None = current_execution_identity()
            assert nested is not None
            assert nested.resource_attempt_id == identity.resource_attempt_id
            assert nested.operation_id is not None
        assert current_execution_identity() == identity

    assert current_execution_identity() is None
    assert current_event_dispatcher() is None


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="transient start failure restores empty context",
            expected_event_types=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_direct_attempt_when_start_fails_then_transient_context_is_restored(
    test_case: OperationLifecycleCase,
) -> None:
    assert test_case.expected_event_types == ()

    with pytest.raises(ObservabilityValidationError):
        with ResourceAttemptLifecycle(
            resource_id="asset:invalid",
            resource_kind="asset",
            resource_name="invalid",
            attempt_number=-1,
        ):
            pass

    assert current_execution_identity() is None
    assert current_event_dispatcher() is None


@pytest.mark.parametrize(
    "test_case",
    (
        OperationLifecycleCase(
            description="nested start failure restores parent context",
            expected_event_types=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_parent_context_when_nested_start_fails_then_exact_parent_is_restored(
    test_case: OperationLifecycleCase,
) -> None:
    dispatcher: EventDispatcher = EventDispatcher()
    assert test_case.expected_event_types == ()

    with (
        invocation_scope("parent-inv"),
        run_scope("parent-run"),
        resource_attempt_scope(resource_id="asset:parent", resource_attempt_id="parent-attempt"),
        dispatcher_scope(dispatcher),
    ):
        parent: ExecutionIdentity | None = current_execution_identity()
        with pytest.raises(ObservabilityValidationError):
            with ResourceAttemptLifecycle(
                resource_id="asset:invalid",
                resource_kind="asset",
                resource_name="invalid",
                attempt_number=-1,
            ):
                pass
        assert current_execution_identity() == parent
        assert current_event_dispatcher() is dispatcher
