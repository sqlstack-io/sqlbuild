"""Apply SQLBuild task metadata to a Python function."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any, cast

from sqlbuild.python_nodes._helpers.attachment import attach_definition
from sqlbuild.python_nodes._helpers.dependency_normalization import (
    normalize_python_node_dependencies,
)
from sqlbuild.python_nodes._helpers.description_resolution import resolve_description
from sqlbuild.python_nodes.models import NodeSetRef, RetryPolicy, SqlResourceRef, TaskDefinition


def apply_task(
    *,
    function: Callable[..., object] | None = None,
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
    """Apply SQLBuild task metadata to a Python function."""

    normalized_deps: tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...] = (
        normalize_python_node_dependencies(depends_on)
    )

    def decorate(inner: Callable[..., object]) -> Callable[..., object]:
        inner_function: Any = cast(Any, inner)
        definition: TaskDefinition = TaskDefinition(
            name=name or inner_function.__name__,
            depends_on=normalized_deps,
            tags=tuple(tags),
            group=group,
            description=resolve_description(function=inner, description=description),
            meta=meta,
            retry=retry,
        )
        return attach_definition(
            function=inner,
            attribute_name="__sqlbuild_task__",
            definition=definition,
        )

    return decorate(function) if function is not None else decorate
