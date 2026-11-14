"""A task whose Python upstream another job builds, so a job group must order the two jobs."""

from python.orders.partner_export.node import partner_orders_file

from sqlbuild.tasks import task


@task(depends_on=partner_orders_file, tags=["orders"], description="Book collection.")
def request_carrier_collection(ctx):
    return None
