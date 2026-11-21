"""Replenishment declared in a flat module."""

from sqlbuild.assets import AssetContext, asset


@asset(description="Reorder products that ran out.")
def replenish(ctx: AssetContext) -> None:
    return None
