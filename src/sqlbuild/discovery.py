"""Public API for discovered nodes: one asset node per item a discover function returns."""

from __future__ import annotations

from collections.abc import Callable

from sqlbuild.python_nodes.main.apply_discover import apply_discover
from sqlbuild.python_nodes.main.read_discover_definition import read_discover_definition
from sqlbuild.python_nodes.models import DiscoverDefinition, Discovered, Item

__all__ = ("Discovered", "Item", "discover", "get_discover_definition")


def discover(
    function: Callable[..., object] | None = None,
    *,
    name: str | None = None,
    check_every: str | None = None,
) -> Callable[..., object] | Callable[[Callable[..., object]], Callable[..., object]]:
    """Mark a function that returns `Discovered`; only a location calls it, when it publishes."""

    return apply_discover(function=function, name=name, check_every=check_every)


def get_discover_definition(function: Callable[..., object]) -> DiscoverDefinition | None:
    """Return SQLBuild discover metadata from a decorated function, if present."""

    return read_discover_definition(function)
