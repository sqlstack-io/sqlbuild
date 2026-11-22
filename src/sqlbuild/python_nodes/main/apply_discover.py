"""Apply SQLBuild discover metadata to a Python function."""

from collections.abc import Callable
from typing import Any, cast

from sqlbuild.python_nodes._helpers.attachment import attach_definition
from sqlbuild.python_nodes.models import DiscoverDefinition


def apply_discover(
    *,
    function: Callable[..., object] | None = None,
    name: str | None = None,
    check_every: str | None = None,
) -> Callable[..., object] | Callable[[Callable[..., object]], Callable[..., object]]:
    """Mark a function as discovering a set of items in the location, at each publication."""

    def decorate(inner: Callable[..., object]) -> Callable[..., object]:
        inner_function: Any = cast(Any, inner)
        return attach_definition(
            function=inner,
            attribute_name="__sqlbuild_discover__",
            definition=DiscoverDefinition(
                name=name or inner_function.__name__, check_every=check_every
            ),
        )

    return decorate(function) if function is not None else decorate
