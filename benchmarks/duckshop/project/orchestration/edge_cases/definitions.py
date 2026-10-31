"""Exercise alias deduplication and cyclic factory containers during native discovery."""

from sqlbuild.orchestration import job, schedule

edge_orders = job(
    "edge_snapshot_orders",
    description="Refresh only the synthetic edge-case models.",
    select="tag:edge_case",
    settings={"run_tests": False},
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
