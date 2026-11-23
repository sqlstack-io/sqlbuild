from python.nodes.tasks.rates import fetch_rates

from sqlbuild.loaders import loader


@loader(depends_on=[fetch_rates])
def partner_rates(ctx):
    """Load the partner's rates before the build's SQL."""
    return [{"rate": ctx.result_of(node_function=fetch_rates).payload["rate"]}]
