"""A sensor declared for another target is left out of this target's snapshot."""

from sqlbuild.orchestration import PlatformEventKind, platform_event, sensor

other_target_orders = sensor(
    "other_target_orders_sensor",
    description="Refresh another target.",
    when=platform_event(kinds=[PlatformEventKind.HEARTBEAT]),
    run="health_orders_job",
    targets=("production",),
)
