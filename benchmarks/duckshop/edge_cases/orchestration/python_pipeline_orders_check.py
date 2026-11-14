"""A Python check on the orders model that a job's tag selector matches.

A job matching the check and the model it checks publishes and runs the check with the model. A job
matching the check alone fails publication, naming the check and the model it leaves out.
"""

from sqlbuild.checks import check
from sqlbuild.refs import model


@check(depends_on=model("orders"), tags=["order_checks"])
def orders_present(ctx):
    """Orders are present."""
    orders = ctx.relation(model("orders"))
    count = ctx.query(f"SELECT count(*) FROM {orders}").fetchone()[0]
    return ctx.pass_() if count else ctx.fail(message="no orders")
