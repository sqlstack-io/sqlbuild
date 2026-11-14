"""Export shipped orders to the carrier."""

from sqlbuild.tasks import task

from python.fulfillment.shipment_export.main import export_shipments


@task
def shipment_export(ctx: object) -> None:
    """Export shipped orders to the carrier."""
    export_shipments(ctx)
