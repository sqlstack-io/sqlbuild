"""Once per unique SQL: SQLQuack's conversion and the relation an empty DuckDB lacks."""

from __future__ import annotations

import hashlib
import json
import multiprocessing
import os
import re
import threading
from collections.abc import Mapping, Sequence
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path
from typing import Any

import sqlquack
from sqlquack import ConvertOptions
from sqlquack.api.constants import DEFAULT_PATH

from sqlbuild.executor.local.classes.empty_duckdb import EmptyDuckDb
from sqlbuild.executor.local.constants import TIMEZONE_PARAMETER, UTC_TIME_ZONE
from sqlbuild.executor.local.models import LocalConversion, LocalSqlCheck, LocalWorkCounts

_PARALLEL_MINIMUM: int = 8
_MAX_WORKERS: int = 4
_CHUNKS_PER_WORKER: int = 4
_IDENTIFIER: re.Pattern[str] = re.compile(r"[A-Za-z_][A-Za-z0-9_$]*")

_CACHE_FORMAT: str = "sqlbuild-local-conversion/1"

type _CheckKey = tuple[str, str, str, bool]
type _CheckJob = tuple[str, dict[str, Any] | None, dict[str, str], bool]


class _LocalWork:
    """Checks shared by every thread of the process, and how much work they took."""

    def __init__(self) -> None:
        self._lock: threading.Lock = threading.Lock()
        self._checks: dict[_CheckKey, LocalSqlCheck] = {}
        self._schema_keys: dict[int, tuple[Mapping[str, Any], str]] = {}
        self._relation_names: dict[int, tuple[Mapping[str, Any], dict[str, list[str]]]] = {}
        self._counts: dict[str, int] = {
            "conversions": 0,
            "bindings": 0,
            "schema_keys": 0,
            "cached": 0,
        }
        self._empty: EmptyDuckDb | None = None
        self._cache_dir: Path | None = None

    def use_cache_dir(self, directory: Path | None) -> None:
        with self._lock:
            self._cache_dir = directory

    def cache_dir(self) -> Path | None:
        with self._lock:
            return self._cache_dir

    def schema_key(self, schema: Mapping[str, Any] | None) -> str:
        """The schema's JSON, serialised once per schema object."""

        if not schema:
            return ""
        with self._lock:
            known: tuple[Mapping[str, Any], str] | None = self._schema_keys.get(id(schema))
            if known is not None and known[0] is schema:
                return known[1]
        key: str = json.dumps(schema, sort_keys=True)
        with self._lock:
            self._schema_keys[id(schema)] = (schema, key)
            self._counts["schema_keys"] += 1
        return key

    def statement_schema(self, schema: Mapping[str, Any] | None, sql: str) -> dict[str, Any] | None:
        """Only the relations whose names `sql` mentions, so each call reads a small schema."""

        if not schema:
            return None
        with self._lock:
            known = self._relation_names.get(id(schema))
        if known is None or known[0] is not schema:
            by_name: dict[str, list[str]] = {}
            for relation in schema.get("relations", {}):
                by_name.setdefault(relation.rsplit(".", 1)[-1].upper(), []).append(relation)
            known = (schema, by_name)
            with self._lock:
                self._relation_names[id(schema)] = known
        relations: Mapping[str, Any] = schema.get("relations", {})
        mentioned: set[str] = {token.upper() for token in _IDENTIFIER.findall(sql)}
        return {
            **schema,
            "relations": {
                relation: relations[relation]
                for name in mentioned & known[1].keys()
                for relation in known[1][name]
            },
        }

    def get(self, key: _CheckKey) -> LocalSqlCheck | None:
        with self._lock:
            return self._checks.get(key)

    def missing(self, keys: Sequence[_CheckKey]) -> list[_CheckKey]:
        with self._lock:
            return list(dict.fromkeys(key for key in keys if key not in self._checks))

    def store(
        self, checks: Mapping[_CheckKey, LocalSqlCheck], *, counts: Mapping[str, int]
    ) -> None:
        with self._lock:
            self._checks.update(checks)
            for name, count in counts.items():
                self._counts[name] += count

    def counts(self) -> LocalWorkCounts:
        with self._lock:
            return LocalWorkCounts(**self._counts)

    def empty(self) -> EmptyDuckDb:
        """This process's empty DuckDB, opened on first use."""

        with self._lock:
            if self._empty is None:
                self._empty = EmptyDuckDb()
            return self._empty


