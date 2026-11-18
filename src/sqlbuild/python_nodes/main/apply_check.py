"""Apply SQLBuild check metadata to a Python function."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any, cast

from sqlbuild.python_nodes._helpers.attachment import attach_definition
from sqlbuild.python_nodes._helpers.dependency_normalization import (
    normalize_python_node_dependencies,
)
from sqlbuild.python_nodes._helpers.description_resolution import resolve_description
from sqlbuild.python_nodes.models import CheckDefinition, NodeSetRef, SqlResourceRef
from sqlbuild.python_nodes.types import PythonCheckSeverity


def apply_check(
    *,
    depends_on: Callable[..., object]
    | SqlResourceRef
    | NodeSetRef
    | tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...]
    | list[Callable[..., object] | SqlResourceRef | NodeSetRef],
    name: str | None = None,
    severity: str | PythonCheckSeverity = PythonCheckSeverity.ERROR,
    tags: Sequence[str] = (),
    group: str | None = None,
    description: str | None = None,
    meta: dict[str, object] | None = None,
) -> Callable[[Callable[..., object]], Callable[..., object]]:
    """Apply SQLBuild check metadata to a Python function."""

    normalized_deps: tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...] = (
        normalize_python_node_dependencies(depends_on)
    )

    def decorate(inner: Callable[..., object]) -> Callable[..., object]:
        inner_function: Any = cast(Any, inner)
        definition: CheckDefinition = CheckDefinition(
            name=name or inner_function.__name__,
            depends_on=normalized_deps,
            severity=PythonCheckSeverity(severity),
            tags=tuple(tags),
            group=group,
            description=resolve_description(function=inner, description=description),
            meta=meta,
        )
        return attach_definition(
            function=inner,
            attribute_name="__sqlbuild_check__",
            definition=definition,
        )

    return decorate
