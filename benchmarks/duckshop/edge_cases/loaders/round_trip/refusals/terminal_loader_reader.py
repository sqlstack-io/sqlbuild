from python.loaders.partner import partner_scores

from sqlbuild.tasks import task


@task(depends_on=[partner_scores])
def report_scores(ctx):
    """Report the scores the partner returned."""
    return ctx.result(payload={})
