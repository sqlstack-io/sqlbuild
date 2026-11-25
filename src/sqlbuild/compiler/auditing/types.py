"""Auditing domain types."""

from __future__ import annotations

from enum import StrEnum


class AuditSeverity(StrEnum):
    WARN = "warn"
    ERROR = "error"


class AuditRunScope(StrEnum):
    FINAL = "final"
    DELTA_AND_FINAL = "delta_and_final"


class AuditAttachmentKind(StrEnum):
    SOURCE = "source"
    SEED = "seed"
    MODEL = "model"
    END = "end"


class AuditEvaluationMode(StrEnum):
    VIOLATIONS = "violations"
    MEASUREMENT = "measurement"


class ThresholdOperator(StrEnum):
    BELOW = "below"
    ABOVE = "above"
    OUTSIDE = "outside"


class AuditOutcome(StrEnum):
    PASS = "pass"
    WARN = "warn"
    ERROR = "error"
    INSUFFICIENT = "insufficient"
    SKIPPED = "skipped"
    NOT_RUN = "not_run"
