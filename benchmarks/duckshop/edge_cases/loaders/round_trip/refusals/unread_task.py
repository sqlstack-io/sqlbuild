from sqlbuild.tasks import task


@task()
def score_orders(ctx):
    """Fetch the partner scores."""
    return ctx.result(payload={})
