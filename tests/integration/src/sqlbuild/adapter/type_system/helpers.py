"""Generated type strings and their native normalization outcomes."""

from __future__ import annotations

import logging
import random
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from itertools import product

from sqlbuild.adapter.contract.models import NormalizedType
from sqlbuild.adapter.type_system.constants import TYPE_NORMALIZATION_LOGGER_NAME
from sqlbuild.adapter.type_system.main._native_normalize_type import normalize_native_type

DIALECTS: tuple[str | None, ...] = (
    None,
    "generic",
    "bigquery",
    "snowflake",
    "duckdb",
    "motherduck",
    "databricks",
    "postgres",
    "tsql",
    "postgresql",
    "DuckDB",
    "mysql",
)
_BASE_TYPES: tuple[str, ...] = (
    "INT",
    "INTEGER",
    "BIGINT",
    "SMALLINT",
    "TINYINT",
    "INT64",
    "LONG",
    "int",
    "Integer",
    "DECIMAL",
    "NUMERIC",
    "NUMBER",
    "BIGNUMERIC",
    "BIGDECIMAL",
    "DEC",
    "FLOAT",
    "FLOAT64",
    "DOUBLE",
    "REAL",
    "DOUBLE PRECISION",
    "VARCHAR",
    "CHAR",
    "CHARACTER",
    "CHARACTER VARYING",
    "STRING",
    "TEXT",
    "NVARCHAR",
    "BOOL",
    "BOOLEAN",
    "TIMESTAMP",
    "TIMESTAMP_NTZ",
    "TIMESTAMP_LTZ",
    "TIMESTAMP_TZ",
    "TIMESTAMPTZ",
    "TIMESTAMPNTZ",
    "TIMESTAMP WITH TIME ZONE",
    "TIMESTAMP WITHOUT TIME ZONE",
    "DATE",
    "DATETIME",
    "TIME",
    "INTERVAL",
    "JSON",
    "VARIANT",
    "BYTES",
    "BLOB",
    "UUID",
    "GEOGRAPHY",
    "HUGEINT",
    "MONEY",
    '"quoted"',
    '"Mixed Case"',
    "order_status",
    "foo bar",
    "1abc",
    "INT;",
    "",
)
_PARAMETERS: tuple[str, ...] = (
    "",
    "(10)",
    "(10,2)",
    "(10, 2)",
    "( 38 , 0 )",
    "(0)",
    "(-1)",
    "(1_000)",
    "(MAX)",
    "(abc)",
    "()",
    "(99999999999999999999)",
    "(10,2,3)",
    "(+5)",
)
_PADDING: tuple[str, ...] = ("", " ", "\t", "\n", " \n")
_MAX_NESTING: int = 3
_NESTED_TEMPLATES: tuple[str, ...] = (
    "ARRAY<{0}>",
    "{0}[]",
    "STRUCT<a {0}, b {1}>",
    "STRUCT(a {0})",
    "MAP({0}, {1})",
)
_TEMPLATES_BY_DEPTH: tuple[tuple[str, ...], ...] = (
    *((("{scalar}",) * 15 + _NESTED_TEMPLATES,) * _MAX_NESTING),
    ("{scalar}",),
)
_CASINGS: tuple[Callable[[str], str], ...] = (str,) * 6 + (str.lower,)
_SPACINGS: tuple[str, ...] = (" ", "  ")


@dataclass(frozen=True)
class TypeOutcome:
    """One type string and dialect: its normalization or Python's error, and the errors logged."""

    type_sql: str
    dialect: str | None
    normalized: NormalizedType | str
    logged_errors: tuple[tuple[str, object, object, object], ...]


def generated_type(*, rng: random.Random, depth: int = 0) -> str:
    """Return a type string: scalar, parameterized, nested, quoted, padded or invalid."""

    children: list[str] = [
        generated_type(rng=rng, depth=depth + 1) for _ in range(2 * (depth < _MAX_NESTING))
    ]
    template: str = rng.choice(_TEMPLATES_BY_DEPTH[min(depth, _MAX_NESTING)])
    return template.format(*children, scalar=_generated_scalar(rng=rng))


def type_outcomes(*, type_strings: list[str]) -> list[TypeOutcome]:
    """Normalize every type string under every dialect."""

    return [
        type_outcome(type_sql=type_sql, dialect=dialect)
        for type_sql, dialect in product(type_strings, DIALECTS)
    ]


def type_outcome(*, type_sql: str, dialect: str | None) -> TypeOutcome:
    """Normalize one type natively, recording the parse errors it logs."""

    with _logged_errors() as errors:
        normalized: NormalizedType | str
        try:
            normalized = normalize_native_type(type_sql=type_sql, dialect=dialect)
        except ValueError as error:
            normalized = f"{type(error).__name__}: {error}"
    return TypeOutcome(
        type_sql=type_sql, dialect=dialect, normalized=normalized, logged_errors=tuple(errors)
    )


def is_normalized(outcome: TypeOutcome) -> bool:
    """Return whether the type normalized rather than raising."""

    return isinstance(outcome.normalized, NormalizedType)


def logged_parse_error(outcome: TypeOutcome) -> bool:
    """Return whether a type parse failure was logged for this type."""

    return bool(outcome.logged_errors)


def _generated_scalar(*, rng: random.Random) -> str:
    type_sql: str = rng.choice(_CASINGS)(rng.choice(_BASE_TYPES) + rng.choice(_PARAMETERS))
    return (
        rng.choice(_PADDING) + type_sql.replace(" ", rng.choice(_SPACINGS)) + rng.choice(_PADDING)
    )


@contextmanager
def _logged_errors() -> Iterator[list[tuple[str, object, object, object]]]:
    records: list[tuple[str, object, object, object]] = []
    handler: _RecordingHandler = _RecordingHandler(records=records)
    logger: logging.Logger = logging.getLogger(TYPE_NORMALIZATION_LOGGER_NAME)
    previous_level: int = logger.level
    logger.addHandler(handler)
    logger.setLevel(logging.DEBUG)
    try:
        yield records
    finally:
        logger.removeHandler(handler)
        logger.setLevel(previous_level)


class _RecordingHandler(logging.Handler):
    def __init__(self, *, records: list[tuple[str, object, object, object]]) -> None:
        super().__init__(level=logging.DEBUG)
        self._records: list[tuple[str, object, object, object]] = records

    def emit(self, record: logging.LogRecord) -> None:
        self._records.append(
            (
                record.getMessage(),
                getattr(record, "type_sql", None),
                getattr(record, "dialect", None),
                getattr(record, "sqlbuild_error", None),
            )
        )


def dialect_outcomes(outcomes: list[TypeOutcome]) -> dict[str, tuple[object, bool]]:
    """One type string's normalization and whether it logged errors, by dialect."""

    return {
        str(outcome.dialect): (outcome.normalized, bool(outcome.logged_errors))
        for outcome in outcomes
    }
