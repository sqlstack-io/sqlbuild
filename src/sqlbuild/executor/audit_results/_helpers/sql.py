"""Portable SQL builders for append-only audit result history."""

from __future__ import annotations

from collections.abc import Callable, Sequence

from sqlbuild._native import (
    AUDIT_RESULTS_TABLE_NAME,
    render_state_sql_literal,
    render_state_table_create_sql,
)
from sqlbuild.adapter.contract.types import FrameworkType
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.audit_results.constants import (
    AUDIT_RESULT_COLUMN_TYPES,
    AUDIT_RESULT_COLUMNS,
    COLUMN_ATTACHED_COLUMN_NAME,
    COLUMN_ATTACHED_TARGET_KIND,
    COLUMN_ATTACHED_TARGET_NAME,
    COLUMN_ATTACHMENT_KIND,
    COLUMN_AUDIT_DEFINITION_NAME,
    COLUMN_AUDIT_DESCRIPTION,
    COLUMN_AUDIT_META_JSON,
    COLUMN_AUDIT_NAME,
    COLUMN_BINDING_KEY,
    COLUMN_DEFINITION_FINGERPRINT,
    COLUMN_EVALUATION_MODE,
    COLUMN_EVIDENCE_COUNT,
    COLUMN_EVIDENCE_ERROR,
    COLUMN_EVIDENCE_JSON,
    COLUMN_EVIDENCE_SQL,
    COLUMN_EVIDENCE_TRUNCATED,
    COLUMN_EXECUTED_SQL,
    COLUMN_EXECUTION_ERROR,
    COLUMN_EXECUTION_FINGERPRINT,
    COLUMN_INVOCATION_ID,
    COLUMN_MEASURED_VALUE,
    COLUMN_MEASUREMENT_SQL,
    COLUMN_METADATA_JSON,
    COLUMN_MINIMUM_SAMPLES,
    COLUMN_OCCURRED_AT,
    COLUMN_OUTCOME,
    COLUMN_RESULT_ID,
    COLUMN_REUSED,
    COLUMN_RUN_ID,
    COLUMN_RUN_SCOPE_PHASE,
    COLUMN_SAMPLE_COUNT,
    COLUMN_SAMPLE_UNIT,
    COLUMN_SCHEMA_VERSION,
    COLUMN_SEVERITY,
    COLUMN_SQL_DIGEST,
    COLUMN_TARGET_DATABASE,
    COLUMN_TARGET_NAME,
    COLUMN_TARGET_SCHEMA,
    COLUMN_THRESHOLDS_JSON,
    COLUMN_VIOLATION_COUNT,
)
from sqlbuild.executor.audit_results.models import AuditResultRecord

_REQUIRED_COLUMNS: frozenset[str] = frozenset(
    {
        COLUMN_RESULT_ID,
        COLUMN_SCHEMA_VERSION,
        COLUMN_OCCURRED_AT,
        COLUMN_INVOCATION_ID,
        COLUMN_RUN_ID,
        COLUMN_AUDIT_NAME,
        COLUMN_AUDIT_DEFINITION_NAME,
        COLUMN_BINDING_KEY,
        COLUMN_DEFINITION_FINGERPRINT,
        COLUMN_EXECUTION_FINGERPRINT,
        COLUMN_EVALUATION_MODE,
        COLUMN_RUN_SCOPE_PHASE,
        COLUMN_ATTACHMENT_KIND,
        COLUMN_SEVERITY,
        COLUMN_OUTCOME,
        COLUMN_REUSED,
    }
)


def build_qualified_table_name(
    *, database: str | None, schema: str, render_qualified_name: Callable[..., str | None]
) -> str:
    qualified_name: str | None = render_qualified_name(
        database=database, schema=schema, name=AUDIT_RESULTS_TABLE_NAME
    )
    if qualified_name is None:
        raise ExecutorInputError("audit result table requires a target schema")
    return qualified_name


def build_create_table_sql(
    *,
    database: str | None,
    schema: str,
    render_qualified_name: Callable[..., str | None],
    render_framework_type: Callable[[FrameworkType], str],
) -> str:
    return render_state_table_create_sql(
        qualified_name=build_qualified_table_name(
            database=database, schema=schema, render_qualified_name=render_qualified_name
        ),
        columns=AUDIT_RESULT_COLUMNS,
        column_types=AUDIT_RESULT_COLUMN_TYPES,
        required_columns=_REQUIRED_COLUMNS,
        render_framework_type=render_framework_type,
    )


