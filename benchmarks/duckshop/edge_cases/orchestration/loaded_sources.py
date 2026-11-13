"""Loaders for a two-loader chain, a hook-read source and a source no job reads."""

from sqlbuild.loaders import loader


@loader
def raw_customers(ctx):
    """Load synthetic customers."""
    return [{"customer_id": 7}]


@loader(depends_on=[raw_customers])
def raw_orders(ctx):
    """Load one order per loaded customer."""
    customers = ctx.loader(raw_customers)
    rows = ctx.query(f"SELECT customer_id FROM {customers.destination}").fetchall()
    return [{"order_id": row[0]} for row in rows]


@loader
def raw_returns(ctx):
    """Load synthetic returns."""
    return [{"order_id": 7}]


@loader
def raw_inventory(ctx):
    """Load synthetic inventory."""
    return [{"product_id": 1}]
