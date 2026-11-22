"""Trigger on each real orders rebuild, not on freshness-only success."""

from sqlbuild.orchestration import NodeEventKind, RunRequest, node_event, sensor


@sensor(
    description="Build inventory for every genuine orders rebuild.",
    when=node_event(select="orders orders*", kinds=[NodeEventKind.SUCCESS]),
)
def on_orders_sensor(ctx):
    return RunRequest(job="inventory_job")
