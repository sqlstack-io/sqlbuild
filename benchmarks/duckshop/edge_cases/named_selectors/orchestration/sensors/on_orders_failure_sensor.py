"""Starts the repair job when any of four listed models fails: one over the limit."""

from sqlbuild.orchestration import node_event, sensor

on_orders_failure = sensor(
    "on_orders_failure_sensor",
    description="Repair orders after a failure",
    when=node_event(select="stg_orders stg_payments int_orders order_totals", kinds=["failure"]),
    run="orders_repair_job",
)
