"""Frozen parameter cases for runtime observability tests."""

from collections.abc import Mapping
from dataclasses import dataclass
from datetime import datetime

from sqlbuild.runtime.observability.models import OpaqueLifecycleEvent
from sqlbuild.runtime.observability.types import JSONValue


@dataclass(frozen=True)
class EventCase:
    description: str
    event_type: str
    expected_event_type: str
    run_id: str | None = None
    resource_id: str | None = None
    resource_attempt_id: str | None = None
    operation_id: str | None = None
    statement_id: str | None = None


@dataclass(frozen=True)
class StatementPrivacyCase:
    description: str
    field_name: str
    value: JSONValue
    expected_error: str


@dataclass(frozen=True)
class OperationAdapterCase:
    description: str
    adapter_name: object
    expected_adapter: str


@dataclass(frozen=True)
class LifecycleErrorCase:
    description: str
    event_type: str
    payload: Mapping[str, JSONValue]
    expected_error: str
    run_id: str | None = None
    resource_id: str | None = None
    statement_id: str | None = None


@dataclass(frozen=True)
class TimestampErrorCase:
    description: str
    occurred_at: datetime
    expected_error: str


@dataclass(frozen=True)
class TerminalSemanticsCase:
    description: str
    event_types: tuple[str, ...]
    expected_terminal: tuple[bool, ...]


@dataclass(frozen=True)
class TerminalEvidenceCase:
    description: str
    expected_terminal_count: int


@dataclass(frozen=True)
class CatalogVersionCase:
    description: str
    schema_version: int
    event_type: str
    expected_terminal: bool
    expected_unsupported_version: int


@dataclass(frozen=True)
class CatalogEventPresenceCase:
    description: str
    schema_version: int
    event_type: str
    expected_present: bool


@dataclass(frozen=True)
class IdempotencyCase:
    description: str
    duplicate_command: str | None
    expected_event_id: str
    expected_error: str


@dataclass(frozen=True)
class ImmutabilityCase:
    description: str
    command: str
    expected_command: str


@dataclass(frozen=True)
class SchemaVersionCase:
    description: str
    schema_version: object
    expected_error: str


@dataclass(frozen=True)
class RunLifecycleErrorCase:
    description: str
    selected_count: object
    configured_concurrency: object
    worker_count: object
    expected_error: str


@dataclass(frozen=True)
class ExactJsonCase:
    description: str
    payload: Mapping[str, JSONValue]
    expected_json: str


@dataclass(frozen=True)
class OpaqueRoundTripCase:
    description: str
    raw: Mapping[str, JSONValue]
    expected_raw: Mapping[str, JSONValue]


@dataclass(frozen=True)
class RetiredOperationCase:
    description: str
    operation_kind: str
    operation_name: str
    expected_operation_name: str


@dataclass(frozen=True)
class JsonErrorCase:
    description: str
    encoded: str
    expected_error: str


@dataclass(frozen=True)
class StatementRoundTripCase:
    description: str
    payload: Mapping[str, JSONValue]
    expected_affected_rows: int


@dataclass(frozen=True)
class MetadataBoundaryCase:
    description: str
    metadata_value: str
    expected_encoded_size: int


@dataclass(frozen=True)
class DiagnosticRoundTripCase:
    description: str
    severity: str
    message: str
    expected_message: str


@dataclass(frozen=True)
class DiagnosticSeparationCase:
    description: str
    expected_type: type[OpaqueLifecycleEvent]


@dataclass(frozen=True)
class EnvelopeFieldErrorCase:
    description: str
    field_name: str
    field_value: JSONValue
    expected_error: str


@dataclass(frozen=True)
class IdentityFieldErrorCase:
    description: str
    field_name: str
    field_value: object
    expected_error: str


@dataclass(frozen=True)
class IdentityBehaviorCase:
    description: str
    expected_invocation_id: str


@dataclass(frozen=True)
class DispatchCountCase:
    description: str
    expected_lifecycle_count: int
    expected_diagnostic_count: int


