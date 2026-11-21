"""Freeze source-job selectors without executing user code in the scheduler."""

from sqlbuild.orchestration import RunEventKind, RunRequest, run_event, sensor


@sensor(
    description="Build inventory only after orders actually succeeds.",
    when=run_event(
        job="source_orders_job",
        kinds=[RunEventKind.COMPLETED],
        executed_successfully=("orders",),
    ),
)
def on_orders_sensor(ctx):
    return RunRequest(job="inventory_job")