_WORK: _LocalWork = _LocalWork()


def use_conversion_cache(directory: Path | None) -> None:
    """Keep conversions in `directory` across runs, keyed by SQL, schema and SQLQuack version."""

    _WORK.use_cache_dir(directory)


def converted(
    *, sql: str, schema: Mapping[str, Any] | None, time_zone: str | None, path: str
) -> LocalConversion:
    """SQLQuack's conversion of `sql`: DuckDB SQL and its macros, or the refusals."""

    conversion: LocalConversion | None = local_check(
        sql=sql, schema=schema, time_zone=time_zone, path=path
    ).conversion
    assert conversion is not None
    return conversion


def local_check(
    *,
    sql: str,
    schema: Mapping[str, Any] | None,
    time_zone: str | None,
    path: str,
    convert: bool = True,
) -> LocalSqlCheck:
    """`sql` converted when `convert`, else the first relation an empty DuckDB lacks for it."""

    key: _CheckKey = _key(sql=sql, schema=schema, time_zone=time_zone, convert=convert)
    cached: LocalSqlCheck | None = _WORK.get(key)
    if cached is not None:
        return cached
    job: _CheckJob = (
        sql,
        _WORK.statement_schema(schema, sql),
        _session(time_zone=time_zone),
        convert,
    )
    stored: LocalSqlCheck | None = _stored(job)
    if stored is not None:
        _WORK.store({key: stored}, counts={"cached": 1})
        return stored
    check: LocalSqlCheck = _check(job, empty=_WORK.empty(), path=path)
    _keep(job, check)
    _WORK.store({key: check}, counts=_check_counts(check))
    return check


