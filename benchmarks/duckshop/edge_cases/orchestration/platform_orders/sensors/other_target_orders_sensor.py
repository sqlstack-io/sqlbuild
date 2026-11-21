"""A sensor declared for another target is left out of this target's snapshot."""

from sqlbuild.orchestration import PlatformEventKind, RunRequest, platform_event, sensor


@sensor(
    description="Refresh another target.",
    when=platform_event(kinds=[PlatformEventKind.HEARTBEAT]),
    targets=("production",),
)
def other_target_orders_sensor(ctx):
    return RunRequest(job="health_orders_job")
