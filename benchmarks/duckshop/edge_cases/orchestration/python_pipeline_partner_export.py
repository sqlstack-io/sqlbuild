"""An asset one job builds that a task in another job of the same group depends on."""

from sqlbuild.assets import asset
from sqlbuild.refs import model


@asset(depends_on=model("orders"), tags=["orders"], description="Partner order file.")
def partner_orders_file(ctx):
    return None
