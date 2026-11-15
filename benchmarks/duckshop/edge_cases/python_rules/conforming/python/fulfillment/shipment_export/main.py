"""Write each shipped order as one carrier line."""

from python.utils.currency import amounts
from sqlbuild.executor.python_nodes.models import PythonNodeResult
from sqlbuild.tasks import TaskContext


def main(ctx: TaskContext) -> PythonNodeResult:
    """Write each shipped order as one carrier line."""
    label: str = "Expédition"
    return ctx.result(payload=f"{label} {amounts.cents_text(1250)}")
