"""Audit execution within build and model lifecycle."""

from __future__ import annotations

import json
import math
from collections.abc import Mapping
from dataclasses import replace
from numbers import Number
from typing import Any, cast

from sqlbuild._native import (
    AuditPlanEntry,
    CompiledRelationLocation,
    CostContext,
    SourceEntry,
    assert_no_unresolved_sql_markers,
    evaluate_measurement,
    render_audit_sql,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.compiler.auditing.types import (
    AuditEvaluationMode,
    AuditOutcome,
    AuditRunScope,
)
from sqlbuild.diagnostics.main.diagnostics_context import diagnostics_context
from sqlbuild.executor.auditing._helpers.evidence_setting import keeps_audit_evidence
from sqlbuild.executor.auditing.constants import (
    DEFAULT_EVIDENCE_ROW_LIMIT,
    MAX_EVIDENCE_SERIALIZED_BYTES,
)
from sqlbuild.executor.auditing.exceptions import AuditMeasurementExecutionError
from sqlbuild.executor.auditing.models import AuditExecutionContext, AuditExecutionResult
from sqlbuild.runtime.observability.classes.operation_lifecycle import OperationLifecycle
from sqlbuild.runtime.observability.models import OperationAttributes


def execute_audit_impl(  # noqa: PLR0913
    *,
    audit: AuditPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    model_locations: dict[str, CompiledRelationLocation],
    seed_locations: dict[str, CompiledRelationLocation],
    source_map: dict[str, SourceEntry],
    relation_overrides: dict[str, str] | None,
    run_scope_phase: AuditRunScope,
    quality_scope: str | None = None,
) -> AuditExecutionResult:
    """Execute and evaluate one audit, returning quality failure as result data."""

    context: AuditExecutionContext = AuditExecutionContext(
        adapter=adapter,
        connection=connection,
        model_locations=model_locations,
        seed_locations=seed_locations,
        source_map=source_map,
        relation_overrides=relation_overrides,
        run_scope_phase=run_scope_phase,
    )
    with OperationLifecycle(
        operation_kind="quality",
        operation_name="audit_evaluation",
        attributes=OperationAttributes(
            phase="evaluate",
            target_kind="audit",
            scope=quality_scope or audit.attachment_kind.value,
        ),
    ) as lifecycle:
        executed_sql: str = _render_sql(
            resolved_sql=audit.resolved_sql,
            unresolved_sql=audit.unresolved_sql,
            audit=audit,
            adapter=adapter,
            model_locations=model_locations,
            seed_locations=seed_locations,
            source_map=source_map,
            relation_overrides=relation_overrides,
            context="executable SQL",
        )
        cursor: Any = _execute_sql(
            sql=executed_sql,
            audit=audit,
            adapter=adapter,
            connection=connection,
            run_scope_phase=run_scope_phase,
        )
        rows: list[Any] = cursor.fetchall()

        if audit.evaluation_mode == AuditEvaluationMode.MEASUREMENT:
            result: AuditExecutionResult = _measurement_result(
                audit=audit,
                cursor=cursor,
                rows=rows,
                executed_sql=executed_sql,
                context=context,
            )
        else:
            row_count: int = len(rows)
            outcome: AuditOutcome = (
                AuditOutcome.PASS if row_count == 0 else AuditOutcome(audit.severity.value)
            )
            result = _base_result(
                audit=audit,
                outcome=outcome,
                row_count=row_count,
                executed_sql=executed_sql,
                run_scope_phase=run_scope_phase,
            )
            if keeps_audit_evidence(outcome=outcome):
                evidence_rows, evidence_truncated = _bounded_evidence(
                    cursor=cursor, raw_rows=rows, row_limit=_evidence_row_limit(audit)
                )
                result = replace(
                    result, evidence_rows=evidence_rows, evidence_truncated=evidence_truncated
                )
        lifecycle.completed()
        return result


def failed_audit_result(
    *, audit: AuditPlanEntry, error: Exception, run_scope_phase: AuditRunScope
) -> AuditExecutionResult:
    """Return an error result for an audit whose execution raised."""

    return replace(
        _base_result(
            audit=audit,
            outcome=AuditOutcome.ERROR,
            row_count=0,
            executed_sql=audit.resolved_sql,
            run_scope_phase=run_scope_phase,
        ),
        execution_error=str(error),
    )


def _measurement_result(  # noqa: PLR0913
    *,
    audit: AuditPlanEntry,
    cursor: Any,
    rows: list[Any],
    executed_sql: str,
    context: AuditExecutionContext,
) -> AuditExecutionResult:
    error: str | None = None
    measured_value: float | None = None
    sample_count: int | None = None
    if len(rows) != 1:
        error = f"measurement audit expected exactly one row but returned {len(rows)}"
    elif audit.value_column is None or audit.thresholds is None:
        error = "measurement audit is missing its compiled value column or thresholds"
    else:
        row: dict[str, object] = _row_mapping(cursor=cursor, row=rows[0])
        if audit.sample_count_column is not None:
            sample_count, error = _sample_count(row=row, column=audit.sample_count_column)
        if error is None:
            raw_value, value_found = _case_insensitive_value(row=row, column=audit.value_column)
            if not value_found:
                error = f"measurement audit result is missing value column '{audit.value_column}'"
            elif (
                raw_value is None
                and audit.minimum_samples is not None
                and (sample_count is None or sample_count < audit.minimum_samples)
            ):
                return replace(
                    _base_result(
                        audit=audit,
                        outcome=AuditOutcome.INSUFFICIENT,
                        row_count=0,
                        executed_sql=executed_sql,
                        run_scope_phase=context.run_scope_phase,
                    ),
                    sample_count=sample_count,
                )
            else:
                measured_value, error = _measurement_value(row=row, column=audit.value_column)

    if error is not None:
        return replace(
            _base_result(
                audit=audit,
                outcome=AuditOutcome.ERROR,
                row_count=0,
                executed_sql=executed_sql,
                run_scope_phase=context.run_scope_phase,
            ),
            measured_value=measured_value,
            sample_count=sample_count,
            execution_error=error,
        )

    if measured_value is None or audit.thresholds is None:
        raise AuditMeasurementExecutionError("validated measurement contract is unavailable")
    outcome: AuditOutcome = evaluate_measurement(
        measured_value=measured_value,
        sample_count=sample_count,
        minimum_samples=audit.minimum_samples,
        thresholds=audit.thresholds,
    )
    evidence_rows: tuple[Mapping[str, object], ...] = ()
    evidence_truncated: bool = False
    evidence_error: str | None = None
    evidence_sql: str | None = None
    if (
        outcome in {AuditOutcome.WARN, AuditOutcome.ERROR}
        and audit.evidence_resolved_sql is not None
    ):
        evidence_sql = _render_sql(
            resolved_sql=audit.evidence_resolved_sql,
            unresolved_sql=audit.evidence_unresolved_sql or audit.evidence_resolved_sql,
            audit=audit,
            adapter=context.adapter,
            model_locations=context.model_locations,
            seed_locations=context.seed_locations,
            source_map=context.source_map,
            relation_overrides=context.relation_overrides,
            context="evidence SQL",
        )
        try:
            evidence_cursor: Any = _execute_sql(
                sql=evidence_sql,
                audit=audit,
                adapter=context.adapter,
                connection=context.connection,
                run_scope_phase=context.run_scope_phase,
            )
            row_limit: int = _evidence_row_limit(audit)
            fetchmany: object = getattr(evidence_cursor, "fetchmany", None)
            evidence_rows, evidence_truncated = _bounded_evidence(
                cursor=evidence_cursor,
                raw_rows=list(
                    fetchmany(row_limit + 1) if callable(fetchmany) else evidence_cursor.fetchall()
                ),
                row_limit=row_limit,
            )
        except Exception as caught_error:
            evidence_error = f"{type(caught_error).__name__}: {caught_error}"

    return replace(
        _base_result(
            audit=audit,
            outcome=outcome,
            row_count=0,
            executed_sql=executed_sql,
            run_scope_phase=context.run_scope_phase,
        ),
        measured_value=measured_value,
        sample_count=sample_count,
        evidence_rows=evidence_rows,
        evidence_truncated=evidence_truncated,
        evidence_error=evidence_error,
        evidence_sql=evidence_sql,
    )


def _base_result(
    *,
    audit: AuditPlanEntry,
    outcome: AuditOutcome,
    row_count: int,
    executed_sql: str,
    run_scope_phase: AuditRunScope,
) -> AuditExecutionResult:
    return AuditExecutionResult(
        audit_name=audit.name,
        audit_definition_name=audit.definition_name,
        audit_description=audit.description,
        audit_meta=audit.meta,
        attachment_kind=audit.attachment_kind,
        severity=audit.severity,
        outcome=outcome,
        row_count=row_count,
        executed_sql=executed_sql,
        run_scope_phase=run_scope_phase,
        attached_target_kind=audit.attached_target_kind,
        attached_target_name=audit.attached_target_name,
        attached_column_name=audit.attached_column_name,
        evaluation_mode=audit.evaluation_mode,
        sample_unit=audit.sample_unit,
        minimum_samples=audit.minimum_samples,
        thresholds=audit.thresholds,
        ignored=audit.ignore_failure and outcome == AuditOutcome.ERROR,
    )


def _render_sql(  # noqa: PLR0913
    *,
    resolved_sql: str,
    unresolved_sql: str,
    audit: AuditPlanEntry,
    adapter: BaseAdapter,
    model_locations: dict[str, CompiledRelationLocation],
    seed_locations: dict[str, CompiledRelationLocation],
    source_map: dict[str, SourceEntry],
    relation_overrides: dict[str, str] | None,
    context: str,
) -> str:
    sql: str = (
        resolved_sql
        if relation_overrides is None
        else render_audit_sql(
            unresolved_sql=unresolved_sql,
            model_locations=model_locations,
            seed_locations=seed_locations,
            source_map=source_map,
            adapter=adapter,
            relation_overrides=relation_overrides,
        )
    )
    _ = assert_no_unresolved_sql_markers(sql=sql, context=f"audit '{audit.name}' {context}")
    return sql


def _execute_sql(
    *,
    sql: str,
    audit: AuditPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    run_scope_phase: AuditRunScope,
) -> Any:
    resource_name: str = (
        audit.name
        if audit.attached_target_name is None
        else f"{audit.attached_target_name}.{audit.name}"
    )
    with CostContext.resource_scope(
        resource_type="audit",
        resource_name=resource_name,
        phase=f"{audit.attachment_kind.value}_audit_{run_scope_phase.value}",
    ):
        with diagnostics_context(
            sqlbuild_phase="audit",
            sqlbuild_audit_name=audit.name,
            sqlbuild_column_name=audit.attached_column_name,
        ):
            return adapter.execute(connection=connection, sql=sql)


def _row_mapping(*, cursor: Any, row: Any) -> dict[str, object]:
    if isinstance(row, Mapping):
        return {str(key): value for key, value in row.items()}
    description: object = getattr(cursor, "description", None)
    if not description:
        return {}
    names: list[str] = [str(column[0]) for column in description]
    return dict(zip(names, row, strict=False))


def _case_insensitive_value(*, row: Mapping[str, object], column: str) -> tuple[object, bool]:
    expected: str = column.casefold()
    for name, value in row.items():
        if name.casefold() == expected:
            return value, True
    return None, False


def _measurement_value(
    *, row: Mapping[str, object], column: str
) -> tuple[float | None, str | None]:
    value, found = _case_insensitive_value(row=row, column=column)
    if not found:
        return None, f"measurement audit result is missing value column '{column}'"
    if value is None:
        return None, f"measurement value column '{column}' must not be NULL"
    if isinstance(value, bool) or not isinstance(value, Number):
        return None, f"measurement value column '{column}' must be numeric"
    try:
        measured_value: float = float(cast(Any, value))
    except (TypeError, ValueError, OverflowError):
        return None, f"measurement value column '{column}' must be numeric"
    if not math.isfinite(measured_value):
        return None, f"measurement value column '{column}' must be finite"
    return measured_value, None


def _sample_count(*, row: Mapping[str, object], column: str) -> tuple[int | None, str | None]:
    value, found = _case_insensitive_value(row=row, column=column)
    if not found:
        return None, f"measurement audit result is missing sample count column '{column}'"
    if value is None:
        return None, None
    if isinstance(value, bool) or not isinstance(value, Number):
        return None, f"measurement sample count column '{column}' must be a non-negative integer"
    try:
        numeric: float = float(cast(Any, value))
    except (TypeError, ValueError, OverflowError):
        return None, f"measurement sample count column '{column}' must be a non-negative integer"
    if not math.isfinite(numeric) or numeric < 0 or not numeric.is_integer():
        return None, f"measurement sample count column '{column}' must be a non-negative integer"
    return int(numeric), None


def _evidence_row_limit(audit: AuditPlanEntry) -> int:
    return DEFAULT_EVIDENCE_ROW_LIMIT if audit.evidence_limit is None else audit.evidence_limit


def _bounded_evidence(
    *, cursor: Any, raw_rows: list[Any], row_limit: int
) -> tuple[tuple[Mapping[str, object], ...], bool]:
    """The leading rows within the row limit and serialized byte cap, and whether rows were cut."""

    truncated: bool = len(raw_rows) > row_limit
    retained: list[Mapping[str, object]] = []
    serialized_bytes: int = len(b"[]")
    for raw_row in raw_rows[:row_limit]:
        safe_row: Mapping[str, object] = {
            key: _json_safe(value)
            for key, value in _row_mapping(cursor=cursor, row=raw_row).items()
        }
        row_bytes: int = len(
            json.dumps(safe_row, sort_keys=True, separators=(",", ":")).encode("utf-8")
        )
        separator_bytes: int = len(b",") if retained else 0
        if serialized_bytes + separator_bytes + row_bytes > MAX_EVIDENCE_SERIALIZED_BYTES:
            truncated = True
            break
        serialized_bytes += separator_bytes + row_bytes
        retained.append(safe_row)
    return tuple(retained), truncated


def _json_safe(value: object) -> object:
    if isinstance(value, float) and not math.isfinite(value):
        return str(value)
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    return str(value)
