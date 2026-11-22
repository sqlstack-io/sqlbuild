"""Exercise alias deduplication and cyclic containers while discovering a sensor module."""

from sqlbuild.orchestration import PlatformEventKind, RunRequest, platform_event, sensor


@sensor(
    description="Catch up retained platform observations with a stopped group sensor.",
    when=platform_event(
        kinds=[
            PlatformEventKind.TICK_OVERDUE,
            PlatformEventKind.LOCATION_BROKEN,
            PlatformEventKind.LAUNCH_FAILED,
        ]
    ),
    minimum_interval_seconds=7,
    default_status="paused",
)
def edge_platform_orders_sensor(ctx):
    return RunRequest(job="edge_snapshot_job_group")


edge_alias = edge_platform_orders_sensor
edge_factory_results = [[edge_platform_orders_sensor], (edge_alias,)]
edge_factory_results.append(edge_factory_results)
