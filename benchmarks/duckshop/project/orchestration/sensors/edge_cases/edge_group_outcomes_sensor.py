"""Observe group completion, including cancellation and presumed failure."""

from sqlbuild.orchestration import RunEventKind, run_event, sensor

edge_group_outcomes = sensor(
    "edge_group_outcomes_sensor",
    description="Observe group completion including cancellation and presumed failure.",
    when=run_event(
        job="edge_snapshot_job_group",
        kinds=[RunEventKind.COMPLETED],
        executed_successfully=("tag:edge_case",),
    ),
    run="edge_snapshot_inventory_job",
    default_status="paused",
)
