"""Public decorator API for SQLBuild tasks."""

from __future__ import annotations

from collections.abc import Callable, Sequence

from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.executor.python_nodes.models import TaskContext
from sqlbuild.python_nodes.main.apply_task import apply_task
from sqlbuild.python_nodes.main.read_task_definition import read_task_definition
from sqlbuild.python_nodes.models import NodeSetRef, RetryPolicy, SqlResourceRef, TaskDefinition

__all__ = ("SkipMode", "TaskContext", "get_task_definition", "task")


def task(
    function: Callable[..., object] | None = None,
    *,
    name: str | None = None,
    depends_on: Callable[..., object]
    | SqlResourceRef
    | NodeSetRef
    | tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...]
    | list[Callable[..., object] | SqlResourceRef | NodeSetRef] = (),
    tags: Sequence[str] = (),
    group: str | None = None,
    description: str | None = None,
    meta: dict[str, object] | None = None,
    retry: RetryPolicy | None = None,
) -> Callable[..., object] | Callable[[Callable[..., object]], Callable[..., object]]:
    """Mark a Python function as a SQLBuild task."""

    return apply_task(
        function=function,
        name=name,
        depends_on=depends_on,
        tags=tags,
        group=group,
        description=description,
        meta=meta,
        retry=retry,
    )


def get_task_definition(function: Callable[..., object]) -> TaskDefinition | None:
    """Return SQLBuild task metadata from a decorated function, if present."""

    return read_task_definition(function)
