"""Write each shipped order as one carrier line."""

from python.nodes.utils.currency import amounts
from sqlbuild.assets import AssetContext
from sqlbuild.executor.python_nodes.models import PythonNodeResult


def main(ctx: AssetContext) -> PythonNodeResult:
    """Write each shipped order as one carrier line."""
    label: str = "Expédition"
    return ctx.result(payload=f"{label} {amounts.cents_text(1250)}")
