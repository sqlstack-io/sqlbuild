from sqlbuild.assets import asset
from sqlbuild.refs import model


@asset(depends_on=model("orders"))
def score_orders(ctx):
    """Send the orders to the partner and keep the scores it returns."""
    relation = ctx.relation(model("orders"))
    rows = ctx.query(f"SELECT order_id FROM {relation} ORDER BY order_id").fetchall()
    return ctx.result(payload={"scores": [[row[0], row[0] * 10] for row in rows]})
