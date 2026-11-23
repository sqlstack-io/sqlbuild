"""A discover function's `Discovered` return value as the JSON SQLBuild saves in the snapshot."""

from __future__ import annotations

import json
from collections.abc import Callable

from sqlbuild._native import PythonNodeDiscoveryError
from sqlbuild.python_nodes._helpers.discovered_results import checked_items


def discovered_result_json(
    *,
    returned: object,
    functions: tuple[Callable[..., object], ...],
    provider_names: frozenset[str],
) -> str:
    """`returned` as JSON, each item's args checked against every `for_each` asset function."""

    payload: dict[str, object] = checked_items(
        returned=returned, functions=functions, provider_names=provider_names
    )
    try:
        return json.dumps(payload)
    except TypeError as error:
        raise PythonNodeDiscoveryError(
            f"discovered args and tags must be JSON data: {error}"
        ) from error
