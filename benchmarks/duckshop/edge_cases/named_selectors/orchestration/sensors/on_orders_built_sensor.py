"""Refreshes totals after one model: a condition a function builds, which cannot be counted."""

from orchestration.sensors._orders_conditions import landed_after

from sqlbuild.orchestration import sensor

on_orders_built = sensor(
    "on_orders_built_sensor",
    description="Refresh order totals after the totals model",
    when=landed_after("order_totals"),
    run="orders_folder_job",
)
