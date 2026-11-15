"""Refreshes totals when orders land: a condition imported from a project module, over the limit."""

from orchestration.sensors._orders_conditions import ORDERS_LANDED

from sqlbuild.orchestration import sensor

on_orders_landed = sensor(
    "on_orders_landed_sensor",
    description="Refresh order totals after orders land",
    when=ORDERS_LANDED,
    run="orders_folder_job",
)