@dataclass(frozen=True)
class OpaqueDispatchCase:
    description: str
    raw: Mapping[str, JSONValue]
    expected_typed_count: int
    expected_opaque_count: int


@dataclass(frozen=True)
class DispatchOrderCase:
    description: str
    expected_order: tuple[str, ...]


@dataclass(frozen=True)
class DispatchFailureCase:
    description: str
    expected_health_count: int
    expected_healthy_count: int
    expected_channel: str


@dataclass(frozen=True)
class RecursiveHealthCase:
    description: str
    channel: str
    expected_health_count: int
    expected_healthy_count: int


@dataclass(frozen=True)
class DispatchMutationCase:
    description: str
    expected_first_publish: tuple[str, ...]
    expected_second_publish: tuple[str, ...]
    expected_after_cleanup: tuple[str, ...]


@dataclass(frozen=True)
class ConcurrentDispatchCase:
    description: str
    publisher_count: int
    events_per_publisher: int
    expected_count: int


@dataclass(frozen=True)
class ProducerVersionCase:
    description: str
    producer: str
    expected_error: str


@dataclass(frozen=True)
class InvocationMetadataCase:
    description: str
    event_count: int
    expected_external_context: Mapping[str, JSONValue]
    expected_first_sequence: int


@dataclass(frozen=True)
class PublicScopeSequenceCase:
    description: str
    expected_sequence: int


@dataclass(frozen=True)
class BlockingDispatchCase:
    description: str
    expected_before_release: tuple[str, ...]
    expected_after_release: tuple[str, ...]


@dataclass(frozen=True)
class FactoryCase:
    description: str
    expected_event_id: str
    expected_producer: str
    expected_producer_version: str
    expected_invocation_id: str
    expected_run_id: str
    expected_resource_id: str
    expected_resource_attempt_id: str
    expected_operation_id: str
    expected_statement_id: str


@dataclass(frozen=True)
class DispatcherContextCase:
    description: str
    expected_outer_restored: bool
    expected_thread_isolated: bool
    expected_task_isolated: bool


@dataclass(frozen=True)
class StatementLifecycleCase:
    description: str
    sql: str
    parameters: tuple[str, ...]
    expected_event_types: tuple[str, ...]
    expected_batch_size: int
    expected_call_count: int = 1


@dataclass(frozen=True)
class OperationLifecycleCase:
    description: str
    expected_event_types: tuple[str, ...]
    operation_kind: str = "project"
    operation_name: str = "project_compile"


@dataclass(frozen=True)
class StatementKindPrivacyCase:
    description: str
    sql: str
    expected_statement_kind: str
    private_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ErrorCodePrivacyCase:
    description: str
    attribute_name: str
    code: object
    expected_error_code: str | None
    private_fragments: tuple[str, ...]


@dataclass(frozen=True)
class FastStatementMonitorCase:
    description: str
    statement_count: int
    expected_monitor_threads: int


@dataclass(frozen=True)
class DeferredMonitorCase:
    description: str
    threshold_seconds: float
    query_id: str
    expected_submissions: tuple[str, ...]


@dataclass(frozen=True)
class FreezeJsonErrorCase:
    description: str
    payload: object
    expected_error: str


@dataclass(frozen=True)
class InvocationSequenceCase:
    description: str
    sequence: object
    expected_error: str = ""
    expected_sequence: int | None = None


@dataclass(frozen=True)
class FinishedStatementRetentionCase:
    description: str
    statement_count: int
    sql_bytes: int
    expected_live_lifecycles: int


@dataclass(frozen=True)
class DeferredStartCancellationCase:
    description: str
    delay_seconds: float
    expected_pending: int
    expected_runs: int


@dataclass(frozen=True)
class FailedMonitorStartCase:
    description: str
    threshold_seconds: float
    expected_event_types: tuple[str, ...]


@dataclass(frozen=True)
class HandledFailureCase:
    description: str
    error: BaseException
    expected_outcome: str | None
