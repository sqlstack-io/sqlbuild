"""Polling callbacks return durable run keys and keep their cursor in the native transaction."""

from sqlbuild.orchestration import sensor


@sensor(
    description="Poll fulfillment readiness.",
    minimum_interval_seconds=1,
    timeout_seconds=20,
)
def poll_orders_sensor(ctx, fulfillment):
    def poll():
        return fulfillment.poll(ctx)

    return poll()
