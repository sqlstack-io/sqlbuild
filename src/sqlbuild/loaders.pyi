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
    connection: str | None = None,
    destination: str | None = None,
    columns: Sequence[LoaderColumnSpec | SourceColumnEntry] = (),
    description: str | None = None,
    **settings: object,
) -> Callable[[Callable[..., object]], Callable[..., object]]: ...
def get_loader_definition(function: Callable[..., object]) -> LoaderDefinition | None: ...
