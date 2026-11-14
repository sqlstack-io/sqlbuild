"""Export shipped orders to the carrier."""

from python.fulfillment.shipment_export.main import export_shipments
from sqlbuild.tasks import task


@task
def shipment_export(ctx: object) -> None:
    """Export shipped orders to the carrier."""
    export_shipments(ctx)
