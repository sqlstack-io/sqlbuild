"""Apply SQLBuild loader metadata to a Python function."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any, cast

from sqlbuild._native import SourceColumnEntry
from sqlbuild.python_nodes._helpers.attachment import attach_definition
from sqlbuild.python_nodes._helpers.column_normalization import normalize_columns
from sqlbuild.python_nodes._helpers.dependency_normalization import normalize_loader_dependencies
from sqlbuild.python_nodes._helpers.description_resolution import resolve_description
from sqlbuild.python_nodes._helpers.loader_normalization import (
    normalize_unique_key,
    normalize_write_strategy,
)
from sqlbuild.python_nodes.models import LoaderDefinition, NodeSetRef
from sqlbuild.python_nodes.types import LoaderColumnSpec


def apply_loader(
    *,
    function: Callable[..., object] | None = None,
    name: str | None = None,
    depends_on: tuple[Callable[..., object] | NodeSetRef, ...]
    | list[Callable[..., object] | NodeSetRef] = (),
    destination: str | None = None,
    write_strategy: str | None = None,
    cursor_column: str | None = None,
    unique_key: str | Sequence[str] | None = None,
    columns: Sequence[LoaderColumnSpec | SourceColumnEntry] = (),
    contract: str | None = None,
    description: str | None = None,
) -> Callable[..., object] | Callable[[Callable[..., object]], Callable[..., object]]:
    """Apply SQLBuild loader metadata to a Python function."""

    normalized_deps: tuple[Callable[..., object] | NodeSetRef, ...] = normalize_loader_dependencies(
        depends_on
    )

    def decorate(inner: Callable[..., object]) -> Callable[..., object]:
        inner_function: Any = cast(Any, inner)
        definition: LoaderDefinition = LoaderDefinition(
            name=name or inner_function.__name__,
            depends_on=normalized_deps,
            destination=destination,
            write_strategy=normalize_write_strategy(write_strategy),
            cursor_column=cursor_column,
            unique_key=normalize_unique_key(unique_key),
            columns=normalize_columns(columns),
            contract=contract,
            description=resolve_description(function=inner, description=description),
        )
        return attach_definition(
            function=inner,
            attribute_name="__sqlbuild_loader__",
            definition=definition,
        )

    return decorate(function) if function is not None else decorate
