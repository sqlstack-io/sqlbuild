from collections.abc import Callable, Sequence
from typing import overload

from sqlbuild._native import SourceColumnEntry
from sqlbuild.python_nodes.models import LoaderDefinition as LoaderDefinition
from sqlbuild.python_nodes.models import NodeSetRef
from sqlbuild.python_nodes.types import LoaderColumnSpec

@overload
def loader(function: Callable[..., object]) -> Callable[..., object]: ...
@overload
def loader(
    *,
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
) -> Callable[[Callable[..., object]], Callable[..., object]]: ...
def get_loader_definition(function: Callable[..., object]) -> LoaderDefinition | None: ...
