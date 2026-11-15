"""Replenishment declared in a flat module."""

from sqlbuild.tasks import TaskContext, task


@task(description="Reorder products that ran out.")
def replenish(ctx: TaskContext) -> None:
    return None
