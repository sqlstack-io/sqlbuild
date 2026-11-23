from python.nodes.tasks.rates import fetch_rates
from python.nodes.tasks.scoring import score_orders

from sqlbuild.loaders import loader


@loader(depends_on=[score_orders, fetch_rates])
def partner_scores(ctx):
    """Load the scores, gated by a read-side asset and an ingress asset."""
    result = ctx.result_of(node_function=score_orders)
    return [{"order_id": o, "score": s} for o, s in result.payload["scores"]]
