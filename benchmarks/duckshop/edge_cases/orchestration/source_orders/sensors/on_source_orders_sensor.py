"""Consume changed warehouse source versions without treating a sync poll as new data."""

from sqlbuild.orchestration import RunRequest, sensor, source_new_data


@sensor(
    description="Refresh orders for the first source version and each changed version.",
    when=source_new_data(select="source:raw_orders"),
    minimum_interval_seconds=1,
)
def on_source_orders_sensor(ctx):
    return RunRequest(job="orders_refresh_job")
