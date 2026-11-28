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
from sqlquack.api.constants import DEFAULT_PATH

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.local._helpers.local_inputs import convert_options, relied_on
from sqlbuild.executor.local.classes.empty_duckdb import EmptyDuckDb
from sqlbuild.executor.local.constants import (
    CONVERSION_CACHE_INDEX,
    CONVERSION_CACHE_LIFETIME_SECONDS,
    TIMEZONE_PARAMETER,
    UTC_TIME_ZONE,
)
from sqlbuild.executor.local.models import (
    ConvertedSql,
    LocalInputs,
    LocalSqlCheck,
    LocalWorkCounts,
)

_PARALLEL_MINIMUM: int = 8
_MAX_WORKERS: int = 4
_CHUNKS_PER_WORKER: int = 4
_IDENTIFIER: re.Pattern[str] = re.compile(r"[A-Za-z_][A-Za-z0-9_$]*")

_CACHE_FORMAT: str = "sqlbuild-local-conversion/2"

type _CheckKey = tuple[str, str, str, bool, str]
type _CheckJob = tuple[str, dict[str, Any] | None, dict[str, str], bool, LocalInputs]


class _LocalWork:
    """Checks shared by every thread of the process, and how much work they took."""

    def __init__(self) -> None:
        self._lock: threading.Lock = threading.Lock()
        self._checks: dict[_CheckKey, LocalSqlCheck] = {}
        self._schema_keys: dict[int, tuple[Mapping[str, Any], str]] = {}
        self._relation_names: dict[int, tuple[Mapping[str, Any], dict[str, list[str]]]] = {}
        self._slices: dict[int, dict[str, dict[str, Any]]] = {}
        self._counts: dict[str, int] = {
            "conversions": 0,
            "bindings": 0,
            "schema_keys": 0,
            "cached": 0,
        }
        self._empty: EmptyDuckDb | None = None
        self._cache_dir: Path | None = None
        self._used: set[str] = set()
        self._inputs: LocalInputs = LocalInputs()

    def use_inputs(self, inputs: LocalInputs) -> None:
        with self._lock:
            self._inputs = inputs

    def inputs(self) -> LocalInputs:
        with self._lock:
            return self._inputs

    def use_cache_dir(self, directory: Path | None) -> None:
        with self._lock:
            self._cache_dir = directory

    def cache_dir(self) -> Path | None:
        with self._lock:
            return self._cache_dir

    def mark_used(self, path: Path) -> None:
        with self._lock:
            self._used.add(path.stem)

    def take_used(self) -> set[str]:
        """The cache entries this process read or wrote since the last call."""

        with self._lock:
            used: set[str] = self._used
            self._used = set()
            return used

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

    def statement_schema(
        self, *, schema: Mapping[str, Any] | None, sql: str
    ) -> dict[str, Any] | None:
        """Only the relations whose names `sql` mentions, so each call reads a small schema."""

        if not schema:
            return None
        with self._lock:
            known: tuple[Mapping[str, Any], dict[str, list[str]]] | None = self._relation_names.get(
                id(schema)
            )
        if known is None or known[0] is not schema:
            by_name: dict[str, list[str]] = {}
            for relation in schema.get("relations", {}):
                by_name.setdefault(relation.rsplit(".", 1)[-1].upper(), []).append(relation)
            known = (schema, by_name)
            with self._lock:
                self._relation_names[id(schema)] = known
                self._slices[id(schema)] = {}
        with self._lock:
            sliced: dict[str, Any] | None = self._slices.get(id(schema), {}).get(sql)
        if sliced is not None:
            return sliced
        relations: Mapping[str, Any] = schema.get("relations", {})
        mentioned: set[str] = {token.upper() for token in _IDENTIFIER.findall(sql)}
        read: dict[str, Any] = {}
        for name in mentioned & known[1].keys():
            for relation in known[1][name]:
                read[relation] = relations[relation]
        sliced = {**schema, "relations": read}
        with self._lock:
            self._slices.setdefault(id(schema), {})[sql] = sliced
        return sliced

    def get(self, key: _CheckKey) -> LocalSqlCheck | None:
        with self._lock:
            return self._checks.get(key)

    def missing(self, keys: Sequence[_CheckKey]) -> list[_CheckKey]:
        with self._lock:
            return list(dict.fromkeys(key for key in keys if key not in self._checks))

    def store(
        self, *, checks: Mapping[_CheckKey, LocalSqlCheck], counts: Mapping[str, int]
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


def use_local_inputs(inputs: LocalInputs) -> None:
    """Convert with the project's allowances and overrides, which key every conversion too."""

    _WORK.use_inputs(inputs)


def converted(
    *, sql: str, schema: Mapping[str, Any] | None, time_zone: str | None, path: str
) -> ConvertedSql:
    """SQLQuack's conversion of `sql`: DuckDB SQL and its macros, or the refusals."""

    conversion: ConvertedSql | None = local_check(
        sql=sql, schema=schema, time_zone=time_zone, path=path
    ).conversion
    if conversion is None:
        raise ExecutorInputError(f"SQL checked for conversion was not converted: {path}")
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
        _WORK.statement_schema(schema=schema, sql=sql),
        _session(time_zone=time_zone),
        convert,
        _WORK.inputs(),
    )
    stored: LocalSqlCheck | None = _stored(job)
    if stored is not None:
        _WORK.store(checks={key: stored}, counts={"cached": 1})
        return stored
    check: LocalSqlCheck = _check(job=job, empty=_WORK.empty(), path=path)
    _keep(job=job, check=check)
    _WORK.store(checks={key: check}, counts=_check_counts(check))
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
            _WORK.statement_schema(schema=schema, sql=key[0]),
            _session(time_zone=time_zone),
            convert,
            _WORK.inputs(),
        )
        for key in missing
    }
    stored: dict[_CheckKey, LocalSqlCheck] = {
        key: check for key, job in jobs_by_key.items() if (check := _stored(job)) is not None
    }
    _WORK.store(checks=stored, counts={"cached": len(stored)})
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
        _keep(job=job, check=check)
        for name, count in _check_counts(check).items():
            totals[name] += count
    _WORK.store(checks=dict(zip(missing, results, strict=True)), counts=totals)
    return len(results)


