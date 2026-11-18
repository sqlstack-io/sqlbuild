"""Audit execution result models."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

from sqlbuild._native import CompiledRelationLocation, MeasurementThresholds, SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditEvaluationMode,
    AuditOutcome,
    AuditRunScope,
    AuditSeverity,
)
from sqlbuild.compiler.compile.types import AttachedAuditTargetKind


@dataclass(frozen=True)
class AuditExecutionContext:
    """Warehouse and rendering inputs shared by one audit execution."""

    adapter: BaseAdapter
    connection: Any
    model_locations: dict[str, CompiledRelationLocation]
    seed_locations: dict[str, CompiledRelationLocation]
    source_map: dict[str, SourceEntry]
    relation_overrides: dict[str, str] | None
    run_scope_phase: AuditRunScope


@dataclass(frozen=True)
class AuditResultProjection:
    """Best-effort audit history projection counts for command reporting."""

    attempted_count: int = 0
    written_count: int = 0
    failed_count: int = 0
    failure_reason: str | None = None

    @property
    def degraded(self) -> bool:
        return self.failed_count > 0


@dataclass(frozen=True)
class AuditExecutionResult:
    """Outcome of one audit execution against a built relation."""

    audit_name: str
    audit_definition_name: str
    attachment_kind: AuditAttachmentKind
    severity: AuditSeverity
    outcome: AuditOutcome
    row_count: int
    executed_sql: str
    run_scope_phase: AuditRunScope = AuditRunScope.FINAL
    attached_target_kind: AttachedAuditTargetKind | None = None
    attached_target_name: str | None = None
    attached_column_name: str | None = None
    reused: bool = False
    evaluation_mode: AuditEvaluationMode = AuditEvaluationMode.VIOLATIONS
    measured_value: float | None = None
    sample_count: int | None = None
    sample_unit: str | None = None
    minimum_samples: int | None = None
    thresholds: MeasurementThresholds | None = None
    evidence_rows: tuple[Mapping[str, object], ...] = ()
    evidence_truncated: bool = False
    evidence_error: str | None = None
    evidence_sql: str | None = None
    audit_description: str | None = None
    execution_error: str | None = None
    audit_meta: Mapping[str, object] | None = None
    skip_reason: str | None = None
    ignored: bool = False

    @property
    def blocks(self) -> bool:
        """Whether this failure holds back what depends on it; an ignored failure holds nothing."""

        return self.outcome == AuditOutcome.ERROR and not self.ignored
