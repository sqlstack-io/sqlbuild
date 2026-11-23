"""Loaders for the edge-case area's raw tables, with the fixed rows in python.loaders._edge_cases.

These rows are hand-picked edge values, not generated volume: `data_scale` does not change them,
and they never touch the core tables, so core outputs and expectations stay the same.
"""

from python.nodes.loaders._edge_cases import DELIVERY_BATCH_VAR, TABLES, delivery_updates
from python.nodes.loaders._writer import statements

from sqlbuild.factories import factory
from sqlbuild.executor.load.models import LoaderContext
from sqlbuild.loaders import loader


def _load(ctx: LoaderContext, source_name: str) -> object:
    rows = TABLES[source_name]
    for sql in statements(ctx.destination, source_name, rows):
        ctx.execute_sql(sql)
    return ctx.result(metadata={"rows": len(rows)})



@factory
def edge_cases_loaders() -> list[object]:
    @loader
    def edge__partner_payloads(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__partner_payloads")

    @loader
    def edge__gateway_callbacks(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__gateway_callbacks")

    @loader
    def edge__price_windows(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__price_windows")

    @loader
    def edge__customer_profiles(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__customer_profiles")

    @loader
    def edge__ledger_entries(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__ledger_entries")

    @loader
    def edge__ticket_notes(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__ticket_notes")

    @loader
    def edge__partner_listings(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__partner_listings")

    @loader
    def edge__basket_lines(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__basket_lines")

    @loader
    def edge__date_texts(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__date_texts")

    @loader
    def edge__amount_checks(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__amount_checks")

    @loader
    def edge__variant_values(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__variant_values")

    @loader
    def edge__category_links(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__category_links")

    @loader
    def edge__supplier_notes(ctx: LoaderContext) -> object:
        return _load(ctx, "edge__supplier_notes")

    @loader
    def edge__delivery_updates(ctx: LoaderContext) -> object:
        batch = int(ctx.vars.get(DELIVERY_BATCH_VAR, 2))  # type: ignore[call-overload]
        rows = delivery_updates(batch)
        for sql in statements(ctx.destination, "edge__delivery_updates", rows):
            ctx.execute_sql(sql)
        return ctx.result(metadata={"rows": len(rows), "batch": batch})


    return [edge__partner_payloads, edge__gateway_callbacks, edge__price_windows, edge__customer_profiles, edge__ledger_entries, edge__ticket_notes, edge__partner_listings, edge__basket_lines, edge__date_texts, edge__amount_checks, edge__variant_values, edge__category_links, edge__supplier_notes, edge__delivery_updates]
