"""SQLQuack conversion of Snowflake SQL to DuckDB SQL, once per text, schema and session."""

from __future__ import annotations

import json
import threading
from collections.abc import Mapping
from typing import Any

import sqlquack
from sqlquack import ConvertOptions

from sqlbuild.executor.local.constants import TIMEZONE_PARAMETER, UTC_TIME_ZONE

_CACHE: dict[tuple[str, str, str], Any] = {}
_LOCK: threading.Lock = threading.Lock()


def converted(
    *, sql: str, schema: Mapping[str, Any] | None, time_zone: str | None, path: str
) -> Any:
    """SQLQuack's conversion of `sql`: DuckDB SQL and its macros, or the refusals."""

    schema_key: str = json.dumps(schema, sort_keys=True) if schema else ""
    zone: str = time_zone or UTC_TIME_ZONE
    key: tuple[str, str, str] = (sql, schema_key, zone)
    with _LOCK:
        cached: Any = _CACHE.get(key)
    if cached is not None:
        return cached
    session: dict[str, str] = {} if zone == UTC_TIME_ZONE else {TIMEZONE_PARAMETER: zone}
    result: Any = sqlquack.convert(
        sql=sql,
        options=ConvertOptions(no_config=True, schema=schema or None, session=session),
        path=path,
    )
    with _LOCK:
        _CACHE[key] = result
    return result
