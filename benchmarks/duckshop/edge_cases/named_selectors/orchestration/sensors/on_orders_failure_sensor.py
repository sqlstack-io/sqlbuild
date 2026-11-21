"""Starts the repair job when any of four models fails: one over the limit, bound to a name."""

from sqlbuild.orchestration import RunRequest, node_event, sensor

ORDERS_FAILED = node_event(
    select="stg_orders stg_payments int_orders order_totals", kinds=["failure"]
)


@sensor(
    description="Repair orders after a failure",
    when=ORDERS_FAILED,
)
def on_orders_failure_sensor(ctx):
    return RunRequest(job="orders_repair_job")