def warehouse_relations(
    *, sql: str, schema: Mapping[str, Any] | None, conversion: ConvertedSql
) -> tuple[str, ...]:
    """Declared relations the converted SQL names in full, not via a fixture CTE: warehouse-only."""

    relations: Mapping[str, Any] = (_WORK.statement_schema(schema=schema, sql=sql) or {}).get(
        "relations", {}
    )
    return tuple(
        relation for relation in sorted(relations) if _quoted_name(relation) in conversion.sql
    )


def _quoted_name(relation: str) -> str:
    """`DB.SCHEMA.TABLE` as converted SQL names it: `"DB"."SCHEMA"."TABLE"`."""

    parts: list[str] = ['"' + part.replace('"', '""') + '"' for part in relation.split(".")]
    return ".".join(parts)


def _key(
    *, sql: str, schema: Mapping[str, Any] | None, time_zone: str | None, convert: bool
) -> _CheckKey:
    return (
        sql,
        _WORK.schema_key(schema),
        time_zone or UTC_TIME_ZONE,
        convert,
        _WORK.inputs().digest,
    )


def _session(*, time_zone: str | None) -> dict[str, str]:
    zone: str = time_zone or UTC_TIME_ZONE
    return {} if zone == UTC_TIME_ZONE else {TIMEZONE_PARAMETER: zone}


