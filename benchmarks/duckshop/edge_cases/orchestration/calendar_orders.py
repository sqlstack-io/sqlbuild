"""Provider-backed fulfillment times evaluated in the code location, never by the scheduler."""

from sqlbuild.orchestration import calendar_schedule, job

orders = job("order_reporting", description="Build order reporting.", select="orders")


@calendar_schedule(
    job=orders,
    description="Refresh orders at upcoming fulfillment times.",
    horizon="8s",
    timezone="America/New_York",
)
def fulfillment_calendar(window, fulfillment):
    return fulfillment.upcoming(window)


# Declared for another target without decorating a function: development publication skips it.
production_calendar = calendar_schedule(
    job=orders,
    description="Production fulfillment times, not yet attached to a provider.",
    horizon="8s",
    timezone="UTC",
    targets=("production",),
)
