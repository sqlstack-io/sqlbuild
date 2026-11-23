from python.nodes.loaders.partner import partner_scores

from sqlbuild.assets import asset


@asset(depends_on=[partner_scores])
def report_scores(ctx):
    """Report the scores the partner returned."""
    return ctx.result(payload={})
