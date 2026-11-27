from python.nodes.tasks.packaging.asset import package_scores

from sqlbuild.loaders import loader


@loader(write_strategy="replace", depends_on=[package_scores])
def partner_scores(ctx):
    """Load the packaged scores; only its direct upstream decides whether it runs."""
    result = ctx.result_of(node_function=package_scores)
    return [{"order_id": o, "score": s} for o, s in result.payload["scores"]]
