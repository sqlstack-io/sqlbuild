"""A sensor returned from a factory, aliased and held in a cyclic container, is found once."""

from sqlbuild.orchestration import PlatformEventKind, platform_event, sensor


def definitions():
    return [
        sensor(
            "overdue_orders_sensor",
            description="Catch up overdue order reporting.",
            when=platform_event(kinds=[PlatformEventKind.TICK_OVERDUE]),
            run="health_orders_job",
            minimum_interval_seconds=7,
            default_status="paused",
            targets=("development",),
        )
    ]


triggers = definitions()
alias = triggers[0]
triggers.append(triggers)
