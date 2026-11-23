from python.nodes.tasks.scoring.asset import score_orders

from sqlbuild.loaders import loader


@loader(depends_on=[score_orders])
def partner_scores(ctx):
    """Load the scores the partner returned."""
    result = ctx.result_of(node_function=score_orders)
    return [{"order_id": o, "score": s} for o, s in result.payload["scores"]]
