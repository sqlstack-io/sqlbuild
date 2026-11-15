"""Repairs orders when a tick is overdue: an imported platform event, which selects no nodes."""

from orchestration.sensors._orders_conditions import TICK_OVERDUE

from sqlbuild.orchestration import sensor

on_tick_overdue = sensor(
    "on_tick_overdue_sensor",
    description="Repair orders after an overdue tick",
    when=TICK_OVERDUE,
    run="orders_repair_job",
)
