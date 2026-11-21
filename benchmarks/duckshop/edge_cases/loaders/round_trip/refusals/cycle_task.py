from sqlbuild.assets import asset
from sqlbuild.refs import model


@asset(depends_on=model("scored_orders"))
def score_orders(ctx):
    """Send the scored orders to the partner."""
    return ctx.result(payload={})
