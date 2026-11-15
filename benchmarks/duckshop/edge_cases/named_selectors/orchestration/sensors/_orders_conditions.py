"""Conditions the orders sensors import: one over the limit, a platform event and a builder."""

from sqlbuild.orchestration import PlatformEventKind, node_event, platform_event

ORDERS_LANDED = node_event(
    select="stg_orders stg_payments int_orders order_totals", kinds=["success"]
)
TICK_OVERDUE = platform_event(kinds=[PlatformEventKind.TICK_OVERDUE])


def landed_after(model: str) -> object:
    """A success condition on one model, built when the sensor module loads."""

    return node_event(select=model, kinds=["success"])
