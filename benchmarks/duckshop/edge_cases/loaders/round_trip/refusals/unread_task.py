from sqlbuild.assets import asset


@asset()
def score_orders(ctx):
    """Fetch the partner scores."""
    return ctx.result(payload={})
