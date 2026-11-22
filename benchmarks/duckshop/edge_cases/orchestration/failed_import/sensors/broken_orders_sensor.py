"""Negative fixture: a failed declaration import must fail every publication retry."""

from sqlbuild.orchestration import PlatformEventKind, RunRequest, platform_event, sensor

raise RuntimeError("broken declaration")


@sensor(
    description="Never declared: the module fails first.",
    when=platform_event(kinds=[PlatformEventKind.HEARTBEAT]),
)
def broken_orders_sensor(ctx):
    return RunRequest(job="orders_job")
