"""A condition whose select is not a string literal cannot be counted and is reported."""

from sqlbuild.orchestration import node_event, sensor

ORDERS_SELECTION = "stg_orders"

on_orders_computed = sensor(
    "on_orders_computed_sensor",
    description="Refresh totals when staged orders change",
    when=node_event(select=ORDERS_SELECTION, kinds=["success"]),
    run="orders_folder_job",
)
