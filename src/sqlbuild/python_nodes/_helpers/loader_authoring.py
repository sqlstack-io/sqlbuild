"""Public decorator API for SQLBuild source loaders."""

from __future__ import annotations

from collections.abc import Callable, Sequence

from sqlbuild._native import SourceColumnEntry
from sqlbuild._native import SourceWriteStrategy as SourceWriteStrategy
from sqlbuild.python_nodes.main._apply_loader import apply_loader
from sqlbuild.python_nodes.main.read_loader_definition import read_loader_definition
from sqlbuild.python_nodes.models import LoaderDefinition, NodeSetRef
from sqlbuild.python_nodes.types import LoaderColumnSpec


def loader(
    function: Callable[..., object] | None = None,
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
) -> Callable[..., object] | Callable[[Callable[..., object]], Callable[..., object]]:
    """Mark a Python function as a SQLBuild source loader."""

    return apply_loader(
        function=function,
        name=name,
        depends_on=depends_on,
        destination=destination,
        write_strategy=write_strategy,
        cursor_column=cursor_column,
        unique_key=unique_key,
        columns=columns,
        contract=contract,
        description=description,
    )


def get_loader_definition(function: Callable[..., object]) -> LoaderDefinition | None:
    """Return SQLBuild loader metadata from a decorated function, if present."""

    return read_loader_definition(function)
