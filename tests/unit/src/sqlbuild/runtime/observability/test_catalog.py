"""Tests for lifecycle catalog validation and semantic helpers."""

from dataclasses import replace
from datetime import datetime, timedelta, timezone

import pytest

from sqlbuild._native import (
    LIFECYCLE_EVENT_SCHEMA_VERSIONS,
    lifecycle_event_catalogued,
    lifecycle_event_terminal,
)
from sqlbuild.observability import create_lifecycle_event, invocation_scope, run_scope
from sqlbuild.runtime.observability.classes.run_lifecycle import RunLifecycle
from sqlbuild.runtime.observability.exceptions import ObservabilityValidationError
from sqlbuild.runtime.observability.main.canonicalize_operation_adapter import (
    canonicalize_operation_adapter,
)
from sqlbuild.runtime.observability.main.is_terminal_event import is_terminal_event
from sqlbuild.runtime.observability.main.validate_idempotent_duplicate import (
    validate_idempotent_duplicate,
)
from sqlbuild.runtime.observability.models import LifecycleEvent
from sqlbuild.runtime.observability.types import JSONValue
from tests.unit.src.sqlbuild.runtime.observability._test_types import (
    CatalogEventPresenceCase,
    CatalogVersionCase,
    IdempotencyCase,
    LifecycleErrorCase,
    OperationAdapterCase,
    RunLifecycleErrorCase,
    StatementPrivacyCase,
    TerminalEvidenceCase,
    TerminalSemanticsCase,
    TimestampErrorCase,
)
from tests.unit.src.sqlbuild.runtime.observability.helpers import lifecycle_event


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleErrorCase(
            description="complete measurement audit payload",
            event_type="audit_completed",
            payload={},
            expected_error="audit_completed",
            run_id="run-1",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_complete_audit_payload_when_creating_lifecycle_event_then_completed_fact_validates(
    test_case: LifecycleErrorCase,
) -> None:
    payload: dict[str, JSONValue] = {
        "audit_name": "valid_order_rate",
        "audit_definition_name": "dq_column_rate",
        "evaluation_mode": "measurement",
        "outcome": "warn",
        "severity": "warn",
        "run_scope_phase": "final",
        "attachment_kind": "model",
        "binding_key": "orders.valid_order_rate",
        "definition_fingerprint": "definition-123",
        "execution_fingerprint": "execution-456",
        "attached_target_kind": "model",
        "attached_target_name": "orders",
        "target_database": "analytics",
        "target_schema": "quality",
        "target_name": "orders",
        "measured_value": 99.5,
        "sample_count": 100,
        "sample_unit": "rows",
        "minimum_samples": 50,
        "thresholds": {"warn": {"below": 100}, "error": {"below": 99}},
        "evidence": [{"order_id": 42}],
        "evidence_count": 1,
        "evidence_truncated": False,
        "measurement_sql": "SELECT 99.5 AS valid_rate, 100 AS total_rows",
        "executed_sql": "SELECT 99.5 AS valid_rate, 100 AS total_rows",
        "sql_digest": "digest-789",
        "reused": False,
        "result_id": "result-123",
    }

    with invocation_scope("invocation-1"), run_scope(test_case.run_id or "run-1"):
        event: LifecycleEvent = create_lifecycle_event(
            event_type=test_case.event_type, payload=payload
        )

    assert event.event_type == test_case.expected_error
    assert event.run_id == "run-1"
    assert event.payload["audit_name"] == payload["audit_name"]
    assert event.payload["thresholds"] == payload["thresholds"]
    assert event.payload["evidence_count"] == payload["evidence_count"]
    assert is_terminal_event(event) is True


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleErrorCase(
            description="negative audit sample count",
            event_type="audit_completed",
            payload={"sample_count": -1},
            run_id="run-1",
            expected_error="sample_count.*nonnegative integer",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_audit_payload_when_creating_lifecycle_event_then_typed_field_is_rejected(
    test_case: LifecycleErrorCase,
) -> None:
    with invocation_scope("invocation-1"), run_scope(test_case.run_id or "run-1"):
        with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
            create_lifecycle_event(
                event_type=test_case.event_type,
                payload={
                    "audit_name": "valid_order_rate",
                    "audit_definition_name": "dq_column_rate",
                    "evaluation_mode": "measurement",
                    "outcome": "pass",
                    "severity": "warn",
                    "run_scope_phase": "final",
                    "attachment_kind": "model",
                    "sample_count": -1,
                },
            )


@pytest.mark.parametrize(
    "test_case",
    [
        StatementPrivacyCase(
            description="full SQL field is rejected",
            field_name="sql",
            value="select * from customers",
            expected_error="sql",
        ),
        StatementPrivacyCase(
            description="full SQL alias is rejected",
            field_name="full_sql",
            value="select secret from credentials",
            expected_error="full_sql",
        ),
        StatementPrivacyCase(
            description="parameter object is rejected",
            field_name="parameters",
            value={"token": "secret"},
            expected_error="parameters",
        ),
        StatementPrivacyCase(
            description="operation phase is rejected from statements",
            field_name="phase",
            value="inspect",
            expected_error="phase",
        ),
        StatementPrivacyCase(
            description="operation strategy is rejected from statements",
            field_name="strategy",
            value="rename",
            expected_error="strategy",
        ),
        StatementPrivacyCase(
            description="operation target kind is rejected from statements",
            field_name="target_kind",
            value="relation",
            expected_error="target_kind",
        ),
        StatementPrivacyCase(
            description="parameter values are rejected",
            field_name="parameter_values",
            value=["secret"],
            expected_error="parameter_values",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_statement_fact_with_private_content_when_constructing_then_field_is_rejected(
    test_case: StatementPrivacyCase,
) -> None:
    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        lifecycle_event(
            "statement_started",
            statement_id="statement-1",
            payload={test_case.field_name: test_case.value},
        )


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleErrorCase(
            description="nested parameter values are rejected",
            event_type="statement_failed",
            statement_id="statement-1",
            payload={"error_type": {"parameter_values": ["secret"]}},
            expected_error="parameter_values",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_nested_parameter_values_when_constructing_statement_fact_then_they_are_rejected(
    test_case: LifecycleErrorCase,
) -> None:
    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        lifecycle_event(
            test_case.event_type,
            statement_id=test_case.statement_id,
            payload=test_case.payload,
        )


@pytest.mark.parametrize(
    "test_case",
    (
        OperationAdapterCase(
            description="built-in adapter remains identifiable",
            adapter_name="snowflake",
            expected_adapter="snowflake",
        ),
        OperationAdapterCase(
            description="custom adapter name is bounded",
            adapter_name="customer_secret_warehouse",
            expected_adapter="custom",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_adapter_name_when_canonicalizing_then_only_catalogued_identity_is_returned(
    test_case: OperationAdapterCase,
) -> None:
    assert canonicalize_operation_adapter(test_case.adapter_name) == test_case.expected_adapter


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleErrorCase(
            description="resource attempt ID is required",
            event_type="resource_attempt_started",
            run_id="run-1",
            resource_id="model.orders",
            payload={},
            expected_error="requires correlation field.*resource_attempt_id",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_known_scoped_event_without_correlation_when_constructing_then_error_is_actionable(
    test_case: LifecycleErrorCase,
) -> None:
    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        lifecycle_event(
            test_case.event_type,
            run_id=test_case.run_id,
            resource_id=test_case.resource_id,
            payload=test_case.payload,
        )


@pytest.mark.parametrize(
    "test_case",
    (
        LifecycleErrorCase(
            description="retry requires safe error type",
            event_type="retry_scheduled",
            run_id="run-1",
            resource_id="asset:orders",
            payload={
                "failed_attempt_number": 1,
                "next_attempt_number": 2,
                "delay_ms": 100,
            },
            expected_error="requires payload field.*error_type",
        ),
        LifecycleErrorCase(
            description="retry attempts must be consecutive",
            event_type="retry_scheduled",
            run_id="run-1",
            resource_id="asset:orders",
            payload={
                "failed_attempt_number": 1,
                "next_attempt_number": 3,
                "delay_ms": 100,
                "error_type": "TimeoutError",
            },
            expected_error="next_attempt_number must equal",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_retry_contract_when_constructing_then_bounded_sequence_is_rejected(
    test_case: LifecycleErrorCase,
) -> None:
    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        lifecycle_event(
            test_case.event_type,
            run_id=test_case.run_id,
            resource_id=test_case.resource_id,
            resource_attempt_id="attempt-1",
            operation_id="operation-1",
            payload=test_case.payload,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        TimestampErrorCase(
            description="non-UTC offset is rejected",
            occurred_at=datetime(2026, 1, 1, tzinfo=timezone(timedelta(hours=1))),
            expected_error="occurred_at must use UTC",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_non_utc_timestamp_when_constructing_then_it_is_rejected(
    test_case: TimestampErrorCase,
) -> None:
    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        lifecycle_event(occurred_at=test_case.occurred_at)


@pytest.mark.parametrize(
    "test_case",
    [
        TerminalSemanticsCase(
            description="statement completion is terminal while progress facts are not",
            event_types=("statement_started", "statement_submitted", "statement_completed"),
            expected_terminal=(False, False, True),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_terminal_and_non_terminal_facts_when_classifying_then_catalog_semantics_apply(
    test_case: TerminalSemanticsCase,
) -> None:
    events: tuple[LifecycleEvent, ...] = tuple(
        lifecycle_event(event_type, statement_id="statement-1")
        for event_type in test_case.event_types
    )
    actual_terminal: tuple[bool, ...] = tuple(is_terminal_event(event) for event in events)

    assert actual_terminal == test_case.expected_terminal


@pytest.mark.parametrize(
    "test_case",
    [
        TerminalEvidenceCase(
            description="started and submitted facts contain no terminal evidence",
            expected_terminal_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_only_started_and_submitted_facts_when_inspecting_then_no_terminal_evidence_exists(
    test_case: TerminalEvidenceCase,
) -> None:
    events: tuple[LifecycleEvent, ...] = (
        lifecycle_event("invocation_started"),
        lifecycle_event("run_started", run_id="run-1"),
        lifecycle_event("statement_started", statement_id="statement-1"),
        lifecycle_event("statement_submitted", statement_id="statement-1"),
    )
    terminal_count: int = sum(is_terminal_event(event) for event in events)

    assert terminal_count == test_case.expected_terminal_count


@pytest.mark.parametrize(
    "test_case",
    [
        CatalogVersionCase(
            description="v2 catalog is authoritative and v3 is unsupported",
            schema_version=2,
            event_type="statement_completed",
            expected_terminal=True,
            expected_unsupported_version=3,
        ),
        CatalogVersionCase(
            description="microbatch completion is terminal in v2",
            schema_version=2,
            event_type="microbatch_completed",
            expected_terminal=True,
            expected_unsupported_version=3,
        ),
        CatalogVersionCase(
            description="microbatch failure is terminal in v2",
            schema_version=2,
            event_type="microbatch_failed",
            expected_terminal=True,
            expected_unsupported_version=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_catalog_when_looking_up_events_then_authority_is_version_dimensional(
    test_case: CatalogVersionCase,
) -> None:
    assert (
        lifecycle_event_terminal(
            schema_version=test_case.schema_version, event_type=test_case.event_type
        )
        is test_case.expected_terminal
    )
    assert test_case.expected_unsupported_version not in LIFECYCLE_EVENT_SCHEMA_VERSIONS


@pytest.mark.parametrize(
    "test_case",
    (
        CatalogEventPresenceCase(
            description="v1 excludes microbatch events",
            schema_version=1,
            event_type="microbatch_started",
            expected_present=False,
        ),
        CatalogEventPresenceCase(
            description="v2 includes microbatch events",
            schema_version=2,
            event_type="microbatch_started",
            expected_present=True,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_microbatch_events_when_inspecting_versioned_catalog_then_v1_remains_unchanged(
    test_case: CatalogEventPresenceCase,
) -> None:
    assert (
        lifecycle_event_catalogued(
            schema_version=test_case.schema_version, event_type=test_case.event_type
        )
        is test_case.expected_present
    )


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleErrorCase(
            description="negative duration is rejected",
            event_type="invocation_completed",
            payload={"duration_ms": -0.1},
            expected_error="nonnegative finite number",
        ),
        LifecycleErrorCase(
            description="infinite duration is rejected",
            event_type="invocation_completed",
            payload={"duration_ms": float("inf")},
            expected_error="NaN or infinity",
        ),
        LifecycleErrorCase(
            description="boolean duration is rejected",
            event_type="invocation_completed",
            payload={"duration_ms": True},
            expected_error="excluding bool",
        ),
        LifecycleErrorCase(
            description="negative count is rejected",
            event_type="run_started",
            run_id="run-1",
            payload={"selected_count": -1},
            expected_error="nonnegative integer",
        ),
        LifecycleErrorCase(
            description="boolean count is rejected",
            event_type="run_started",
            run_id="run-1",
            payload={"selected_count": True},
            expected_error="excluding bool",
        ),
        LifecycleErrorCase(
            description="zero configured concurrency is rejected",
            event_type="run_started",
            run_id="run-1",
            payload={
                "selected_count": 1,
                "configured_concurrency": 0,
                "worker_count": 0,
            },
            expected_error="configured_concurrency must be a positive integer",
        ),
        LifecycleErrorCase(
            description="worker count above selected count is rejected",
            event_type="run_started",
            run_id="run-1",
            payload={
                "selected_count": 1,
                "configured_concurrency": 2,
                "worker_count": 2,
            },
            expected_error="worker_count must equal min",
        ),
        LifecycleErrorCase(
            description="audit run count sum mismatch is rejected",
            event_type="run_completed",
            run_id="run-1",
            payload={
                "succeeded_count": 1,
                "failed_count": 0,
                "pass_count": 1,
                "warn_count": 1,
                "fail_count": 0,
            },
            expected_error="succeeded_count must equal",
        ),
        LifecycleErrorCase(
            description="non-string command is rejected",
            event_type="invocation_started",
            payload={"command": 7},
            expected_error="non-empty string",
        ),
        LifecycleErrorCase(
            description="boolean exit code is rejected",
            event_type="invocation_completed",
            payload={"exit_code": False},
            expected_error="integer excluding bool",
        ),
        LifecycleErrorCase(
            description="fractional affected rows are rejected",
            event_type="statement_completed",
            statement_id="statement-1",
            payload={"affected_rows": 1.5},
            expected_error="nonnegative integer",
        ),
        LifecycleErrorCase(
            description="non-object metadata is rejected",
            event_type="statement_completed",
            statement_id="statement-1",
            payload={"metadata": ["not", "an", "object"]},
            expected_error="metadata.*JSON object",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_catalogued_payload_value_when_constructing_then_it_is_rejected(
    test_case: LifecycleErrorCase,
) -> None:
    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        lifecycle_event(
            test_case.event_type,
            run_id=test_case.run_id,
            statement_id=test_case.statement_id,
            payload=test_case.payload,
        )


@pytest.mark.parametrize(
    "test_case",
    (
        RunLifecycleErrorCase(
            description="boolean selected count",
            selected_count=True,
            configured_concurrency=1,
            worker_count=1,
            expected_error="selected_count must be an integer excluding bool",
        ),
        RunLifecycleErrorCase(
            description="zero configured concurrency",
            selected_count=1,
            configured_concurrency=0,
            worker_count=0,
            expected_error="configured_concurrency must be positive",
        ),
        RunLifecycleErrorCase(
            description="physical worker mismatch",
            selected_count=1,
            configured_concurrency=2,
            worker_count=2,
            expected_error="worker_count must equal min",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_run_bounds_when_constructing_run_lifecycle_then_it_is_rejected(
    test_case: RunLifecycleErrorCase,
) -> None:
    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        RunLifecycle(
            run_kind="audit",
            selected_count=test_case.selected_count,  # ty: ignore[invalid-argument-type]
            configured_concurrency=test_case.configured_concurrency,  # ty: ignore[invalid-argument-type]
            worker_count=test_case.worker_count,  # ty: ignore[invalid-argument-type]
        )


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleErrorCase(
            description="metadata above 4096 encoded bytes is rejected",
            event_type="statement_completed",
            statement_id="statement-1",
            payload={"metadata": {"x": "x" * 4089}},
            expected_error="at most 4096 bytes",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_metadata_over_encoded_bound_when_constructing_then_it_is_rejected(
    test_case: LifecycleErrorCase,
) -> None:
    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        lifecycle_event(
            test_case.event_type,
            statement_id=test_case.statement_id,
            payload=test_case.payload,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        IdempotencyCase(
            description="identical duplicate event is idempotent",
            duplicate_command=None,
            expected_event_id="evt-1",
            expected_error="",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_identical_duplicate_event_id_when_validating_then_it_is_idempotent(
    test_case: IdempotencyCase,
) -> None:
    event: LifecycleEvent = lifecycle_event()

    validate_idempotent_duplicate(original=event, duplicate=event)

    assert event.event_id == test_case.expected_event_id


@pytest.mark.parametrize(
    "test_case",
    [
        IdempotencyCase(
            description="same event ID with changed payload conflicts",
            duplicate_command="plan",
            expected_event_id="evt-1",
            expected_error="conflicting lifecycle facts",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_duplicate_event_id_with_different_fact_when_validating_then_conflict_is_rejected(
    test_case: IdempotencyCase,
) -> None:
    event: LifecycleEvent = lifecycle_event()
    conflicting: LifecycleEvent = replace(event, payload={"command": test_case.duplicate_command})

    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        validate_idempotent_duplicate(original=event, duplicate=conflicting)

    assert conflicting.event_id == test_case.expected_event_id
