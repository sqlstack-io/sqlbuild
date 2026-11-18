"""Measurement audit execution test cases."""

from dataclasses import dataclass

from sqlbuild.compiler.auditing.types import AuditOutcome, AuditSeverity


@dataclass(frozen=True)
class MeasurementRowCountCase:
    description: str
    row_count: int
    expected_error: str


@dataclass(frozen=True)
class InvalidMeasurementCase:
    description: str
    columns: tuple[str, str]
    row: tuple[object, object]
    expected_error: str


@dataclass(frozen=True)
class MeasurementOutcomeCase:
    description: str
    value: int
    sample_count: int | None
    expected_outcome: AuditOutcome


@dataclass(frozen=True)
class NullMeasurementOutcomeCase:
    description: str
    sample_count: int
    expected_outcome: AuditOutcome


@dataclass(frozen=True)
class AuditExecutionCase:
    description: str
    expected_outcome: AuditOutcome


@dataclass(frozen=True)
class AuditRaisedErrorCase:
    description: str
    severity: AuditSeverity
    error: Exception
    expected_execution_error: str


@dataclass(frozen=True)
class AuditInterruptCase:
    description: str
    error: BaseException
    expected_error_type: type[BaseException]


@dataclass(frozen=True)
class IgnoredFailureProjectionCase:
    description: str
    ignored: bool
    expected_metadata_json: str | None
    expected_payload_ignored: bool | None
