"""Tests for deterministic observability JSON codecs."""

import json

import pytest

from sqlbuild.runtime.observability.exceptions import ObservabilityValidationError
from sqlbuild.runtime.observability.main.diagnostic_log_from_json import diagnostic_log_from_json
from sqlbuild.runtime.observability.main.diagnostic_log_to_json import diagnostic_log_to_json
from sqlbuild.runtime.observability.main.lifecycle_event_from_json import lifecycle_event_from_json
from sqlbuild.runtime.observability.main.lifecycle_event_to_json import lifecycle_event_to_json
from sqlbuild.runtime.observability.models import (
    DiagnosticLog,
    LifecycleEvent,
    OpaqueLifecycleEvent,
)
from tests.unit.src.sqlbuild.runtime.observability._test_types import (
    DiagnosticRoundTripCase,
    DiagnosticSeparationCase,
    EnvelopeFieldErrorCase,
    EventCase,
    ExactJsonCase,
    InvocationMetadataCase,
    JsonErrorCase,
    MetadataBoundaryCase,
    OpaqueRoundTripCase,
    RetiredOperationCase,
    SchemaVersionCase,
    StatementRoundTripCase,
)
from tests.unit.src.sqlbuild.runtime.observability.helpers import OCCURRED_AT, lifecycle_event


