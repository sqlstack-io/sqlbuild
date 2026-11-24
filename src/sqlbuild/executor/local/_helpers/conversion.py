"""SQLQuack conversion of Snowflake SQL to DuckDB SQL, once per text, schema and session."""

from __future__ import annotations

import json
import multiprocessing
import os
import threading
from collections.abc import Mapping, Sequence
from concurrent.futures import ProcessPoolExecutor
from typing import Any

import sqlquack
from sqlquack import ConvertOptions

from sqlbuild.executor.local.constants import TIMEZONE_PARAMETER, UTC_TIME_ZONE

_CACHE: dict[tuple[str, str, str], Any] = {}
_LOCK: threading.Lock = threading.Lock()
# SQLQuack holds the GIL while it converts, so many statements convert in worker processes.
_PARALLEL_MINIMUM: int = 8
_MAX_WORKERS: int = 4


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


def convert_all(
    *, sqls: Sequence[str], schema: Mapping[str, Any] | None, time_zone: str | None
) -> None:
    """Convert every statement not converted yet, in worker processes when there are many."""

    schema_key: str = json.dumps(schema, sort_keys=True) if schema else ""
    zone: str = time_zone or UTC_TIME_ZONE
    with _LOCK:
        missing: list[str] = list(
            dict.fromkeys(sql for sql in sqls if (sql, schema_key, zone) not in _CACHE)
        )
    workers: int = min(_MAX_WORKERS, os.cpu_count() or 1)
    if len(missing) < _PARALLEL_MINIMUM or workers < 2:
        return
    session: dict[str, str] = {} if zone == UTC_TIME_ZONE else {TIMEZONE_PARAMETER: zone}
    with ProcessPoolExecutor(
        max_workers=workers, mp_context=multiprocessing.get_context("spawn")
    ) as pool:
        results: list[Any] = list(
            pool.map(
                _convert_one,
                missing,
                [dict(schema) if schema else None] * len(missing),
                [session] * len(missing),
                chunksize=max(1, len(missing) // (workers * 4)),
            )
        )
    with _LOCK:
        for sql, result in zip(missing, results, strict=True):
            _CACHE[(sql, schema_key, zone)] = result


def _convert_one(sql: str, schema: dict[str, Any] | None, session: dict[str, str]) -> Any:
    return sqlquack.convert(
        sql=sql, options=ConvertOptions(no_config=True, schema=schema, session=session)
    )
