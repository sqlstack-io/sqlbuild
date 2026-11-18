from sqlbuild.refs import model
from sqlbuild.tasks import task


@task(depends_on=model("scored_orders"))
def score_orders(ctx):
    """Send the scored orders to the partner."""
    return ctx.result(payload={})
