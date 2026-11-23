"""Loaders for the synthetic inventory source tables, generated in python.loaders._inventory."""

from python.nodes.loaders._registry import load

from sqlbuild.executor.load.models import LoaderContext
from sqlbuild.factories import factory
from sqlbuild.loaders import loader


@factory
def inventory_loaders() -> list[object]:
    @loader
    def inventory__warehouses(ctx: LoaderContext) -> object:
        return load(ctx, "inventory__warehouses")

    @loader
    def inventory__stock_movements(ctx: LoaderContext) -> object:
        return load(ctx, "inventory__stock_movements")

    @loader
    def inventory__stock_counts(ctx: LoaderContext) -> object:
        return load(ctx, "inventory__stock_counts")

    @loader
    def inventory__purchase_orders(ctx: LoaderContext) -> object:
        return load(ctx, "inventory__purchase_orders")

    @loader
    def inventory__po_receipts(ctx: LoaderContext) -> object:
        return load(ctx, "inventory__po_receipts")

    @loader
    def inventory__stock_transfers(ctx: LoaderContext) -> object:
        return load(ctx, "inventory__stock_transfers")

    @loader
    def inventory__stock_reservations(ctx: LoaderContext) -> object:
        return load(ctx, "inventory__stock_reservations")

    @loader
    def inventory__bin_locations(ctx: LoaderContext) -> object:
        return load(ctx, "inventory__bin_locations")

    return [
        inventory__warehouses,
        inventory__stock_movements,
        inventory__stock_counts,
        inventory__purchase_orders,
        inventory__po_receipts,
        inventory__stock_transfers,
        inventory__stock_reservations,
        inventory__bin_locations,
    ]
