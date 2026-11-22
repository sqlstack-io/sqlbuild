"""Refreshes totals when orders land: a condition imported from a project module, over the limit."""

from orchestration.sensors._orders_conditions import ORDERS_LANDED

from sqlbuild.orchestration import RunRequest, sensor


@sensor(
    description="Refresh order totals after orders land",
    when=ORDERS_LANDED,
)
def on_orders_landed_sensor(ctx):
    return RunRequest(job="orders_folder_job")
