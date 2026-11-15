"""A sensor outside a domain folder that documents itself twice and reads a node module."""

import python.report

from sqlbuild.orchestration import SensorContext, SkipReason, sensor

STALE_REASON: str = "Stock feed is fresh"


@sensor(
    "stock_feed_stale_sensor",
    description="Count stock when the supplier feed goes stale.",
    run="stock_count_job",
    minimum_interval_seconds=60,
)
def stock_feed_stale(ctx: SensorContext) -> SkipReason:
    """Count stock when the supplier feed goes stale."""
    return SkipReason(f"{STALE_REASON} {python.report.__name__}")
