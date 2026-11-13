"""A sensor on new source data starts a job that builds only what changed, source data included."""

from sqlbuild.orchestration import job, sensor, source_new_data

orders_refresh = job(
    "orders_refresh",
    description="Refresh orders when the source has new data.",
    select="orders",
    settings={"changed": True},
)
on_source_orders = sensor(
    "on_source_orders",
    description="Refresh orders for each changed source version.",
    when=source_new_data(select="source:raw_orders"),
    run=orders_refresh,
    minimum_interval_seconds=1,
)
