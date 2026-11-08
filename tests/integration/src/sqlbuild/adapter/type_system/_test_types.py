"""Test case types for native type normalization."""

from __future__ import annotations

from dataclasses import dataclass

from sqlbuild.adapter.contract.models import NormalizedType
from sqlbuild.adapter.contract.types import TypeFamily


@dataclass(frozen=True)
class GeneratedTypeTestCase:
    """Seeded generated type strings normalized under every dialect."""

    description: str
    seed: int
    count: int
    expected_minimum_normalized: int
    expected_minimum_parse_errors: int
    expected_unknown_dialect_errors: int


@dataclass(frozen=True)
class TypeNormalizationTestCase:
    """One type string under one dialect, its normalization and whether a parse error is logged."""

    description: str
    type_sql: str
    dialect: str | None
    expected_type: NormalizedType
    expected_parse_error_logged: bool


@dataclass(frozen=True)
class UnknownDialectTypeTestCase:
    """A dialect SQLBuild has no type rules for, and the error normalization raises for it."""

    description: str
    dialect: str
    expected_error: str


@dataclass(frozen=True)
class DeepTypeTestCase:
    """A deeply nested type the Python wheel normalized."""

    description: str
    type_sql: str
    expected_family: TypeFamily


@dataclass(frozen=True)
class PublicNativeTypeTestCase:
    """Types normalized through the public entry point."""

    description: str
    dialect: str
    type_strings: tuple[str, ...]
    expected_native_calls: tuple[tuple[str, str], ...]
