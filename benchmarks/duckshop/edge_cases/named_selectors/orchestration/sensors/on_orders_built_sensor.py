"""Refreshes totals after one model: a condition a function builds, which cannot be counted."""

from orchestration.sensors._orders_conditions import landed_after

from sqlbuild.orchestration import RunRequest, sensor


@sensor(
    description="Refresh order totals after the totals model",
    when=landed_after("order_totals"),
)
def on_orders_built_sensor(ctx):
    return RunRequest(job="orders_folder_job")
