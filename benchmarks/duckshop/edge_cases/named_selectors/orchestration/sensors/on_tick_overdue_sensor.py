"""Repairs orders when a tick is overdue: an imported platform event, which selects no nodes."""

from orchestration.sensors._orders_conditions import TICK_OVERDUE

from sqlbuild.orchestration import RunRequest, sensor


@sensor(
    description="Repair orders after an overdue tick",
    when=TICK_OVERDUE,
)
def on_tick_overdue_sensor(ctx):
    return RunRequest(job="orders_repair_job")
