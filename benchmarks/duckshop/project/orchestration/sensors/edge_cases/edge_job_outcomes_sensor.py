"""Keep warning and partial job outcomes distinct from success."""

from sqlbuild.orchestration import RunEventKind, run_event, sensor

edge_job_outcomes = sensor(
    "edge_job_outcomes_sensor",
    description="Keep warning and partial outcomes distinct from success.",
    when=run_event(job="edge_snapshot_orders_job", kinds=[RunEventKind.WARN, RunEventKind.PARTIAL]),
    run="edge_snapshot_inventory_job",
    default_status="paused",
)
