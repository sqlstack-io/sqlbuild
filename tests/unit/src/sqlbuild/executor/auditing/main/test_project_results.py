"""Completed audit projection tests."""

from __future__ import annotations

from dataclasses import replace
from typing import Any, cast
from unittest.mock import Mock

import pytest

import sqlbuild.executor.auditing._helpers.result_projection as projection_module
from sqlbuild._native import CompiledRelationLocation, PlanOutput
from sqlbuild.compiler.auditing.types import AuditOutcome
from sqlbuild.executor.audit_results.exceptions import AuditResultStorageError
from sqlbuild.executor.audit_results.models import AuditResultRecord
from sqlbuild.executor.auditing.main._project_results import project_audit_result_batch
from sqlbuild.executor.auditing.main.audit_result_publication_scope import (
    audit_result_publication_scope,
)
from sqlbuild.executor.auditing.main.publish_completed_audit_results import (
    publish_completed_audit_results,
)
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.runtime.observability.classes.event_dispatcher import EventDispatcher
from sqlbuild.runtime.observability.exceptions import ObservabilityValidationError
from sqlbuild.runtime.observability.main.dispatcher_scope import dispatcher_scope
from sqlbuild.runtime.observability.main.identity_scope import identity_scope
from sqlbuild.runtime.observability.models import ExecutionIdentity, LifecycleEvent
from tests.unit.src.sqlbuild.executor.auditing.main._test_types import (
    AuditExecutionCase,
    IgnoredFailureProjectionCase,
)
from tests.unit.src.sqlbuild.executor.auditing.main.helpers import (
    build_projection_entry,
    build_projection_result,
    writer_adapter,
)


