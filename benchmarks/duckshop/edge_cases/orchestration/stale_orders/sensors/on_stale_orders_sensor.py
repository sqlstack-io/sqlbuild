"""Start one refresh for each timestamp source version older than a minute."""

from sqlbuild.orchestration import RunRequest, sensor, source_stale


@sensor(
    description="Refresh orders once for each timestamp version older than one minute.",
    when=source_stale(select="source:raw_orders", after_seconds=60),
    minimum_interval_seconds=1,
)
def on_stale_orders_sensor(ctx):
    return RunRequest(job="orders_refresh_job")
