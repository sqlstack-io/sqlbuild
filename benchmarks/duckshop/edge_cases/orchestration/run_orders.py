"""Freeze source-job selectors without executing user code in the scheduler."""

from sqlbuild.orchestration import RunEventKind, job, run_event, sensor

source_orders = job("source_orders", description="Source orders.", select="orders")
inventory = job("inventory", description="Destination inventory.", select="stg_orders")
on_orders = sensor(
    "on_orders",
    description="Build inventory only after orders actually succeeds.",
    when=run_event(
        job=source_orders,
        kinds=[RunEventKind.COMPLETED],
        executed_successfully=("orders",),
    ),
    run=inventory,
)
