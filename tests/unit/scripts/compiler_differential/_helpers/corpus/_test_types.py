"""Test case types for the differential harness corpus helpers."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.compiler_differential.models import DifferentialCommand, ExpectedOutcome


@dataclass(frozen=True)
class CorpusExpectationTestCase:
    """A corpus selection and the expectation each named entry must declare."""

    description: str
    corpora: tuple[str, ...]
    expected_outcomes: dict[str, ExpectedOutcome]


@dataclass(frozen=True)
class InlineCompileCodesTestCase:
    """The compile package and the codes every inline render-stage literal must belong to."""

    description: str
    compile_root: Path
    accounted_codes: frozenset[str]
    expected_unaccounted: dict[str, tuple[str, ...]]


@dataclass(frozen=True)
class DialectVariantTestCase:
    """A seed range and the dialect variants its first seed must add to the corpus."""

    description: str
    seeds: range
    expected_names: tuple[str, ...]
    expected_variant_commands: tuple[DifferentialCommand, ...]


@dataclass(frozen=True)
class AnalysisCodesTestCase:
    """The compiler package and the analysis codes its scan must find and document."""

    description: str
    compiler_root: Path
    expected_codes: frozenset[str]


@dataclass(frozen=True)
class DiscoveryCodesTestCase:
    """The discovery codes no failure-corpus case declares, which must be none."""

    description: str
    expected_undeclared_codes: frozenset[str]


@dataclass(frozen=True)
class FailureGoldensTestCase:
    """The golden directory and the failure cases with no golden there, which must be none."""

    description: str
    golden_dir: Path
    expected_missing: frozenset[str]
