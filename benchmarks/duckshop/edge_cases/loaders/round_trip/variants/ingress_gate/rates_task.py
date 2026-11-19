from sqlbuild.assets import asset


@asset()
def fetch_rates(ctx):
    """Fetch the partner's rates; reads no SQL, so it runs before the build's SQL."""
    return ctx.result(payload={"rate": 10})
