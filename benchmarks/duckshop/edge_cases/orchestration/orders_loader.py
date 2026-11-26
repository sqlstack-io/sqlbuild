from sqlbuild.loaders import loader


@loader(write_strategy="replace")
def raw_orders(ctx):
    """Load synthetic orders in the target loader namespace."""
    return [{"order_id": 1}]
