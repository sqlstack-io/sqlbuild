"""The discovered item an asset node runs for: its arguments, context and placeholder failure."""

from __future__ import annotations

from collections.abc import Mapping

from sqlbuild.executor.python_nodes.constants import (
    CONNECTION_PRIVATE_MEMORY_DATABASE_KEYS,
    MEMORY_DATABASE,
    MIN_PARALLEL_ITEMS,
)
from sqlbuild.executor.python_nodes.models import DiscoveredContext, PythonNodeRuntime
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


def parallel_set(
    *, discovered: Mapping[str, object] | None, runtime: PythonNodeRuntime
) -> tuple[str, int] | None:
    """The set and `max_parallel` of an item that may run beside others; `None` runs serially."""

    if discovered is None or discovered.get("placeholder"):
        return None
    limit: object = discovered.get("max_parallel")
    if not isinstance(limit, int) or limit < MIN_PARALLEL_ITEMS:
        return None
    if not shares_warehouse(runtime):
        return None
    return str(discovered["set"]), limit


def shares_warehouse(runtime: PythonNodeRuntime) -> bool:
    """Whether another connection opened from the runtime's settings reaches the same warehouse."""

    key: str | None = CONNECTION_PRIVATE_MEMORY_DATABASE_KEYS.get(runtime.adapter.adapter_name)
    if key is None:
        return True
    return str(runtime.connection_config.get(key, MEMORY_DATABASE)) != MEMORY_DATABASE