def check_all(
    *,
    sqls: Sequence[str],
    schema: Mapping[str, Any] | None,
    time_zone: str | None,
    convert: bool,
) -> int:
    """Check in worker processes every statement not checked yet; how many they checked."""

    missing: list[_CheckKey] = _WORK.missing(
        [_key(sql=sql, schema=schema, time_zone=time_zone, convert=convert) for sql in sqls]
    )
    jobs_by_key: dict[_CheckKey, _CheckJob] = {
        key: (
            key[0],
            _WORK.statement_schema(schema, key[0]),
            _session(time_zone=time_zone),
            convert,
        )
        for key in missing
    }
    stored: dict[_CheckKey, LocalSqlCheck] = {
        key: check for key, job in jobs_by_key.items() if (check := _stored(job)) is not None
    }
    _WORK.store(stored, counts={"cached": len(stored)})
    missing = [key for key in missing if key not in stored]
    workers: int = min(_MAX_WORKERS, os.cpu_count() or 1)
    if len(missing) < _PARALLEL_MINIMUM or workers <= 1:
        return 0
    jobs: list[_CheckJob] = [jobs_by_key[key] for key in missing]
    with ProcessPoolExecutor(
        max_workers=workers, mp_context=multiprocessing.get_context("spawn")
    ) as pool:
        results: list[LocalSqlCheck] = list(
            pool.map(
                _worker_check,
                jobs,
                chunksize=max(1, len(jobs) // (workers * _CHUNKS_PER_WORKER)),
            )
        )
    totals: dict[str, int] = {"conversions": 0, "bindings": 0}
    for job, check in zip(jobs, results, strict=True):
        _keep(job, check)
        for name, count in _check_counts(check).items():
            totals[name] += count
    _WORK.store(dict(zip(missing, results, strict=True)), counts=totals)
    return len(results)


def warehouse_relations(
    *, sql: str, schema: Mapping[str, Any] | None, conversion: LocalConversion
) -> tuple[str, ...]:
    """Declared relations the converted SQL names in full, not via a fixture CTE: warehouse-only."""

    relations: Mapping[str, Any] = (_WORK.statement_schema(schema, sql) or {}).get("relations", {})
    return tuple(
        relation
        for relation in sorted(relations)
        if ".".join('"' + part.replace('"', '""') + '"' for part in relation.split("."))
        in conversion.sql
    )


def local_work_counts() -> LocalWorkCounts:
    """Conversions, bindings, schema serialisations and cache reads this process did or received."""

    return _WORK.counts()


def _key(
    *, sql: str, schema: Mapping[str, Any] | None, time_zone: str | None, convert: bool
) -> _CheckKey:
    return (sql, _WORK.schema_key(schema), time_zone or UTC_TIME_ZONE, convert)


def _session(*, time_zone: str | None) -> dict[str, str]:
    zone: str = time_zone or UTC_TIME_ZONE
    return {} if zone == UTC_TIME_ZONE else {TIMEZONE_PARAMETER: zone}


def _check(job: _CheckJob, *, empty: EmptyDuckDb, path: str) -> LocalSqlCheck:
    """Convert Snowflake SQL, whose unknown names SQLQuack refuses; bind DuckDB SQL as authored."""

    sql, schema, session, convert = job
    if not convert:
        return LocalSqlCheck(conversion=None, missing=empty.missing_relation(sql=sql, macros=()))
    conversion: Any = sqlquack.convert(
        sql=sql,
        options=ConvertOptions(
            no_config=True, schema=schema, session=session, compact=True, bind_check=False
        ),
        path=path,
    )
    return LocalSqlCheck(
        conversion=LocalConversion(
            converted=conversion.converted,
            sql=conversion.sql,
            macros=tuple(conversion.macros),
            refusals=tuple(conversion.report["refusals"]),
            rendered=conversion.rendered,
        ),
        missing=None,
        bound=False,
    )


def _cache_file(job: _CheckJob) -> Path | None:
    """Where a conversion of `job` is kept: a file named by the hash of everything it reads."""

    directory: Path | None = _WORK.cache_dir()
    sql, schema, session, convert = job
    if directory is None or not convert:
        return None
    digest: str = hashlib.sha256(
        json.dumps(
            [_CACHE_FORMAT, sqlquack.__version__, sql, schema, session], sort_keys=True
        ).encode()
    ).hexdigest()
    return directory / digest[:2] / f"{digest}.json"


def _stored(job: _CheckJob) -> LocalSqlCheck | None:
    """The kept conversion of `job`, if a readable one is there."""

    path: Path | None = _cache_file(job)
    if path is None:
        return None
    try:
        kept: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return LocalSqlCheck(
        conversion=LocalConversion(
            converted=bool(kept["converted"]),
            sql=str(kept["sql"]),
            macros=tuple(kept["macros"]),
            refusals=tuple(kept["refusals"]),
            rendered=str(kept["rendered"]),
        ),
        missing=None,
        bound=False,
    )


def _keep(job: _CheckJob, check: LocalSqlCheck) -> None:
    """Write `check`'s conversion for later runs; a write that fails only loses the shortcut."""

    path: Path | None = _cache_file(job)
    if path is None or check.conversion is None:
        return
    conversion: LocalConversion = check.conversion
    partial: Path = path.with_suffix(f".{os.getpid()}.{threading.get_ident()}.tmp")
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        partial.write_text(
            json.dumps(
                {
                    "converted": conversion.converted,
                    "sql": conversion.sql,
                    "macros": list(conversion.macros),
                    "refusals": list(conversion.refusals),
                    "rendered": conversion.rendered,
                }
            ),
            encoding="utf-8",
        )
        os.replace(partial, path)
    except OSError:
        partial.unlink(missing_ok=True)


def _check_counts(check: LocalSqlCheck) -> dict[str, int]:
    return {
        "conversions": int(check.conversion is not None),
        "bindings": int(check.bound),
    }


def _worker_check(job: _CheckJob) -> LocalSqlCheck:
    return _check(job, empty=_WORK.empty(), path=DEFAULT_PATH)
