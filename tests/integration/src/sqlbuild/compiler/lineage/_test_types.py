from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class GeneratedLineageParityTestCase:
    """Seeded generated projects whose fast lineage must not depend on the analysis catalog."""

    description: str
    seed: int
    count: int
    dialects: tuple[str | None, ...]
    expected_minimum_native: int
    expected_minimum_parsed: int


@dataclass(frozen=True)
class RootStarLineageTestCase:
    """A root star over a model without SQL analysis, whose facts leave the star to fast lineage."""

    description: str
    dialect: str | None
    expected_star_expansions: int


@dataclass(frozen=True)
class FormerlyDeferredLineageTestCase:
    """Parsed models Python once built, now answered natively or raising Python's error."""

    description: str
    dialect: str
    keeps_catalog: bool
    expected_models_with_lineage: int
    expected_error: str | None


@dataclass(frozen=True)
class UnparsedLineageTestCase:
    """A model without compact facts whose SQL does not parse, so it has no fast lineage."""

    description: str
    query_sql: str
    expected_messages: tuple[str, ...]


@dataclass(frozen=True)
class TruncatedSqlLineageTestCase:
    """A model whose SQL ends mid-expression, which fast lineage leaves out."""

    description: str
    dialect: str
    query_sql: str
    expected_has_model: bool
