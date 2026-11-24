"""Audit cases every shipment table shares."""

from sqlbuild.audits import AuditCase, audit_factory


@audit_factory
def shipment_quality() -> list[AuditCase]:
    """Every shipment has a positive identifier."""
    return [
        AuditCase(
            name="positive_shipment_id",
            definition="expression_is_true",
            arguments={"expression": "shipment_id > 0"},
        )
    ]