def build_insert_sql(
    *,
    database: str | None,
    schema: str,
    records: Sequence[AuditResultRecord],
    render_qualified_name: Callable[..., str | None],
    include_meta: bool = True,
) -> str:
    if not records:
        raise ExecutorInputError("audit result insert requires at least one record")
    qualified_name: str = build_qualified_table_name(
        database=database, schema=schema, render_qualified_name=render_qualified_name
    )
    columns: tuple[str, ...] = insert_columns(records=records, include_meta=include_meta)
    rows: str = ", ".join(_record_values(record=record, columns=columns) for record in records)
    return f"INSERT INTO {qualified_name} ({', '.join(columns)}) VALUES {rows}"


def insert_columns(
    *, records: Sequence[AuditResultRecord], include_meta: bool = True
) -> tuple[str, ...]:
    """Return inserted columns; the additive meta column is named only when a record has meta."""

    if include_meta and any(record.audit_meta_json is not None for record in records):
        return AUDIT_RESULT_COLUMNS
    return tuple(column for column in AUDIT_RESULT_COLUMNS if column != COLUMN_AUDIT_META_JSON)


def _record_values(*, record: AuditResultRecord, columns: tuple[str, ...]) -> str:
    values: dict[str, object | None] = {
        COLUMN_RESULT_ID: record.result_id,
        COLUMN_SCHEMA_VERSION: record.schema_version,
        COLUMN_OCCURRED_AT: record.occurred_at,
        COLUMN_INVOCATION_ID: record.invocation_id,
        COLUMN_RUN_ID: record.run_id,
        COLUMN_AUDIT_NAME: record.audit_name,
        COLUMN_AUDIT_DEFINITION_NAME: record.audit_definition_name,
        COLUMN_AUDIT_DESCRIPTION: record.audit_description,
        COLUMN_BINDING_KEY: record.binding_key,
        COLUMN_DEFINITION_FINGERPRINT: record.definition_fingerprint,
        COLUMN_EXECUTION_FINGERPRINT: record.execution_fingerprint,
        COLUMN_EVALUATION_MODE: record.evaluation_mode,
        COLUMN_RUN_SCOPE_PHASE: record.run_scope_phase,
        COLUMN_ATTACHMENT_KIND: record.attachment_kind,
        COLUMN_ATTACHED_TARGET_KIND: record.attached_target_kind,
        COLUMN_ATTACHED_TARGET_NAME: record.attached_target_name,
        COLUMN_ATTACHED_COLUMN_NAME: record.attached_column_name,
        COLUMN_TARGET_DATABASE: record.target_database,
        COLUMN_TARGET_SCHEMA: record.target_schema,
        COLUMN_TARGET_NAME: record.target_name,
        COLUMN_SEVERITY: record.severity,
        COLUMN_OUTCOME: record.outcome,
        COLUMN_EXECUTION_ERROR: record.execution_error,
        COLUMN_VIOLATION_COUNT: record.violation_count,
        COLUMN_MEASURED_VALUE: _canonical_float(record.measured_value),
        COLUMN_SAMPLE_COUNT: record.sample_count,
        COLUMN_SAMPLE_UNIT: record.sample_unit,
        COLUMN_MINIMUM_SAMPLES: record.minimum_samples,
        COLUMN_THRESHOLDS_JSON: record.thresholds_json,
        COLUMN_EVIDENCE_JSON: record.evidence_json,
        COLUMN_EVIDENCE_COUNT: record.evidence_count,
        COLUMN_EVIDENCE_TRUNCATED: _text_boolean(record.evidence_truncated),
        COLUMN_EVIDENCE_ERROR: record.evidence_error,
        COLUMN_MEASUREMENT_SQL: record.measurement_sql,
        COLUMN_EVIDENCE_SQL: record.evidence_sql,
        COLUMN_EXECUTED_SQL: record.executed_sql,
        COLUMN_SQL_DIGEST: record.sql_digest,
        COLUMN_METADATA_JSON: record.metadata_json,
        COLUMN_REUSED: _text_boolean(record.reused),
        COLUMN_AUDIT_META_JSON: record.audit_meta_json,
    }
    return (
        "("
        + ", ".join(_column_literal(column=column, value=values[column]) for column in columns)
        + ")"
    )


def _canonical_float(value: float | None) -> str | None:
    return None if value is None else repr(value)


def _text_boolean(value: bool | None) -> str | None:
    if value is None:
        return None
    return "true" if value else "false"


def _column_literal(*, column: str, value: object | None) -> str:
    return render_state_sql_literal(value=value, declared_type=AUDIT_RESULT_COLUMN_TYPES[column])
