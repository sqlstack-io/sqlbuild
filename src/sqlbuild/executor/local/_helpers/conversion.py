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

_PARALLEL_MINIMUM: int = 8
_MAX_WORKERS: int = 4
_CHUNKS_PER_WORKER: int = 4

type _ConversionKey = tuple[str, str, str]
type _ConversionJob = tuple[str, dict[str, Any] | None, dict[str, str]]


class _ConversionCache:
    """Conversions shared by every thread of the process, keyed by SQL, schema and time zone."""

    def __init__(self) -> None:
        self._lock: threading.Lock = threading.Lock()
        self._results: dict[_ConversionKey, Any] = {}

    def get(self, key: _ConversionKey) -> Any:
        with self._lock:
            return self._results.get(key)

    def missing(self, keys: Sequence[_ConversionKey]) -> list[_ConversionKey]:
        with self._lock:
            return list(dict.fromkeys(key for key in keys if key not in self._results))

    def store(self, results: Mapping[_ConversionKey, Any]) -> None:
        with self._lock:
            self._results.update(results)


_CONVERSIONS: _ConversionCache = _ConversionCache()


def converted(
    *, sql: str, schema: Mapping[str, Any] | None, time_zone: str | None, path: str
) -> Any:
    """SQLQuack's conversion of `sql`: DuckDB SQL and its macros, or the refusals."""

    key: _ConversionKey = _key(sql=sql, schema=schema, time_zone=time_zone)
    cached: Any = _CONVERSIONS.get(key)
    if cached is not None:
        return cached
    result: Any = sqlquack.convert(
        sql=sql,
        options=ConvertOptions(
            no_config=True, schema=schema or None, session=_session(time_zone=time_zone)
        ),
        path=path,
    )
    _CONVERSIONS.store({key: result})
    return result


def convert_all(
    *, sqls: Sequence[str], schema: Mapping[str, Any] | None, time_zone: str | None
) -> int:
    """Convert in worker processes every statement not converted yet; how many they converted."""

    missing: list[_ConversionKey] = _CONVERSIONS.missing(
        [_key(sql=sql, schema=schema, time_zone=time_zone) for sql in sqls]
    )
    workers: int = min(_MAX_WORKERS, os.cpu_count() or 1)
    if len(missing) < _PARALLEL_MINIMUM or workers <= 1:
        return 0
    jobs: list[_ConversionJob] = [
        (key[0], dict(schema) if schema else None, _session(time_zone=time_zone)) for key in missing
    ]
    with ProcessPoolExecutor(
        max_workers=workers, mp_context=multiprocessing.get_context("spawn")
    ) as pool:
        results: list[Any] = list(
            pool.map(
                _convert_job,
                jobs,
                chunksize=max(1, len(jobs) // (workers * _CHUNKS_PER_WORKER)),
            )
        )
    _CONVERSIONS.store(dict(zip(missing, results, strict=True)))
    return len(results)


def _key(*, sql: str, schema: Mapping[str, Any] | None, time_zone: str | None) -> _ConversionKey:
    schema_key: str = json.dumps(schema, sort_keys=True) if schema else ""
    return (sql, schema_key, time_zone or UTC_TIME_ZONE)


def _session(*, time_zone: str | None) -> dict[str, str]:
    zone: str = time_zone or UTC_TIME_ZONE
    return {} if zone == UTC_TIME_ZONE else {TIMEZONE_PARAMETER: zone}


def _convert_job(job: _ConversionJob) -> Any:
    sql, schema, session = job
    return sqlquack.convert(
        sql=sql, options=ConvertOptions(no_config=True, schema=schema, session=session)
    )
