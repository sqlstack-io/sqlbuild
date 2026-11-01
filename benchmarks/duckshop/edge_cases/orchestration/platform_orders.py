"""Sensor-only roots retain referenced jobs while target filters and event enums stay native."""

from sqlbuild.orchestration import PlatformEventKind, job, platform_event, sensor


def definitions():
    refresh = job("health_orders", description="Refresh orders.", select="orders")
    return [
        sensor(
            "overdue_orders",
            description="Catch up overdue order reporting.",
            when=platform_event(kinds=[PlatformEventKind.TICK_OVERDUE]),
            run=refresh,
            minimum_interval_seconds=7,
            default_status="stopped",
            targets=("development",),
        ),
        sensor(
            "other_target_orders",
            description="Refresh another target.",
            when=platform_event(kinds=[PlatformEventKind.HEARTBEAT]),
            run=refresh,
            targets=("production",),
        ),
    ]


triggers = definitions()
alias = triggers[0]
triggers.append(triggers)
