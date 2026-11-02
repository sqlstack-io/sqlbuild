"""Consume changed warehouse source versions without treating a sync poll as new data."""

from sqlbuild.orchestration import job, sensor, source_new_data

orders_refresh = job(
    "orders_refresh",
    description="Refresh orders for a changed source version.",
    select="orders",
    overlap="allow",
)
on_source_orders = sensor(
    "on_source_orders",
    description="Refresh orders for the first source version and each changed version.",
    when=source_new_data(select="source:raw_orders"),
    run=orders_refresh,
    minimum_interval_seconds=1,
)
