"""Start the changed-only refresh for each new source version."""

from sqlbuild.orchestration import sensor, source_new_data

on_source_orders = sensor(
    "on_source_orders_sensor",
    description="Refresh orders for each changed source version.",
    when=source_new_data(select="source:raw_orders"),
    run="orders_refresh_job",
    minimum_interval_seconds=1,
)
