from collections.abc import Callable
from typing import overload

from sqlbuild.python_nodes.models import DiscoverDefinition
from sqlbuild.python_nodes.models import Discovered as Discovered
from sqlbuild.python_nodes.models import Item as Item

__all__ = ("Discovered", "Item", "discover", "get_discover_definition")

@overload
def discover(function: Callable[..., object]) -> Callable[..., object]: ...
@overload
def discover(
    *, name: str | None = None, check_every: str | None = None
) -> Callable[[Callable[..., object]], Callable[..., object]]: ...
def get_discover_definition(function: Callable[..., object]) -> DiscoverDefinition | None: ...
