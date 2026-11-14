"""Polling callbacks return durable run keys and keep their cursor in the native transaction."""

from sqlbuild.orchestration import sensor


@sensor(
    "poll_orders_sensor",
    description="Poll fulfillment readiness.",
    run="annual_orders_job",
    minimum_interval_seconds=1,
)
def poll_orders(ctx, fulfillment):
    def poll():
        return fulfillment.poll(ctx)

    return poll()
