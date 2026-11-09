"""Native type normalization."""

from __future__ import annotations

import logging

import sqlbuild._native as _native
from sqlbuild.adapter.contract.models import NormalizedType
from sqlbuild.adapter.contract.types import TypeDialect, TypeFamily
from sqlbuild.adapter.type_system.constants import (
    TYPE_NORMALIZATION_LOGGER_NAME,
    TYPE_PARSE_FALLBACK_MESSAGE,
)
from sqlbuild.diagnostics.main.log_debug_event import log_debug_event


def normalize_native_type(*, type_sql: str, dialect: TypeDialect | str | None) -> NormalizedType:
    """Return the native normalization of one type, raising ValueError for an unknown dialect."""

    (normalized_name, family, precision, scale, length), parse_error = _native.normalize_type(
        type_sql, str(dialect or "generic")
    )
    if parse_error is not None:
        log_debug_event(
            logger=logging.getLogger(TYPE_NORMALIZATION_LOGGER_NAME),
            message=TYPE_PARSE_FALLBACK_MESSAGE,
            type_sql=type_sql,
            dialect=str(dialect),
            sqlbuild_error=parse_error,
        )
    return NormalizedType(
        normalized_name=normalized_name,
        family=TypeFamily(family),
        precision=_python_int(precision),
        scale=_python_int(scale),
        length=_python_int(length),
    )


def _python_int(value: str | None) -> int | None:
    return None if value is None else int(value)
