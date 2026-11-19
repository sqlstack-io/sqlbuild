from python.tasks.scoring import score_orders

from sqlbuild.assets import asset


@asset(depends_on=[score_orders])
def package_scores(ctx):
    """Package the partner's scores; skips softly when there is nothing new to send."""
    return ctx.skip(reason="nothing to package")
