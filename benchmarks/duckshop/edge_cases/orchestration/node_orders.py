"""Trigger on each real orders rebuild, not on freshness-only success."""

from sqlbuild.orchestration import NodeEventKind, job, node_event, sensor

source_orders = job("source_orders", description="Source orders.", select="orders")
inventory = job("inventory", description="Destination inventory.", select="stg_orders")
on_orders = sensor(
    "on_orders",
    description="Build inventory for every genuine orders rebuild.",
    when=node_event(select="orders orders*", kinds=[NodeEventKind.SUCCESS]),
    run=inventory,
)
