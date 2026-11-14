"""Upcoming fulfillment times from the provider, evaluated in the code location."""

from sqlbuild.orchestration import calendar


@calendar(
    name="fulfillment_calendar",
    description="Refresh orders at upcoming fulfillment times.",
    horizon="8s",
)
def fulfillment_calendar(window, fulfillment):
    return fulfillment.upcoming(window)