@pytest.mark.parametrize(
    "test_case",
    [
        ExactJsonCase(
            description="known event serialization is exact and deterministic",
            payload={"command": "build"},
            expected_json=(
                '{"event_id":"evt-1","event_type":"invocation_started",'
                '"invocation_id":"inv-1","occurred_at":"2026-08-31T12:34:56.123456Z",'
                '"operation_id":null,"payload":{"command":"build"},"producer":"sqlbuild",'
                '"producer_version":"0.72.1","resource_attempt_id":null,'
                '"resource_id":null,"run_id":null,"schema_version":1,"statement_id":null}'
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_lifecycle_event_when_serializing_then_json_is_exact_and_deterministic(
    test_case: ExactJsonCase,
) -> None:
    event: LifecycleEvent = lifecycle_event(payload=test_case.payload)
    encoded: str = lifecycle_event_to_json(event)

    assert encoded == test_case.expected_json
    assert lifecycle_event_to_json(event) == test_case.expected_json


@pytest.mark.parametrize(
    "test_case",
    [
        EventCase("invocation started", "invocation_started", "invocation_started"),
        EventCase("invocation completed", "invocation_completed", "invocation_completed"),
        EventCase("invocation failed", "invocation_failed", "invocation_failed"),
        EventCase("run started", "run_started", "run_started", run_id="run-1"),
        EventCase("run completed", "run_completed", "run_completed", run_id="run-1"),
        EventCase("run failed", "run_failed", "run_failed", run_id="run-1"),
        EventCase(
            "resource attempt started",
            "resource_attempt_started",
            "resource_attempt_started",
            run_id="run-1",
            resource_id="model.orders",
            resource_attempt_id="attempt-1",
        ),
        EventCase(
            "resource attempt completed",
            "resource_attempt_completed",
            "resource_attempt_completed",
            run_id="run-1",
            resource_id="model.orders",
            resource_attempt_id="attempt-1",
        ),
        EventCase(
            "resource attempt failed",
            "resource_attempt_failed",
            "resource_attempt_failed",
            run_id="run-1",
            resource_id="model.orders",
            resource_attempt_id="attempt-1",
        ),
        EventCase(
            "operation started",
            "operation_started",
            "operation_started",
            operation_id="operation-1",
        ),
        EventCase(
            "operation completed",
            "operation_completed",
            "operation_completed",
            operation_id="operation-1",
        ),
        EventCase(
            "operation failed", "operation_failed", "operation_failed", operation_id="operation-1"
        ),
        EventCase(
            "statement started",
            "statement_started",
            "statement_started",
            statement_id="statement-1",
        ),
        EventCase(
            "statement submitted",
            "statement_submitted",
            "statement_submitted",
            statement_id="statement-1",
        ),
        EventCase(
            "statement completed",
            "statement_completed",
            "statement_completed",
            statement_id="statement-1",
        ),
        EventCase(
            "statement failed", "statement_failed", "statement_failed", statement_id="statement-1"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_known_event_when_serializing_and_decoding_then_round_trips(
    test_case: EventCase,
) -> None:
    event: LifecycleEvent = lifecycle_event(
        test_case.event_type,
        run_id=test_case.run_id,
        resource_id=test_case.resource_id,
        resource_attempt_id=test_case.resource_attempt_id,
        operation_id=test_case.operation_id,
        statement_id=test_case.statement_id,
    )
    decoded: LifecycleEvent | OpaqueLifecycleEvent = lifecycle_event_from_json(
        lifecycle_event_to_json(event)
    )

    assert decoded == event
    assert isinstance(decoded, LifecycleEvent)
    assert decoded.event_type == test_case.expected_event_type


@pytest.mark.parametrize(
    "test_case",
    [
        RetiredOperationCase(
            "a stored dlt or callable load",
            "loader",
            "external_source_load",
            "external_source_load",
        ),
        RetiredOperationCase(
            "a stored ingestr subprocess", "subprocess", "ingestr_command", "ingestr_command"
        ),
        RetiredOperationCase(
            "a stored Python task attempt", "python_node", "python_task", "python_task"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_operation_from_a_removed_integration_when_decoding_history_then_it_reads_back(
    test_case: RetiredOperationCase,
) -> None:
    stored: str = json.dumps(
        {
            "event_id": "evt-retired",
            "event_type": "operation_started",
            "schema_version": 1,
            "producer": "sqlbuild",
            "producer_version": "0.238.0",
            "occurred_at": "2026-08-31T12:34:56.123456Z",
            "invocation_id": "inv-1",
            "operation_id": "op-1",
            "payload": {
                "operation_kind": test_case.operation_kind,
                "operation_name": test_case.operation_name,
            },
        }
    )

    decoded: LifecycleEvent | OpaqueLifecycleEvent = lifecycle_event_from_json(stored)

    assert isinstance(decoded, LifecycleEvent)
    assert decoded.payload["operation_name"] == test_case.expected_operation_name


@pytest.mark.parametrize(
    "test_case",
    [
        InvocationMetadataCase(
            description="v2 invocation ordering and integration context round trip",
            event_count=1,
            expected_external_context={
                "integration": {
                    "name": "dagster",
                    "run_id": "dagster-run-1",
                    "retry_number": 0,
                }
            },
            expected_first_sequence=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_v2_lifecycle_event_when_round_tripping_then_invocation_metadata_is_preserved(
    test_case: InvocationMetadataCase,
) -> None:
    event: LifecycleEvent = LifecycleEvent(
        event_id="evt-v2",
        event_type="invocation_started",
        schema_version=2,
        producer="sqlbuild",
        producer_version="0.90.0",
        occurred_at=OCCURRED_AT,
        invocation_id="inv-1",
        invocation_sequence=0,
        external_context=test_case.expected_external_context,
        payload={"command": "build"},
    )

    encoded: str = lifecycle_event_to_json(event)
    decoded: LifecycleEvent | OpaqueLifecycleEvent = lifecycle_event_from_json(encoded)

    assert decoded == event
    assert isinstance(decoded, LifecycleEvent)
    assert decoded.invocation_sequence == test_case.expected_first_sequence
    assert decoded.external_context == test_case.expected_external_context


@pytest.mark.parametrize(
    "test_case",
    [
        OpaqueRoundTripCase(
            description="unknown v1 event name remains opaque",
            raw={
                "event_id": "future-1",
                "event_type": "checkpoint_recorded",
                "schema_version": 1,
                "future": {"items": [1, 2]},
            },
            expected_raw={
                "event_id": "future-1",
                "event_type": "checkpoint_recorded",
                "schema_version": 1,
                "future": {"items": [1, 2]},
            },
        ),
        OpaqueRoundTripCase(
            description="known event with newer version remains opaque",
            raw={
                "event_id": "future-2",
                "event_type": "run_started",
                "schema_version": 3,
                "producer": {"identity": "future-producer"},
                "future_field": None,
            },
            expected_raw={
                "event_id": "future-2",
                "event_type": "run_started",
                "schema_version": 3,
                "producer": {"identity": "future-producer"},
                "future_field": None,
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_lifecycle_envelope_when_round_tripping_then_remains_opaque_and_lossless(
    test_case: OpaqueRoundTripCase,
) -> None:
    encoded: str = json.dumps(test_case.raw, separators=(",", ":"))
    decoded: LifecycleEvent | OpaqueLifecycleEvent = lifecycle_event_from_json(encoded)

    assert isinstance(decoded, OpaqueLifecycleEvent)
    assert json.loads(lifecycle_event_to_json(decoded)) == test_case.expected_raw


@pytest.mark.parametrize(
    "test_case",
    [
        JsonErrorCase(
            description="known event missing event ID names the field",
            encoded=(
                '{"event_type":"run_started","schema_version":1,'
                '"occurred_at":"2026-01-01T00:00:00Z"}'
            ),
            expected_error="known event 'run_started' is missing required envelope field 'event_id'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_known_event_missing_field_when_decoding_then_error_names_field_and_event(
    test_case: JsonErrorCase,
) -> None:
    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        lifecycle_event_from_json(test_case.encoded)


@pytest.mark.parametrize(
    "test_case",
    [
        StatementRoundTripCase(
            description="safe statement fields round trip",
            payload={
                "intent": "materialize model",
                "sql_digest": "sha256:abc123",
                "job_id": "warehouse-job-1",
                "query_id": "query-1",
                "row_count": 12,
                "affected_rows": 10,
                "batch_size": 100,
                "duration_ms": 12.5,
                "metadata": {"warehouse": "example", "labels": ["build"]},
            },
            expected_affected_rows=10,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_safe_statement_fields_when_round_tripping_then_values_are_preserved(
    test_case: StatementRoundTripCase,
) -> None:
    event: LifecycleEvent = lifecycle_event(
        "statement_completed", statement_id="statement-1", payload=test_case.payload
    )
    decoded: LifecycleEvent | OpaqueLifecycleEvent = lifecycle_event_from_json(
        lifecycle_event_to_json(event)
    )

    assert decoded == event
    assert isinstance(decoded, LifecycleEvent)
    assert decoded.payload["affected_rows"] == test_case.expected_affected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        MetadataBoundaryCase(
            description="metadata exactly at encoded bound is accepted",
            metadata_value="x" * 4088,
            expected_encoded_size=4096,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_metadata_at_encoded_bound_when_round_tripping_then_it_is_accepted(
    test_case: MetadataBoundaryCase,
) -> None:
    metadata: dict[str, str] = {"x": test_case.metadata_value}
    event: LifecycleEvent = lifecycle_event(
        "statement_completed", statement_id="statement-1", payload={"metadata": metadata}
    )
    encoded_size: int = len(
        json.dumps(metadata, ensure_ascii=True, separators=(",", ":"), sort_keys=True).encode(
            "utf-8"
        )
    )

    assert lifecycle_event_from_json(lifecycle_event_to_json(event)) == event
    assert encoded_size == test_case.expected_encoded_size


@pytest.mark.parametrize(
    "test_case",
    [
        SchemaVersionCase("boolean lifecycle version", True, "positive integer excluding bool"),
        SchemaVersionCase("float lifecycle version", 1.0, "positive integer excluding bool"),
        SchemaVersionCase("string lifecycle version", "1", "positive integer excluding bool"),
        SchemaVersionCase("zero lifecycle version", 0, "positive integer excluding bool"),
        SchemaVersionCase("negative lifecycle version", -1, "positive integer excluding bool"),
    ],
    ids=lambda case: case.description,
)
def test_given_malformed_lifecycle_schema_version_when_decoding_then_error_is_actionable(
    test_case: SchemaVersionCase,
) -> None:
    encoded: str = json.dumps(
        {"event_type": "run_started", "schema_version": test_case.schema_version},
        separators=(",", ":"),
    )

    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        lifecycle_event_from_json(encoded)


@pytest.mark.parametrize(
    "test_case",
    [
        EnvelopeFieldErrorCase(
            description="known v1 lifecycle event rejects unknown envelope field",
            field_name="future_field",
            field_value="not allowed in v1",
            expected_error="unknown top-level field.*future_field",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_known_v1_event_with_unknown_envelope_field_when_decoding_then_it_is_rejected(
    test_case: EnvelopeFieldErrorCase,
) -> None:
    data: dict[str, object] = json.loads(lifecycle_event_to_json(lifecycle_event()))
    data[test_case.field_name] = test_case.field_value

    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        lifecycle_event_from_json(json.dumps(data))


@pytest.mark.parametrize(
    "test_case",
    [
        DiagnosticRoundTripCase(
            description="diagnostic correlations and fields round trip",
            severity="warning",
            message="statement retry scheduled",
            expected_message="statement retry scheduled",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_diagnostic_log_when_round_tripping_then_correlations_and_fields_are_preserved(
    test_case: DiagnosticRoundTripCase,
) -> None:
    log: DiagnosticLog = DiagnosticLog(
        schema_version=1,
        producer="sqlbuild",
        producer_version="0.72.1",
        occurred_at=OCCURRED_AT,
        severity=test_case.severity,
        logger="sqlbuild.executor",
        source="adapter",
        message=test_case.message,
        fields={"attempt": 2, "delays": [0.1, 0.5]},
        log_stream_id="worker-2",
        invocation_id="inv-1",
        statement_id="statement-1",
    )
    decoded: DiagnosticLog = diagnostic_log_from_json(diagnostic_log_to_json(log))
    encoded_data: dict[str, object] = json.loads(diagnostic_log_to_json(log))

    assert decoded == log
    assert decoded.message == test_case.expected_message
    assert str(encoded_data["occurred_at"]).endswith("Z")
    assert encoded_data["run_id"] is None
    assert "event_id" not in encoded_data
    assert "event_type" not in encoded_data


@pytest.mark.parametrize(
    "test_case",
    [
        DiagnosticSeparationCase(
            description="diagnostic JSON is not coerced to lifecycle fact",
            expected_type=OpaqueLifecycleEvent,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_diagnostic_json_when_reading_as_lifecycle_then_it_is_not_coerced_to_a_fact(
    test_case: DiagnosticSeparationCase,
) -> None:
    log: DiagnosticLog = DiagnosticLog(
        schema_version=1,
        producer="sqlbuild",
        producer_version="0.72.1",
        occurred_at=OCCURRED_AT,
        severity="info",
        logger="sqlbuild",
        source="runtime",
        message="working",
    )
    decoded: LifecycleEvent | OpaqueLifecycleEvent = lifecycle_event_from_json(
        diagnostic_log_to_json(log)
    )

    assert isinstance(decoded, test_case.expected_type)


@pytest.mark.parametrize(
    "test_case",
    [
        SchemaVersionCase("boolean diagnostic version", False, "positive integer excluding bool"),
        SchemaVersionCase("float diagnostic version", 1.5, "positive integer excluding bool"),
        SchemaVersionCase("string diagnostic version", "1", "positive integer excluding bool"),
        SchemaVersionCase("zero diagnostic version", 0, "positive integer excluding bool"),
        SchemaVersionCase("negative diagnostic version", -2, "positive integer excluding bool"),
    ],
    ids=lambda case: case.description,
)
def test_given_malformed_diagnostic_schema_version_when_decoding_then_error_is_actionable(
    test_case: SchemaVersionCase,
) -> None:
    data: dict[str, object] = json.loads(
        diagnostic_log_to_json(
            DiagnosticLog(
                schema_version=1,
                producer="sqlbuild",
                producer_version="0.72.1",
                occurred_at=OCCURRED_AT,
                severity="info",
                logger="sqlbuild",
                source="runtime",
                message="working",
            )
        )
    )
    data["schema_version"] = test_case.schema_version

    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        diagnostic_log_from_json(json.dumps(data))


@pytest.mark.parametrize(
    "test_case",
    [
        EnvelopeFieldErrorCase(
            description="known diagnostic rejects lifecycle event ID",
            field_name="event_id",
            field_value="not-a-diagnostic-field",
            expected_error="unknown top-level field.*event_id",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_known_diagnostic_with_unknown_envelope_field_when_decoding_then_it_is_rejected(
    test_case: EnvelopeFieldErrorCase,
) -> None:
    log: DiagnosticLog = DiagnosticLog(
        schema_version=1,
        producer="sqlbuild",
        producer_version="0.72.1",
        occurred_at=OCCURRED_AT,
        severity="info",
        logger="sqlbuild",
        source="runtime",
        message="working",
    )
    data: dict[str, object] = json.loads(diagnostic_log_to_json(log))
    data[test_case.field_name] = test_case.field_value

    with pytest.raises(ObservabilityValidationError, match=test_case.expected_error):
        diagnostic_log_from_json(json.dumps(data))
