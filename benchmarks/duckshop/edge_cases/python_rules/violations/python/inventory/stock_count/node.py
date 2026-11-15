"""Count stock in every warehouse."""

from sqlbuild.tasks import TaskContext, task

WAREHOUSES: tuple[str, ...] = ("north", "south")


@task
def stock_count(context: TaskContext) -> int:
    """Count stock in every warehouse."""
    return len(WAREHOUSES)
