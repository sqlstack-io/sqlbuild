"""Authored discover and asset functions the discovered-result tests check items against."""

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, discover


@discover(check_every="15m")
def orders_plan() -> Discovered:
    return Discovered(items=())


@asset(for_each=orders_plan)
def step(ctx: AssetContext, step_id: str, retries: int = 1, warehouse: object = None) -> None:
    del ctx, step_id, retries, warehouse


def not_discovered() -> None:
    return None
