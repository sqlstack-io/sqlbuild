"""Start one refresh for each timestamp source version older than a minute."""

from sqlbuild.orchestration import sensor, source_stale

on_stale_orders = sensor(
    "on_stale_orders_sensor",
    description="Refresh orders once for each timestamp version older than one minute.",
    when=source_stale(select="source:raw_orders", after_seconds=60),
    run="orders_refresh_job",
    minimum_interval_seconds=1,
)