@pytest.mark.parametrize(
    "test_case",
    [AuditExecutionCase("executed projection", AuditOutcome.WARN)],
    ids=lambda case: case.description,
)
def test_given_executed_measurement_when_projected_then_builds_record_and_lifecycle_event(
    test_case: AuditExecutionCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    records: list[AuditResultRecord] = []

    def write_records(**kwargs: object) -> None:
        records.extend(cast(tuple[AuditResultRecord, ...], kwargs["records"]))

    monkeypatch.setattr(projection_module, "write_audit_result_records", write_records)
    dispatcher: EventDispatcher = EventDispatcher()
    events: list[LifecycleEvent] = []
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    plan: PlanOutput = PlanOutput(
        audit_entries=(build_projection_entry(),),
        model_locations={
            "orders": CompiledRelationLocation(
                database="warehouse", schema="analytics", name="orders", qualified_name=None
            )
        },
    )
    # Writer is replaced, so only method attributes passed into it are required.
    adapter: Any = writer_adapter()

    with (
        identity_scope(ExecutionIdentity(invocation_id="invocation", run_id="run")),
        dispatcher_scope(dispatcher),
    ):
        projection: Any = project_audit_result_batch(
            plan=plan,
            results=(build_projection_result(),),
            adapter=adapter,
            connection=object(),
        )

    assert projection.written_count == 1
    assert projection.degraded is False
    assert records[0].outcome == test_case.expected_outcome.value
    assert records[0].audit_definition_name == "dq_column_rate"
    assert records[0].audit_description == "Valid row percentage"
    assert len(records) == 1
    assert records[0].violation_count is None
    assert records[0].measured_value == 95.0
    assert records[0].thresholds_json == (
        '{"error":{"limit":90.0,"operator":"below"},"warn":{"limit":100.0,"operator":"below"}}'
    )
    assert records[0].evidence_json == '[{"order_id":1}]'
    assert len(events) == 1
    assert events[0].event_type == "audit_completed"
    assert events[0].payload["result_id"] == records[0].result_id
    assert events[0].payload["audit_definition_name"] == "dq_column_rate"
    assert events[0].payload["audit_description"] == "Valid row percentage"


@pytest.mark.parametrize(
    "test_case",
    [
        IgnoredFailureProjectionCase(
            description="an ignored failure is recorded as a failure marked ignored",
            ignored=True,
            expected_metadata_json='{"ignored": true}',
            expected_payload_ignored=True,
        ),
        IgnoredFailureProjectionCase(
            description="a failure that blocked carries no ignored mark",
            ignored=False,
            expected_metadata_json=None,
            expected_payload_ignored=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failed_audit_when_projected_then_history_and_event_say_whether_it_was_ignored(
    test_case: IgnoredFailureProjectionCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    records: list[AuditResultRecord] = []

    def write_records(**kwargs: object) -> None:
        records.extend(cast(tuple[AuditResultRecord, ...], kwargs["records"]))

    monkeypatch.setattr(projection_module, "write_audit_result_records", write_records)
    dispatcher: EventDispatcher = EventDispatcher()
    events: list[LifecycleEvent] = []
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    plan: PlanOutput = PlanOutput(
        audit_entries=(build_projection_entry(),),
        model_locations={
            "orders": CompiledRelationLocation(
                database="warehouse", schema="analytics", name="orders", qualified_name=None
            )
        },
    )
    # Writer is replaced, so only method attributes passed into it are required.
    adapter: Any = writer_adapter()

    with (
        identity_scope(ExecutionIdentity(invocation_id="invocation", run_id="run")),
        dispatcher_scope(dispatcher),
    ):
        _ = project_audit_result_batch(
            plan=plan,
            results=(
                replace(
                    build_projection_result(),
                    outcome=AuditOutcome.ERROR,
                    ignored=test_case.ignored,
                ),
            ),
            adapter=adapter,
            connection=object(),
        )

    assert [(record.outcome, record.metadata_json) for record in records] == [
        (AuditOutcome.ERROR.value, test_case.expected_metadata_json)
    ]
    assert [event.payload.get("ignored") for event in events] == [
        test_case.expected_payload_ignored
    ]


@pytest.mark.parametrize(
    "test_case",
    [AuditExecutionCase("reused projection", AuditOutcome.WARN)],
    ids=lambda case: case.description,
)
def test_given_reused_measurement_when_projected_then_skips_history_and_event(
    test_case: AuditExecutionCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    writer_calls: list[dict[str, Any]] = []
    monkeypatch.setattr(
        projection_module,
        "write_audit_result_records",
        lambda **kwargs: writer_calls.append(kwargs),
    )
    dispatcher: EventDispatcher = EventDispatcher()
    events: list[LifecycleEvent] = []
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    plan: PlanOutput = PlanOutput(audit_entries=(build_projection_entry(),))

    with (
        identity_scope(ExecutionIdentity(invocation_id="invocation", run_id="run")),
        dispatcher_scope(dispatcher),
    ):
        projection: Any = project_audit_result_batch(
            plan=plan,
            results=(replace(build_projection_result(), reused=True),),
            adapter=writer_adapter(),
            connection=object(),
            storage_schema="analytics",
        )

    assert projection.attempted_count == 0
    assert build_projection_result().outcome == test_case.expected_outcome
    assert writer_calls == []
    assert events == []


@pytest.mark.parametrize(
    "test_case",
    [AuditExecutionCase("failed projection", AuditOutcome.WARN)],
    ids=lambda case: case.description,
)
def test_given_history_storage_failure_when_projected_then_reports_degradation_without_raising(
    test_case: AuditExecutionCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fail_write(**kwargs: object) -> None:
        del kwargs
        raise AuditResultStorageError("unavailable")

    monkeypatch.setattr(
        projection_module,
        "write_audit_result_records",
        fail_write,
    )
    adapter: Any = writer_adapter()
    plan: PlanOutput = PlanOutput(
        audit_entries=(build_projection_entry(),),
        model_locations={
            "orders": CompiledRelationLocation(
                database=None, schema="analytics", name="orders", qualified_name=None
            )
        },
    )

    with identity_scope(ExecutionIdentity(invocation_id="invocation", run_id="run")):
        projection: Any = project_audit_result_batch(
            plan=plan,
            results=(build_projection_result(),),
            adapter=adapter,
            connection=object(),
        )

    assert projection.degraded is True
    assert projection.attempted_count == 1
    assert projection.written_count == 0
    assert projection.failed_count == 1
    assert build_projection_result().outcome == test_case.expected_outcome


@pytest.mark.parametrize(
    "test_case",
    [AuditExecutionCase("record build failure", AuditOutcome.WARN)],
    ids=lambda case: case.description,
)
def test_given_record_build_failure_when_projected_then_reports_degradation_without_raising(
    test_case: AuditExecutionCase,
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
) -> None:
    def fail_record_build(**kwargs: object) -> AuditResultRecord:
        del kwargs
        raise RuntimeError("invalid record")

    monkeypatch.setattr(projection_module, "_build_record", fail_record_build)

    with identity_scope(ExecutionIdentity(invocation_id="invocation", run_id="run")):
        projection: Any = project_audit_result_batch(
            plan=PlanOutput(audit_entries=(build_projection_entry(),)),
            results=(build_projection_result(),),
            adapter=writer_adapter(),
            connection=object(),
            storage_schema="analytics",
        )

    assert projection.attempted_count == 1
    assert projection.written_count == 0
    assert projection.failed_count == 1
    assert projection.degraded is True
    assert "Audit result projection degraded" in caplog.text
    assert build_projection_result().outcome == test_case.expected_outcome


@pytest.mark.parametrize(
    "test_case",
    [AuditExecutionCase("unmatched result batch", AuditOutcome.WARN)],
    ids=lambda case: case.description,
)
def test_given_one_unmatched_result_when_projecting_batch_then_publishes_valid_results(
    test_case: AuditExecutionCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    writer_calls: list[dict[str, Any]] = []
    monkeypatch.setattr(
        projection_module,
        "write_audit_result_records",
        lambda **kwargs: writer_calls.append(kwargs),
    )
    dispatcher: EventDispatcher = EventDispatcher()
    events: list[LifecycleEvent] = []
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with (
        identity_scope(ExecutionIdentity(invocation_id="invocation", run_id="run")),
        dispatcher_scope(dispatcher),
    ):
        projection: Any = project_audit_result_batch(
            plan=PlanOutput(audit_entries=(build_projection_entry(),)),
            results=(
                build_projection_result(),
                replace(build_projection_result(), audit_name="unplanned_audit"),
            ),
            adapter=writer_adapter(),
            connection=object(),
            storage_schema="analytics",
        )

    assert projection.attempted_count == 2
    assert projection.written_count == 1
    assert projection.failed_count == 1
    assert len(events) == 1
    assert events[0].payload["audit_name"] == "row_rate"
    assert len(writer_calls) == 1
    assert len(writer_calls[0]["records"]) == 1
    assert build_projection_result().outcome == test_case.expected_outcome


@pytest.mark.parametrize(
    "test_case",
    [AuditExecutionCase("lifecycle validation failure", AuditOutcome.WARN)],
    ids=lambda case: case.description,
)
def test_given_lifecycle_validation_failure_when_projected_then_continues_and_persists_records(
    test_case: AuditExecutionCase,
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
) -> None:
    writer_calls: list[dict[str, Any]] = []
    lifecycle_publisher: Mock = Mock(
        side_effect=(ObservabilityValidationError("invalid lifecycle payload"), None)
    )

    monkeypatch.setattr(projection_module, "_publish_audit_completed", lifecycle_publisher)
    monkeypatch.setattr(
        projection_module,
        "write_audit_result_records",
        lambda **kwargs: writer_calls.append(kwargs),
    )

    with identity_scope(ExecutionIdentity(invocation_id="invocation", run_id="run")):
        projection: Any = project_audit_result_batch(
            plan=PlanOutput(audit_entries=(build_projection_entry(),)),
            results=(build_projection_result(), build_projection_result()),
            adapter=writer_adapter(),
            connection=object(),
            storage_schema="analytics",
        )

    assert projection.attempted_count == 2
    assert projection.written_count == 2
    assert projection.failed_count == 0
    assert projection.degraded is False
    assert len(writer_calls) == 1
    assert len(writer_calls[0]["records"]) == 2
    assert lifecycle_publisher.call_count == 2
    assert "Audit result lifecycle publication degraded" in caplog.text
    assert build_projection_result().outcome == test_case.expected_outcome


@pytest.mark.parametrize(
    "test_case",
    [AuditExecutionCase("incremental publication", AuditOutcome.WARN)],
    ids=lambda case: case.description,
)
def test_given_publication_scope_when_audits_complete_then_events_precede_batch_without_duplicates(
    test_case: AuditExecutionCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    writer_calls: list[dict[str, Any]] = []
    monkeypatch.setattr(
        projection_module,
        "write_audit_result_records",
        lambda **kwargs: writer_calls.append(kwargs),
    )
    dispatcher: EventDispatcher = EventDispatcher()
    events: list[LifecycleEvent] = []
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    plan: PlanOutput = PlanOutput(audit_entries=(build_projection_entry(),))
    early: AuditExecutionResult = build_projection_result()
    late: AuditExecutionResult = build_projection_result()

    with (
        identity_scope(ExecutionIdentity(invocation_id="invocation", run_id="run")),
        dispatcher_scope(dispatcher),
        audit_result_publication_scope(plan=plan, storage_schema="analytics"),
    ):
        publish_completed_audit_results((early,))
        publish_completed_audit_results((early,))
        events_before_batch: int = len(events)
        projection: Any = project_audit_result_batch(
            plan=plan,
            results=(early, late),
            adapter=writer_adapter(),
            connection=object(),
            storage_schema="analytics",
        )

    records: tuple[AuditResultRecord, ...] = writer_calls[0]["records"]
    assert events_before_batch == 1
    assert len(events) == 2
    assert [event.payload["result_id"] for event in events] == [
        record.result_id for record in records
    ]
    assert records[0].result_id != records[1].result_id
    assert projection.written_count == 2
    assert early.outcome == test_case.expected_outcome


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
