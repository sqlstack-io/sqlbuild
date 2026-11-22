"""Start the changed-only refresh for each new source version."""

from sqlbuild.orchestration import RunRequest, sensor, source_new_data


@sensor(
    description="Refresh orders for each changed source version.",
    when=source_new_data(select="source:raw_orders"),
    minimum_interval_seconds=1,
)
def on_source_orders_sensor(ctx):
    return RunRequest(job="orders_refresh_job")
