"""Export shipped orders to the carrier."""

from python.fulfillment.shipment_export.main import main
from sqlbuild.executor.python_nodes.models import PythonNodeResult
from sqlbuild.tasks import TaskContext, task


@task(description="Export shipped orders to the carrier.")
def shipment_export(ctx: TaskContext) -> PythonNodeResult:
    return main(ctx=ctx)
