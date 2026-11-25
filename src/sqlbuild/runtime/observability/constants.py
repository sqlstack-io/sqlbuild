"""Lifecycle envelope, diagnostic log and emitter constants; the event catalogue is native."""

from collections.abc import Mapping
from types import MappingProxyType

LIFECYCLE_INVOCATION_METADATA_SCHEMA_VERSION: int = 2
RESOURCE_ATTEMPT_SKIPPED_EVENT: str = "resource_attempt_skipped"
RENAME_OPERATION_STRATEGY: str = "rename"
RETRY_SCHEDULED_EVENT: str = "retry_scheduled"
STATEMENT_HEARTBEAT_THRESHOLD_SECONDS: float = 30.0
MODEL_BUILD_PURPOSE: str = "model_build"
AUDIT_PURPOSE: str = "audit"
COST_TELEMETRY_PURPOSE: str = "cost_telemetry"
LOCAL_RUN_ON: str = "local"
LIFECYCLE_ENVELOPE_FIELDS_V1: frozenset[str] = frozenset(
    {
        "event_id",
        "event_type",
        "schema_version",
        "producer",
        "producer_version",
        "occurred_at",
        "invocation_id",
        "run_id",
        "resource_id",
        "resource_attempt_id",
        "operation_id",
        "statement_id",
        "payload",
    }
)
LIFECYCLE_ENVELOPE_FIELDS_V2: frozenset[str] = LIFECYCLE_ENVELOPE_FIELDS_V1 | frozenset(
    {"invocation_sequence", "external_context"}
)
LIFECYCLE_ENVELOPE_FIELDS_BY_VERSION: Mapping[int, frozenset[str]] = MappingProxyType(
    {1: LIFECYCLE_ENVELOPE_FIELDS_V1, 2: LIFECYCLE_ENVELOPE_FIELDS_V2}
)
DIAGNOSTIC_ENVELOPE_FIELDS: frozenset[str] = frozenset(
    {
        "schema_version",
        "producer",
        "producer_version",
        "occurred_at",
        "severity",
        "logger",
        "source",
        "message",
        "fields",
        "log_stream_id",
        "invocation_id",
        "run_id",
        "resource_id",
        "resource_attempt_id",
        "operation_id",
        "statement_id",
    }
)
