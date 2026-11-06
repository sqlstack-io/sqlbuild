"""Exercise alias deduplication and cyclic factory containers during native discovery."""

from sqlbuild.orchestration import (
    PlatformEventKind,
    RunEventKind,
    job,
    job_group,
    platform_event,
    run_event,
    schedule,
    sensor,
)

edge_orders = job(
    "edge_snapshot_orders",
    description="Refresh only the synthetic edge-case models.",
    select="tag:edge_case",
    settings={"run_tests": False, "vars": {"edge_delivery_batch": "1"}, "reload": True},
    concurrency=2,
    schedule=schedule(
        cron="15 2 * * *",
        timezone="UTC",
        starts_at="2026-07-03T01:17:23.456789+05:30",
        default_status="paused",
    ),
)
edge_alias = edge_orders
edge_factory_results = [[edge_orders]]
edge_factory_results.append(edge_factory_results)
edge_group = job_group(
    "edge_snapshot_group",
    description="Exercise group discovery through shared, cyclic declaration containers.",
    jobs=[edge_orders],
)
edge_factory_results.append(edge_group)

edge_inventory = job(
    "edge_snapshot_inventory",
    description="Exercise a child limit independent of its parent.",
    select="tag:edge_case",
    concurrency_groups=("edge_products_limit",),
)
edge_products = job(
    "edge_snapshot_products",
    description="Exercise the opposing parent and child acquisition order.",
    select="tag:edge_case",
    concurrency_groups=("edge_inventory_limit",),
)
edge_inventory_group = job_group(
    "edge_inventory_group",
    description="Complete inventory without reserving its child's product limit.",
    jobs=[edge_inventory],
    concurrency_groups=("edge_inventory_limit",),
)
edge_products_group = job_group(
    "edge_products_group",
    description="Wait for safe admission when both installation limits are one.",
    jobs=[edge_products],
    concurrency_groups=("edge_products_limit",),
)
edge_health = sensor(
    "edge_platform_orders",
    description="Catch up retained platform observations with a stopped group sensor.",
    when=platform_event(
        kinds=[
            PlatformEventKind.TICK_OVERDUE,
            PlatformEventKind.LOCATION_BROKEN,
            PlatformEventKind.LAUNCH_FAILED,
        ]
    ),
    run=edge_group,
    minimum_interval_seconds=7,
    default_status="paused",
)
edge_factory_results.append(edge_health)

edge_job_outcomes = sensor(
    "edge_job_outcomes",
    description="Keep warning and partial outcomes distinct from success.",
    when=run_event(job=edge_orders, kinds=[RunEventKind.WARN, RunEventKind.PARTIAL]),
    run=edge_inventory,
    default_status="paused",
)
edge_group_outcomes = sensor(
    "edge_group_outcomes",
    description="Observe group completion including cancellation and presumed failure.",
    when=run_event(
        job=edge_group,
        kinds=[RunEventKind.COMPLETED],
        executed_successfully=("tag:edge_case",),
    ),
    run=edge_inventory,
    default_status="paused",
)
edge_factory_results.extend([edge_job_outcomes, edge_group_outcomes])
