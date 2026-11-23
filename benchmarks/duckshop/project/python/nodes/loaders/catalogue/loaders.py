"""Loaders for the synthetic catalogue source tables, generated in python.loaders._catalogue."""

from python.nodes.loaders._registry import load

from sqlbuild.executor.load.models import LoaderContext
from sqlbuild.factories import factory
from sqlbuild.loaders import loader


@factory
def catalogue_loaders() -> list[object]:
    @loader
    def catalogue__brands(ctx: LoaderContext) -> object:
        return load(ctx, "catalogue__brands")

    @loader
    def catalogue__categories(ctx: LoaderContext) -> object:
        return load(ctx, "catalogue__categories")

    @loader
    def catalogue__products(ctx: LoaderContext) -> object:
        return load(ctx, "catalogue__products")

    @loader
    def catalogue__product_variants(ctx: LoaderContext) -> object:
        return load(ctx, "catalogue__product_variants")

    @loader
    def catalogue__price_lists(ctx: LoaderContext) -> object:
        return load(ctx, "catalogue__price_lists")

    @loader
    def catalogue__product_media(ctx: LoaderContext) -> object:
        return load(ctx, "catalogue__product_media")

    @loader
    def catalogue__bundles(ctx: LoaderContext) -> object:
        return load(ctx, "catalogue__bundles")

    @loader
    def catalogue__bundle_components(ctx: LoaderContext) -> object:
        return load(ctx, "catalogue__bundle_components")

    @loader
    def catalogue__suppliers(ctx: LoaderContext) -> object:
        return load(ctx, "catalogue__suppliers")

    @loader
    def catalogue__supplier_costs(ctx: LoaderContext) -> object:
        return load(ctx, "catalogue__supplier_costs")

    return [
        catalogue__brands,
        catalogue__categories,
        catalogue__products,
        catalogue__product_variants,
        catalogue__price_lists,
        catalogue__product_media,
        catalogue__bundles,
        catalogue__bundle_components,
        catalogue__suppliers,
        catalogue__supplier_costs,
    ]
