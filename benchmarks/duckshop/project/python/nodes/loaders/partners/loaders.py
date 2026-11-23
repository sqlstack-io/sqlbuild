"""Loaders for the synthetic partners source tables, generated in python.loaders._partners."""

from python.nodes.loaders._registry import load

from sqlbuild.executor.load.models import LoaderContext
from sqlbuild.factories import factory
from sqlbuild.loaders import loader


@factory
def partners_loaders() -> list[object]:
    @loader
    def partners__partner_orders(ctx: LoaderContext) -> object:
        return load(ctx, "partners__partner_orders")

    @loader
    def partners__partner_listings(ctx: LoaderContext) -> object:
        return load(ctx, "partners__partner_listings")

    @loader
    def partners__partner_settlements(ctx: LoaderContext) -> object:
        return load(ctx, "partners__partner_settlements")

    return [partners__partner_orders, partners__partner_listings, partners__partner_settlements]
