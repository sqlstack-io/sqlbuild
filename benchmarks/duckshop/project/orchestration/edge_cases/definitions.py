"""Exercise alias deduplication and cyclic factory containers during native discovery."""

from sqlbuild.orchestration import job, job_group, schedule

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
        catch_up="all",
        max_catch_up=3,
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
