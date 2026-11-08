from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class GeneratedSemanticParityTestCase:
    """Seeded failing projects whose native completion matches the recorded Python outputs."""

    description: str
    seed: int
    count: int
    model_count: int
    dialects: tuple[str, ...]
    expected_minimum_native_completions: int
    expected_minimum_native_diagnostics: int
    expected_minimum_type_recovery_plans: int
    expected_minimum_codes: dict[str, int]


@dataclass(frozen=True)
class FormerlyDeferredSemanticTestCase:
    """Projects the native stage once handed back to Python, now answered or raising natively."""

    description: str
    seed: int
    count: int
    non_ascii: bool
    dialects: tuple[str | None, ...]
    expected_outcomes: frozenset[str]


@dataclass(frozen=True)
class DialectFindingTestCase:
    """One model compiled under one dialect: the findings and output types it must have."""

    description: str
    dialect: str
    sql: str
    expected_codes: tuple[str, ...]
    expected_types: tuple[tuple[str, str | None], ...]


@dataclass(frozen=True)
class ModelFindingsTestCase:
    """Models compiled together under one dialect: each finding's model and code."""

    description: str
    dialect: str
    models: dict[str, str]
    expected_findings: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class MissingCatalogTestCase:
    """A project without the analysis catalog every compile builds."""

    description: str
    expected_message: str


@dataclass(frozen=True)
class GeneratedMetadataParityTestCase:
    """Seeded projects whose native metadata checks match the recorded Python outputs."""

    description: str
    seed: int
    count: int
    model_count: int
    dialects: tuple[str | None, ...]
    non_ascii: bool
    golden_prefix: str
    expected_minimum_native: int
    expected_minimum_families: dict[str, int]


@dataclass(frozen=True)
class SessionCompletionTestCase:
    """Seeded preview compiles whose completion reads analysed models from the native session."""

    description: str
    corpus: str
    seed: int
    count: int
    model_count: int
    expected_session_models: int
    expected_payload_models: int
    expected_proven_outputs: int
    expected_minimum_codes: dict[str, int]
