"""A sensor in its domain folder reads its selection from a private support folder."""

from orchestration.sensors.inventory._helpers.selection import ORDERS_SELECTION
from sqlbuild.orchestration import NodeEventKind, node_event, sensor

on_orders = sensor(
    "on_orders_sensor",
    description="Build inventory for every genuine orders rebuild.",
    when=node_event(select=ORDERS_SELECTION, kinds=[NodeEventKind.SUCCESS]),
    run="inventory_job",
)
