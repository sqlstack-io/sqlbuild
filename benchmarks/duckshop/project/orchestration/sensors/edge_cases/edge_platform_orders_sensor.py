"""Exercise alias deduplication and cyclic containers while discovering a sensor module."""

from sqlbuild.orchestration import PlatformEventKind, platform_event, sensor

edge_health = sensor(
    "edge_platform_orders_sensor",
    description="Catch up retained platform observations with a stopped group sensor.",
    when=platform_event(
        kinds=[
            PlatformEventKind.TICK_OVERDUE,
            PlatformEventKind.LOCATION_BROKEN,
            PlatformEventKind.LAUNCH_FAILED,
        ]
    ),
    run="edge_snapshot_job_group",
    minimum_interval_seconds=7,
    default_status="paused",
)
edge_alias = edge_health
edge_factory_results = [[edge_health], (edge_alias,)]
edge_factory_results.append(edge_factory_results)
