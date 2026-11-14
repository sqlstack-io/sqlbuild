"""Freeze source-job selectors without executing user code in the scheduler."""

from sqlbuild.orchestration import RunEventKind, run_event, sensor

on_orders = sensor(
    "on_orders_sensor",
    description="Build inventory only after orders actually succeeds.",
    when=run_event(
        job="source_orders_job",
        kinds=[RunEventKind.COMPLETED],
        executed_successfully=("orders",),
    ),
    run="inventory_job",
)
