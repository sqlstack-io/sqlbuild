"""Observe group completion, including cancellation and presumed failure."""

from sqlbuild.orchestration import RunEventKind, RunRequest, run_event, sensor


@sensor(
    description="Observe group completion including cancellation and presumed failure.",
    when=run_event(
        job="edge_snapshot_job_group",
        kinds=[RunEventKind.COMPLETED],
        executed_successfully=("tag:edge_case",),
    ),
    default_status="paused",
)
def edge_group_outcomes_sensor(ctx):
    return RunRequest(job="edge_snapshot_inventory_job")
