"""Negative fixture: a failed declaration import must fail every publication retry."""

from sqlbuild.orchestration import PlatformEventKind, platform_event, sensor

raise RuntimeError("broken declaration")

broken_orders = sensor(
    "broken_orders_sensor",
    description="Never declared: the module fails first.",
    when=platform_event(kinds=[PlatformEventKind.HEARTBEAT]),
    run="orders_job",
)
