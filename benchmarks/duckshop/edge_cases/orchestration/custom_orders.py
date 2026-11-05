"""Polling callbacks return durable run keys and keep their cursor in the native transaction."""

from sqlbuild.orchestration import job, sensor

orders = job("annual_orders", description="Build orders after source inspection.", select="orders")


@sensor(
    "poll_orders", description="Poll fulfillment readiness.", run=orders, minimum_interval_seconds=1
)
def poll_orders(ctx, fulfillment):
    def poll():
        return fulfillment.poll(ctx)

    return poll()
