"""Export shipped orders to the carrier."""

from python.nodes.fulfillment.shipment_export.main import main
from sqlbuild.assets import AssetContext, asset
from sqlbuild.executor.python_nodes.models import PythonNodeResult


@asset(description="Export shipped orders to the carrier.")
def shipment_export(ctx: AssetContext) -> PythonNodeResult:
    return main(ctx=ctx)
