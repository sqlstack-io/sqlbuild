"""A condition whose select is not a string literal cannot be counted and is reported."""

from sqlbuild.orchestration import RunRequest, node_event, sensor

ORDERS_SELECTION = "stg_orders"


@sensor(
    description="Refresh totals when staged orders change",
    when=node_event(select=ORDERS_SELECTION, kinds=["success"]),
)
def on_orders_computed_sensor(ctx):
    return RunRequest(job="orders_folder_job")
