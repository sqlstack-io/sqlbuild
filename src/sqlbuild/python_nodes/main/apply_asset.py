"""Apply SQLBuild asset metadata to a Python function."""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from typing import Any, cast

from sqlbuild._native import PythonNodeDiscoveryError, SourceColumnEntry
from sqlbuild.python_nodes._helpers.attachment import attach_definition
from sqlbuild.python_nodes._helpers.column_normalization import normalize_columns
from sqlbuild.python_nodes._helpers.dependency_normalization import (
    normalize_python_node_dependencies,
)
from sqlbuild.python_nodes._helpers.description_resolution import resolve_description
from sqlbuild.python_nodes._helpers.lineage_normalization import normalize_column_lineage
from sqlbuild.python_nodes.main.read_discover_definition import read_discover_definition
from sqlbuild.python_nodes.models import (
    AssetDefinition,
    ColumnLineageRef,
    DiscoverDefinition,
    NodeSetRef,
    RetryPolicy,
    SqlResourceRef,
)
from sqlbuild.python_nodes.types import ColumnLineageRefSpec, PythonNodeColumnSpec


def apply_asset(  # noqa: PLR0913
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
    columns: Sequence[PythonNodeColumnSpec | SourceColumnEntry] = (),
    column_lineage: Mapping[str, Sequence[ColumnLineageRefSpec | ColumnLineageRef]] | None = None,
    retry: RetryPolicy | None = None,
    for_each: Callable[..., object] | None = None,
) -> Callable[..., object] | Callable[[Callable[..., object]], Callable[..., object]]:
    """Apply SQLBuild asset metadata to a Python function."""

    discovered_by: str | None = _discover_name(for_each)
    normalized_deps: tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...] = (
        normalize_python_node_dependencies(depends_on)
    )

    def decorate(inner: Callable[..., object]) -> Callable[..., object]:
        inner_function: Any = cast(Any, inner)
        definition: AssetDefinition = AssetDefinition(
            name=name or inner_function.__name__,
            depends_on=normalized_deps,
            tags=tuple(tags),
            group=group,
            description=resolve_description(function=inner, description=description),
            meta=meta,
            columns=normalize_columns(columns),
            column_lineage=normalize_column_lineage(column_lineage),
            retry=retry,
            for_each=discovered_by,
        )
        return attach_definition(
            function=inner,
            attribute_name="__sqlbuild_asset__",
            definition=definition,
        )

    return decorate(function) if function is not None else decorate


def _discover_name(for_each: Callable[..., object] | None) -> str | None:
    """The name of the discover function `for_each` names; anything else is refused."""

    if for_each is None:
        return None
    definition: DiscoverDefinition | None = read_discover_definition(for_each)
    if definition is None:
        raise PythonNodeDiscoveryError("for_each takes a function decorated with @discover")
    return definition.name
