"""Public decorator API for SQLBuild assets."""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence

from sqlbuild._native import SourceColumnEntry
from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.executor.python_nodes.models import AssetContext
from sqlbuild.python_nodes.main.apply_asset import apply_asset
from sqlbuild.python_nodes.main.read_asset_definition import read_asset_definition
from sqlbuild.python_nodes.models import (
    AssetDefinition,
    ColumnLineageRef,
    NodeSetRef,
    RetryPolicy,
    SqlResourceRef,
)
from sqlbuild.python_nodes.types import ColumnLineageRefSpec, PythonNodeColumnSpec

__all__ = ("AssetContext", "SkipMode", "asset", "get_asset_definition")


def asset(  # noqa: PLR0913
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
    columns: Sequence[PythonNodeColumnSpec | SourceColumnEntry] = (),
    column_lineage: Mapping[str, Sequence[ColumnLineageRefSpec | ColumnLineageRef]] | None = None,
    retry: RetryPolicy | None = None,
    for_each: Callable[..., object] | None = None,
) -> Callable[..., object] | Callable[[Callable[..., object]], Callable[..., object]]:
    """Mark a Python function as a SQLBuild asset; `for_each` makes one node per discovered item."""

    return apply_asset(
        function=function,
        name=name,
        depends_on=depends_on,
        tags=tags,
        group=group,
        description=description,
        meta=meta,
        columns=columns,
        column_lineage=column_lineage,
        retry=retry,
        for_each=for_each,
    )


def get_asset_definition(function: Callable[..., object]) -> AssetDefinition | None:
    """Return SQLBuild asset metadata from a decorated function, if present."""

    return read_asset_definition(function)
