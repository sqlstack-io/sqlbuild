from collections.abc import Callable, Mapping, Sequence
from typing import overload

from sqlbuild._native import SourceColumnEntry
from sqlbuild.compiler.python_nodes.types import SkipMode as SkipMode
from sqlbuild.executor.python_nodes.models import AssetContext as AssetContext
from sqlbuild.python_nodes.models import (
    AssetDefinition,
    ColumnLineageRef,
    RetryPolicy,
    SqlResourceRef,
)
from sqlbuild.python_nodes.types import ColumnLineageRefSpec, PythonNodeColumnSpec

__all__ = ("AssetContext", "SkipMode", "asset", "get_asset_definition")

@overload
def asset(function: Callable[..., object]) -> Callable[..., object]: ...
@overload
def asset(
    *,
    name: str | None = None,
    depends_on: Callable[..., object]
    | SqlResourceRef
    | tuple[Callable[..., object] | SqlResourceRef, ...]
    | list[Callable[..., object] | SqlResourceRef] = (),
    tags: Sequence[str] = (),
    group: str | None = None,
    description: str | None = None,
    meta: dict[str, object] | None = None,
    columns: Sequence[PythonNodeColumnSpec | SourceColumnEntry] = (),
    column_lineage: Mapping[str, Sequence[ColumnLineageRefSpec | ColumnLineageRef]] | None = None,
    retry: RetryPolicy | None = None,
    for_each: Callable[..., object] | None = None,
) -> Callable[[Callable[..., object]], Callable[..., object]]: ...
def get_asset_definition(function: Callable[..., object]) -> AssetDefinition | None: ...
