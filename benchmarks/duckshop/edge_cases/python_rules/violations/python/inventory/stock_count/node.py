"""Count stock in every warehouse."""

from orchestration.sensors.fulfillment._helpers.hours import CUTOFF_HOURS

from sqlbuild.assets import AssetContext, asset

WAREHOUSES: tuple[str, ...] = ("north", "south")


@asset
def stock_count(context: AssetContext) -> int:
    """Count stock in every warehouse."""
    return len(WAREHOUSES) + CUTOFF_HOURS
