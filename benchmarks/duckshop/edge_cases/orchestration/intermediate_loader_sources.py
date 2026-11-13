"""A source loader that depends on an intermediate loader no source declares."""

from sqlbuild.loaders import loader


@loader
def stage_shipments(ctx):
    """Stage synthetic shipments in the intermediate loader's own relation."""
    return [{"order_id": 9}]


@loader(depends_on=[stage_shipments])
def raw_shipments(ctx):
    """Load the staged shipments."""
    staged = ctx.loader(stage_shipments)
    rows = ctx.query(f"SELECT order_id FROM {staged.destination}").fetchall()
    return [{"order_id": row[0]} for row in rows]
