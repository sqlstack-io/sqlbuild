"""Trigger on each real orders rebuild, not on freshness-only success."""

from sqlbuild.orchestration import NodeEventKind, node_event, sensor

on_orders = sensor(
    "on_orders_sensor",
    description="Build inventory for every genuine orders rebuild.",
    when=node_event(select="orders orders*", kinds=[NodeEventKind.SUCCESS]),
    run="inventory_job",
)
