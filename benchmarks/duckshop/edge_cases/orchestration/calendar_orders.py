"""Provider-backed fulfillment times evaluated in the code location, never by the scheduler."""

from sqlbuild.orchestration import calendar_schedule, job

orders = job("orders", description="Build order reporting.", select="orders")


@calendar_schedule(
    job=orders,
    description="Refresh orders at upcoming fulfillment times.",
    horizon="8s",
    timezone="America/New_York",
)
def fulfillment_calendar(window, fulfillment):
    return fulfillment.upcoming(window)
