"""Read SQLBuild discover metadata from a Python function."""

from collections.abc import Callable

from sqlbuild.python_nodes._helpers.attachment import read_attached_definition
from sqlbuild.python_nodes.models import DiscoverDefinition


def read_discover_definition(function: Callable[..., object]) -> DiscoverDefinition | None:
    """Return SQLBuild discover metadata from a decorated function, if present."""

    return read_attached_definition(
        function=function,
        attribute_name="__sqlbuild_discover__",
        definition_type=DiscoverDefinition,
    )
