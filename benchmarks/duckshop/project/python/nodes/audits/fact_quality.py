"""Audit cases shared by every fact table."""

from sqlbuild.audits import AuditCase, AuditSeverity, audit_factory


@audit_factory
def fact_table_quality() -> list[AuditCase]:
    """A fact table must never publish empty."""
    return [
        AuditCase(
            name="fact_table_has_rows",
            definition="has_rows",
            arguments={"minimum_rows": 1},
            severity=AuditSeverity.WARN,
            description="A fact table built with no rows usually means an upstream load failed.",
            meta={"check": "volume"},
        )
    ]
