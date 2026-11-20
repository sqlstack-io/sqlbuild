"""A decorated sensor that is aliased and held in a cyclic container is found once."""

from sqlbuild.orchestration import PlatformEventKind, RunRequest, platform_event, sensor


@sensor(
    description="Catch up overdue order reporting.",
    when=platform_event(kinds=[PlatformEventKind.TICK_OVERDUE]),
    minimum_interval_seconds=7,
    default_status="paused",
    targets=("development",),
)
def overdue_orders_sensor(ctx):
    return RunRequest(job="health_orders_job")


triggers = [overdue_orders_sensor]
alias = triggers[0]
triggers.append(triggers)
