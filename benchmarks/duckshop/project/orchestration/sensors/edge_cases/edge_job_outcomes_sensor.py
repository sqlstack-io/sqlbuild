"""Keep warning and failed job outcomes distinct from success."""

from sqlbuild.orchestration import RunEventKind, RunRequest, run_event, sensor


@sensor(
    description="Keep warning and failed outcomes distinct from success.",
    when=run_event(job="edge_snapshot_orders_job", kinds=[RunEventKind.WARN, RunEventKind.FAILED]),
    default_status="paused",
)
def edge_job_outcomes_sensor(ctx):
    return RunRequest(job="edge_snapshot_inventory_job")
