"""Which nodes are discovered items that run in parallel, and how many of their set at once."""

from __future__ import annotations

from collections.abc import Mapping

from sqlbuild.executor.python_nodes.constants import PRIVATE_MEMORY_DATABASE_KEYS
from sqlbuild.executor.python_nodes.models import PythonNodeRuntime


def parallel_set(
    *, discovered: Mapping[str, object] | None, runtime: PythonNodeRuntime
) -> tuple[str, int] | None:
    """The set and its `max_parallel` of an item that may run beside others; `None` runs serially."""

    if discovered is None or discovered.get("placeholder"):
        return None
    limit: object = discovered.get("max_parallel")
    if not isinstance(limit, int) or limit < 2:
        return None
    key: str | None = PRIVATE_MEMORY_DATABASE_KEYS.get(runtime.adapter.adapter_name)
    if key is not None and str(runtime.connection_config.get(key, ":memory:")) == ":memory:":
        return None
    return str(discovered["set"]), limit
