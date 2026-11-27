"""Apply SQLBuild loader metadata to a Python function."""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from typing import Any, cast

from sqlbuild._native import SourceColumnEntry
from sqlbuild.python_nodes._helpers.attachment import attach_definition
from sqlbuild.python_nodes._helpers.column_normalization import normalize_columns
from sqlbuild.python_nodes._helpers.dependency_normalization import normalize_loader_dependencies
from sqlbuild.python_nodes._helpers.description_resolution import resolve_description
from sqlbuild.python_nodes.models import LoaderDefinition, NodeSetRef
from sqlbuild.python_nodes.types import LoaderColumnSpec


def apply_loader(
    *,
    function: Callable[..., object] | None = None,
    name: str | None = None,
    depends_on: tuple[Callable[..., object] | NodeSetRef, ...]
    | list[Callable[..., object] | NodeSetRef] = (),
    connection: str | None = None,
    destination: str | None = None,
    columns: Sequence[LoaderColumnSpec | SourceColumnEntry] = (),
    description: str | None = None,
    settings: Mapping[str, object] | None = None,
) -> Callable[..., object] | Callable[[Callable[..., object]], Callable[..., object]]:
    """Apply SQLBuild loader metadata, with SQLLoad's table options as `settings`, to a function."""

    normalized_deps: tuple[Callable[..., object] | NodeSetRef, ...] = normalize_loader_dependencies(
        depends_on
    )

    def decorate(inner: Callable[..., object]) -> Callable[..., object]:
        inner_function: Any = cast(Any, inner)
        definition: LoaderDefinition = LoaderDefinition(
            name=name or inner_function.__name__,
            depends_on=normalized_deps,
            connection=connection,
            destination=destination,
            columns=normalize_columns(columns),
            description=resolve_description(function=inner, description=description),
            settings=dict(settings or {}),
        )
        return attach_definition(
            function=inner,
            attribute_name="__sqlbuild_loader__",
            definition=definition,
        )

    return decorate(function) if function is not None else decorate