def _check(*, job: _CheckJob, empty: EmptyDuckDb, path: str) -> LocalSqlCheck:
    """Convert Snowflake SQL, whose unknown names SQLQuack refuses; bind DuckDB SQL as authored."""

    sql, schema, session, convert, inputs = job
    if not convert:
        return LocalSqlCheck(conversion=None, missing=empty.missing_relation(sql=sql, macros=()))
    conversion: Any = sqlquack.convert(
        sql=sql,
        options=convert_options(inputs=inputs, schema=schema, session=session),
        path=path,
    )
    return LocalSqlCheck(
        conversion=ConvertedSql(
            converted=conversion.converted,
            sql=conversion.sql,
            macros=tuple(conversion.macros),
            refusals=tuple(conversion.report["refusals"]),
            rendered=conversion.rendered,
            relied_on=relied_on(conversion.report),
        ),
        missing=None,
        bound=False,
    )


def _cache_file(job: _CheckJob) -> Path | None:
    """Where a conversion of `job` is kept: a file named by the hash of everything it reads."""

    directory: Path | None = _WORK.cache_dir()
    sql, schema, session, convert, inputs = job
    if directory is None or not convert:
        return None
    digest: str = hashlib.sha256(
        json.dumps(
            [_CACHE_FORMAT, sqlquack.__version__, sql, schema, session, inputs.digest],
            sort_keys=True,
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
    _WORK.mark_used(path)
    return LocalSqlCheck(
        conversion=ConvertedSql(
            converted=bool(kept["converted"]),
            sql=str(kept["sql"]),
            macros=tuple(kept["macros"]),
            refusals=tuple(kept["refusals"]),
            rendered=str(kept["rendered"]),
            relied_on=tuple(kept["relied_on"]),
        ),
        missing=None,
        bound=False,
    )


def _keep(*, job: _CheckJob, check: LocalSqlCheck) -> None:
    """Write `check`'s conversion for later runs; a write that fails only loses the shortcut."""

    path: Path | None = _cache_file(job)
    if path is None or check.conversion is None:
        return
    conversion: ConvertedSql = check.conversion
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
                    "relied_on": list(conversion.relied_on),
                }
            ),
            encoding="utf-8",
        )
        os.replace(partial, path)
    except OSError:
        partial.unlink(missing_ok=True)
        return
    _WORK.mark_used(path)


def prune_conversion_cache(*, now: float) -> int:
    """Record this run's use of kept conversions, then delete those unused for a week; how many."""

    directory: Path | None = _WORK.cache_dir()
    used: set[str] = _WORK.take_used()
    if directory is None or not directory.is_dir():
        return 0
    index: Path = directory / CONVERSION_CACHE_INDEX
    last_used: dict[str, float] = {**_last_used(index), **dict.fromkeys(used, now)}
    kept: dict[str, float] = {}
    pruned: int = 0
    for entry in directory.glob("*/*.json"):
        stamp: float = last_used.get(entry.stem, now)
        if now - stamp <= CONVERSION_CACHE_LIFETIME_SECONDS:
            kept[entry.stem] = stamp
            continue
        try:
            entry.unlink()
        except OSError:
            kept[entry.stem] = stamp
            continue
        pruned += 1
    _write_last_used(index=index, last_used=kept)
    return pruned


def _last_used(index: Path) -> dict[str, float]:
    """When each kept conversion was last used, by entry name; empty when unreadable."""

    try:
        loaded: Any = json.loads(index.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    if not isinstance(loaded, dict):
        return {}
    return {
        str(name): float(stamp)
        for name, stamp in loaded.items()
        if isinstance(stamp, int | float) and not isinstance(stamp, bool)
    }


def _write_last_used(*, index: Path, last_used: Mapping[str, float]) -> None:
    partial: Path = index.with_suffix(f".{os.getpid()}.{threading.get_ident()}.tmp")
    try:
        partial.write_text(json.dumps(last_used, sort_keys=True), encoding="utf-8")
        os.replace(partial, index)
    except OSError:
        partial.unlink(missing_ok=True)


def _check_counts(check: LocalSqlCheck) -> dict[str, int]:
    return {
        "conversions": int(check.conversion is not None),
        "bindings": int(check.bound),
    }


def _worker_check(job: _CheckJob) -> LocalSqlCheck:
    return _check(job=job, empty=_WORK.empty(), path=DEFAULT_PATH)
