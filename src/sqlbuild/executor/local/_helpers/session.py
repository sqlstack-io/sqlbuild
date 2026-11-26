"""The session time zone a target's warehouse connection sets."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from sqlbuild.executor.local.constants import SESSION_PARAMETERS_KEY, TIMEZONE_PARAMETER


def session_time_zone(project: Any) -> str | None:
    """The `TIMEZONE` session parameter the target's connection sets; `None` when unset."""

    connection: Mapping[str, Any] = getattr(project, "effective_connection", None) or {}
    parameters: Any = connection.get(SESSION_PARAMETERS_KEY) or {}
    if not isinstance(parameters, Mapping):
        return None
    for key, value in parameters.items():
        if str(key).upper() == TIMEZONE_PARAMETER and value:
            return str(value)
    return None
