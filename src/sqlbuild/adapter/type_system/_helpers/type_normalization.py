"""Public type normalization capability for adapter comparisons."""

from __future__ import annotations

from functools import lru_cache

from sqlbuild.adapter.contract.models import NormalizedType
from sqlbuild.adapter.contract.types import TypeDialect, TypeFamily
from sqlbuild.adapter.type_system.main._native_normalize_type import normalize_native_type


@lru_cache(maxsize=1024)
def normalize_type(*, type_sql: str, dialect: TypeDialect | str | None) -> NormalizedType:
    """Normalize one warehouse type string into a semantic comparison shape."""

    return normalize_native_type(type_sql=type_sql, dialect=dialect)


def normalize_numeric_family(*, type_sql: str, dialect: TypeDialect | str | None) -> str | None:
    """Return the normalized numeric family for one type, if numeric."""

    family: TypeFamily = normalize_type(type_sql=type_sql, dialect=dialect).family
    if family in {TypeFamily.INTEGER, TypeFamily.DECIMAL, TypeFamily.FLOAT}:
        return family
    return None


def types_equal(*, left: str, right: str, dialect: TypeDialect | str | None) -> bool:
    """Return whether two type strings are semantically equivalent."""

    left_type: NormalizedType = normalize_type(type_sql=left, dialect=dialect)
    right_type: NormalizedType = normalize_type(type_sql=right, dialect=dialect)
    if left_type == right_type:
        return True
    return (
        str(dialect) in {TypeDialect.DUCKDB, TypeDialect.MOTHERDUCK}
        and left_type.family == TypeFamily.STRING
        and right_type.family == TypeFamily.STRING
    )
