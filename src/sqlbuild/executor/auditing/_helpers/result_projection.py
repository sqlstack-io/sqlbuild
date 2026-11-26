"""Best-effort persistence and lifecycle publication for completed audit batches."""

from __future__ import annotations

import hashlib
import json
import logging
import threading
from collections import defaultdict
from collections.abc import Iterator
from contextlib import contextmanager
from contextvars import Context, ContextVar, Token, copy_context
from datetime import UTC, datetime
from functools import partial
from typing import Any

from sqlbuild._native import (
    AuditIdentity,
    AuditPlanEntry,
    CompiledRelationLocation,
    MeasurementThresholdBound,
    PlanOutput,
    build_audit_gate_identity,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.compiler.auditing.types import AuditEvaluationMode, AuditOutcome, ThresholdOperator
from sqlbuild.executor.audit_results.constants import (
    AUDIT_RESULT_SCHEMA_VERSION,
    IGNORED_FAILURE_METADATA_JSON,
)
from sqlbuild.executor.audit_results.exceptions import AuditResultStorageError
from sqlbuild.executor.audit_results.main._write import write_audit_result_records
from sqlbuild.executor.audit_results.models import (
    AuditResultColumnEvolution,
    AuditResultRecord,
    build_audit_result_id,
)
from sqlbuild.executor.auditing.models import AuditExecutionResult, AuditResultProjection
from sqlbuild.runtime.observability.classes.operation_lifecycle import OperationLifecycle
from sqlbuild.runtime.observability.main.current_execution_identity import (
    current_execution_identity,
)
from sqlbuild.runtime.observability.models import ExecutionIdentity
from sqlbuild.runtime.observability.types import JSONValue

_LOGGER: logging.Logger = logging.getLogger("sqlbuild.execution")
_MAX_FAILURE_REASON_LENGTH: int = 500
_LAST_AUDIT_RESULT_PROJECTION: ContextVar[AuditResultProjection | None] = ContextVar(
    "sqlbuild_last_audit_result_projection", default=None
)


class _AuditResultPublisher:
    """Publish each confirmed audit once, in the captured run context, and keep its record."""

    def __init__(
        self,
        *,
        plan: PlanOutput,
        storage_database: str | None,
        storage_schema: str | None,
    ) -> None:
        self.plan: PlanOutput = plan
        self.identity: ExecutionIdentity | None = current_execution_identity()
        self.database, self.schema = _storage_location(
            plan=plan, database=storage_database, schema=storage_schema
        )
        self._context: Context = copy_context()
        self._lock: threading.Lock = threading.Lock()
        self._occurrence: defaultdict[tuple[str, str], int] = defaultdict(int)
        self._records: dict[int, tuple[AuditExecutionResult, AuditResultRecord | None]] = {}
        self.build_error: Exception | None = None

    @property
    def available(self) -> bool:
        return (
            self.identity is not None
            and self.identity.run_id is not None
            and self.schema is not None
        )

    def publish(self, results: tuple[AuditExecutionResult, ...]) -> None:
        """Build and publish records for newly confirmed, non-reused results."""

        if not self.available:
            return
        with self._lock:
            for result in results:
                if result.reused or id(result) in self._records:
                    continue
                record: AuditResultRecord | None = self._build(result)
                self._records[id(result)] = (result, record)
                if record is not None:
                    not_run_reason: str | None = (
                        result.skip_reason if result.outcome == AuditOutcome.NOT_RUN else None
                    )
                    self._context.run(
                        partial(_publish_safely, record=record, not_run_reason=not_run_reason)
                    )

    def records_for(
        self, results: tuple[AuditExecutionResult, ...]
    ) -> tuple[AuditResultRecord, ...]:
        """Return batch-ordered records, publishing any result not yet confirmed."""

        self.publish(results)
        if self.build_error is not None:
            raise self.build_error
        records: list[AuditResultRecord] = []
        with self._lock:
            for result in results:
                record: AuditResultRecord | None = (
                    None if result.reused else self._records[id(result)][1]
                )
                if record is not None:
                    records.append(record)
        return tuple(records)

    def _build(self, result: AuditExecutionResult) -> AuditResultRecord | None:
        if self.build_error is not None or self.identity is None or self.schema is None:
            return None
        try:
            entry: AuditPlanEntry | None = next(
                (
                    candidate
                    for candidate in self.plan.audit_entries
                    if _matches(entry=candidate, result=result)
                ),
                None,
            )
            if entry is None:
                _LOGGER.warning(
                    "Audit result projection skipped unknown audit result '%s'", result.audit_name
                )
                return None
            audit_id: AuditIdentity = build_audit_gate_identity(audits=(entry,)).audits[0]
            occurrence_key: tuple[str, str] = (audit_id.binding_key, result.run_scope_phase.value)
            ordinal: int = self._occurrence[occurrence_key]
            self._occurrence[occurrence_key] = ordinal + 1
            return _build_record(
                result=result,
                entry=entry,
                audit_id=audit_id,
                ordinal=ordinal,
                plan=self.plan,
                identity=self.identity,
                storage_database=self.database,
                storage_schema=self.schema,
            )
        except Exception as error:
            self.build_error = error
            return None


_ACTIVE_AUDIT_RESULT_PUBLISHER: ContextVar[_AuditResultPublisher | None] = ContextVar(
    "sqlbuild_active_audit_result_publisher", default=None
)


@contextmanager
def audit_result_publication_scope_impl(
    *,
    plan: PlanOutput,
    storage_database: str | None = None,
    storage_schema: str | None = None,
) -> Iterator[None]:
    """Publish audit completions incrementally until the batch projection finalizes them."""

    publisher: _AuditResultPublisher = _AuditResultPublisher(
        plan=plan,
        storage_database=storage_database,
        storage_schema=storage_schema,
    )
    token: Token[_AuditResultPublisher | None] = _ACTIVE_AUDIT_RESULT_PUBLISHER.set(publisher)
    try:
        yield
    finally:
        _ACTIVE_AUDIT_RESULT_PUBLISHER.reset(token)


def publish_completed_audit_results_impl(results: tuple[AuditExecutionResult, ...]) -> None:
    """Publish confirmed audit outcomes through the active scope without raising."""

    publisher: _AuditResultPublisher | None = _ACTIVE_AUDIT_RESULT_PUBLISHER.get()
    if publisher is None:
        return
    try:
        publisher.publish(results)
    except Exception as error:
        _LOGGER.warning("Audit result lifecycle publication degraded (%s)", error)


def current_audit_result_projection_impl() -> AuditResultProjection | None:
    """Return the most recent audit projection report in this execution context."""

    return _LAST_AUDIT_RESULT_PROJECTION.get()


def project_audit_result_batch_impl(
    *,
    plan: PlanOutput,
    results: tuple[AuditExecutionResult, ...],
    adapter: BaseAdapter,
    connection: Any,
    storage_database: str | None = None,
    storage_schema: str | None = None,
) -> AuditResultProjection:
    """Publish and append non-reused audit facts without changing audit outcomes."""

    executed: tuple[AuditExecutionResult, ...] = tuple(
        result for result in results if not result.reused
    )
    if not executed:
        projection: AuditResultProjection = AuditResultProjection()
        _LAST_AUDIT_RESULT_PROJECTION.set(projection)
        return projection
    active: _AuditResultPublisher | None = _ACTIVE_AUDIT_RESULT_PUBLISHER.get()
    publisher: _AuditResultPublisher = (
        active
        if active is not None and active.plan is plan
        else _AuditResultPublisher(
            plan=plan, storage_database=storage_database, storage_schema=storage_schema
        )
    )
    identity: ExecutionIdentity | None = publisher.identity
    if identity is None or identity.run_id is None:
        _LOGGER.warning("Audit result projection skipped because runtime identity is unavailable")
        projection = AuditResultProjection(
            attempted_count=len(executed), failed_count=len(executed)
        )
        _LAST_AUDIT_RESULT_PROJECTION.set(projection)
        return projection
    database: str | None = publisher.database
    schema: str | None = publisher.schema
    if schema is None:
        _LOGGER.warning("Audit result projection skipped because target schema is unavailable")
        projection = AuditResultProjection(
            attempted_count=len(executed), failed_count=len(executed)
        )
        _LAST_AUDIT_RESULT_PROJECTION.set(projection)
        return projection

    try:
        records: tuple[AuditResultRecord, ...] = publisher.records_for(executed)
    except Exception as error:
        _LOGGER.warning(
            "Audit result projection degraded: attempted=%d written=0 failed=%d (%s)",
            len(executed),
            len(executed),
            error,
        )
        projection = AuditResultProjection(
            attempted_count=len(executed), failed_count=len(executed)
        )
        _LAST_AUDIT_RESULT_PROJECTION.set(projection)
        return projection
    unmatched_count: int = len(executed) - len(records)
    if not records:
        projection = AuditResultProjection(
            attempted_count=len(executed), failed_count=len(executed)
        )
        _LAST_AUDIT_RESULT_PROJECTION.set(projection)
        return projection
    try:
        write_audit_result_records(
            connection=connection,
            execute=adapter.execute,
            database=database,
            schema=schema,
            records=records,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
            render_create_table_sql=adapter.render_create_audit_result_table_sql,
            render_create_index_sqls=adapter.render_create_audit_result_index_sqls,
            column_evolution=AuditResultColumnEvolution(
                read_columns=adapter.get_columns, render_add_columns=adapter.render_add_columns
            ),
        )
    except AuditResultStorageError as error:
        reason: str = _storage_failure_reason(error)
        _LOGGER.warning(
            "Audit result persistence degraded: attempted=%d written=0 failed=%d (%s)",
            len(executed),
            len(executed),
            reason,
        )
        projection = AuditResultProjection(
            attempted_count=len(executed), failed_count=len(executed), failure_reason=reason
        )
        _LAST_AUDIT_RESULT_PROJECTION.set(projection)
        return projection
    projection = AuditResultProjection(
        attempted_count=len(executed),
        written_count=len(records),
        failed_count=unmatched_count,
    )
    _LAST_AUDIT_RESULT_PROJECTION.set(projection)
    return projection


def _build_record(  # noqa: PLR0913
    *,
    result: AuditExecutionResult,
    entry: AuditPlanEntry,
    audit_id: AuditIdentity,
    ordinal: int,
    plan: PlanOutput,
    identity: ExecutionIdentity,
    storage_database: str | None,
    storage_schema: str,
) -> AuditResultRecord:
    attempt_key: str = f"{result.run_scope_phase.value}:{ordinal}"
    result_id: str = build_audit_result_id(
        invocation_id=identity.invocation_id,
        run_id=identity.run_id or "",
        binding_key=audit_id.binding_key,
        execution_fingerprint=audit_id.execution_fingerprint,
        run_scope_phase=result.run_scope_phase.value,
        attempt_key=attempt_key,
    )
    target: CompiledRelationLocation | None = (
        plan.model_locations.get(result.attached_target_name)
        if result.attached_target_name is not None
        else None
    )
    thresholds: dict[str, object] | None = _render_thresholds(entry)
    evidence: list[dict[str, object]] = [dict(row) for row in result.evidence_rows]
    return AuditResultRecord(
        result_id=result_id,
        schema_version=AUDIT_RESULT_SCHEMA_VERSION,
        occurred_at=datetime.now(UTC),
        invocation_id=identity.invocation_id,
        run_id=identity.run_id or "",
        audit_name=result.audit_name,
        audit_definition_name=result.audit_definition_name,
        audit_description=result.audit_description,
        audit_meta_json=_json_or_none(result.audit_meta),
        binding_key=audit_id.binding_key,
        definition_fingerprint=audit_id.definition_fingerprint,
        execution_fingerprint=audit_id.execution_fingerprint,
        evaluation_mode=result.evaluation_mode.value,
        run_scope_phase=result.run_scope_phase.value,
        attachment_kind=result.attachment_kind.value,
        attached_target_kind=(
            None if result.attached_target_kind is None else result.attached_target_kind.value
        ),
        attached_target_name=result.attached_target_name,
        attached_column_name=result.attached_column_name,
        target_database=(target.database if target is not None else storage_database),
        target_schema=(target.schema if target is not None else storage_schema),
        target_name=(target.name if target is not None else result.attached_target_name),
        severity=result.severity.value,
        outcome=result.outcome.value,
        execution_error=result.execution_error,
        violation_count=(
            result.row_count if result.evaluation_mode == AuditEvaluationMode.VIOLATIONS else None
        ),
        measured_value=result.measured_value,
        sample_count=result.sample_count,
        sample_unit=result.sample_unit,
        minimum_samples=result.minimum_samples,
        thresholds_json=_json_or_none(thresholds),
        evidence_json=_json_or_none(evidence) if evidence else None,
        evidence_count=len(evidence),
        evidence_truncated=result.evidence_truncated,
        evidence_error=result.evidence_error,
        measurement_sql=(
            result.executed_sql
            if result.evaluation_mode == AuditEvaluationMode.MEASUREMENT
            else None
        ),
        evidence_sql=result.evidence_sql,
        executed_sql=result.executed_sql,
        sql_digest=hashlib.sha256(result.executed_sql.encode("utf-8")).hexdigest(),
        metadata_json=IGNORED_FAILURE_METADATA_JSON if result.ignored else None,
        reused=False,
    )


def _publish_safely(*, record: AuditResultRecord, not_run_reason: str | None) -> None:
    try:
        _publish_audit_completed(record=record, not_run_reason=not_run_reason)
    except Exception as error:
        _LOGGER.warning(
            "Audit result lifecycle publication degraded for '%s' (%s)",
            record.audit_name,
            error,
        )


def _publish_audit_completed(*, record: AuditResultRecord, not_run_reason: str | None) -> None:
    payload: dict[str, JSONValue] = {
        "audit_name": record.audit_name,
        "audit_definition_name": record.audit_definition_name,
        "evaluation_mode": record.evaluation_mode,
        "outcome": record.outcome,
        "severity": record.severity,
        "run_scope_phase": record.run_scope_phase,
        "attachment_kind": record.attachment_kind,
        "binding_key": record.binding_key,
        "definition_fingerprint": record.definition_fingerprint,
        "execution_fingerprint": record.execution_fingerprint,
        "evidence_count": record.evidence_count or 0,
        "evidence_truncated": bool(record.evidence_truncated),
        "executed_sql": record.executed_sql,
        "sql_digest": record.sql_digest,
        "reused": False,
        "result_id": record.result_id,
    }
    if record.metadata_json == IGNORED_FAILURE_METADATA_JSON:
        payload["ignored"] = True
    if not_run_reason:
        payload["not_run_reason"] = not_run_reason
    optional: dict[str, JSONValue | None] = {
        "attached_target_kind": record.attached_target_kind,
        "attached_target_name": record.attached_target_name,
        "attached_column_name": record.attached_column_name,
        "target_database": record.target_database,
        "target_schema": record.target_schema,
        "target_name": record.target_name,
        "violation_count": record.violation_count,
        "measured_value": record.measured_value,
        "sample_count": record.sample_count,
        "sample_unit": record.sample_unit,
        "minimum_samples": record.minimum_samples,
        "thresholds": json.loads(record.thresholds_json) if record.thresholds_json else None,
        "evidence": json.loads(record.evidence_json) if record.evidence_json else None,
        "evidence_error": record.evidence_error,
        "measurement_sql": record.measurement_sql,
        "evidence_sql": record.evidence_sql,
        "execution_error": record.execution_error,
        "audit_description": record.audit_description,
        "audit_meta": json.loads(record.audit_meta_json) if record.audit_meta_json else None,
    }
    payload.update({key: value for key, value in optional.items() if value is not None})
    OperationLifecycle.publish_audit_completed(payload=payload)


def _render_thresholds(entry: AuditPlanEntry) -> dict[str, object] | None:
    if entry.thresholds is None:
        return None
    rendered: dict[str, object] = {}
    for name, bound in (("warn", entry.thresholds.warn), ("error", entry.thresholds.error)):
        if bound is not None:
            rendered[name] = _render_bound(bound)
    return rendered


def _render_bound(bound: MeasurementThresholdBound) -> dict[str, object]:
    if bound.operator == ThresholdOperator.OUTSIDE:
        return {"operator": bound.operator.value, "lower": bound.lower, "upper": bound.upper}
    return {"operator": bound.operator.value, "limit": bound.limit}


def _storage_failure_reason(error: AuditResultStorageError) -> str:
    """The storage error with the warehouse error that caused it, on one bounded line."""

    cause: BaseException | None = error.__cause__
    text: str = str(error) if cause is None else f"{error}: {cause}"
    line: str = " ".join(text.split())
    return line[:_MAX_FAILURE_REASON_LENGTH]


def _json_or_none(value: object | None) -> str | None:
    return None if value is None else json.dumps(value, sort_keys=True, separators=(",", ":"))


def _matches(*, entry: AuditPlanEntry, result: AuditExecutionResult) -> bool:
    return (
        entry.name == result.audit_name
        and entry.attachment_kind == result.attachment_kind
        and entry.attached_target_name == result.attached_target_name
        and entry.attached_column_name == result.attached_column_name
    )


def _storage_location(
    *, plan: PlanOutput, database: str | None, schema: str | None
) -> tuple[str | None, str | None]:
    if schema is not None:
        return database, schema
    location: CompiledRelationLocation
    for location in (*plan.model_locations.values(), *plan.seed_locations.values()):
        if location.schema is not None:
            return database if database is not None else location.database, location.schema
    return database, None
