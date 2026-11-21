"""The discovered item an asset node runs for: its arguments, context and placeholder failure."""

from __future__ import annotations

from collections.abc import Mapping

from sqlbuild.executor.python_nodes.models import DiscoveredContext
from sqlbuild.executor.python_nodes.types import ExecutablePythonNode


def item_args(node: ExecutablePythonNode) -> dict[str, object] | None:
    """The arguments a discovered item's node is called with; `None` for any other node."""

    discovered: Mapping[str, object] | None = node.discovered
    if discovered is None:
        return None
    return _string_keyed(discovered.get("args"))


def discovered_context(node: ExecutablePythonNode) -> DiscoveredContext | None:
    """The item and set tags `ctx.discovered` gives a discovered item's node."""

    discovered: Mapping[str, object] | None = node.discovered
    if discovered is None or discovered.get("placeholder"):
        return None
    return DiscoveredContext(
        set=str(discovered["set"]),
        item=str(discovered["item"]),
        tags=_string_keyed(discovered.get("tags")),
    )


def placeholder_failure(node: ExecutablePythonNode) -> str | None:
    """Why the placeholder of a set that never succeeded cannot run; `None` for any other node."""

    discovered: Mapping[str, object] | None = node.discovered
    if discovered is None or not discovered.get("placeholder"):
        return None
    error: object = discovered.get("error")
    return f"never discovered: {error if error else 'no discovery has run'}"


def _string_keyed(value: object) -> dict[str, object]:
    """A saved JSON object as a dict; anything else as an empty one."""

    if not isinstance(value, Mapping):
        return {}
    return {str(key): item for key, item in value.items()}
