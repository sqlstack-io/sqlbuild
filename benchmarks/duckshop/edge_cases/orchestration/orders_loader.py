from sqlbuild.loaders import loader


@loader
def raw_orders(ctx):
    """Load synthetic orders in the target loader namespace."""
    return [{"order_id": 1}]
