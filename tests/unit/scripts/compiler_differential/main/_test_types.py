"""Test case types for the generator and failure corpus entry points."""

from __future__ import annotations

from dataclasses import dataclass

from scripts.compiler_differential.models import FailureCase


@dataclass(frozen=True)
class SeedDeterminismTestCase:
    """One seed generated twice."""

    description: str
    seed: int
    expected_identical: bool


@dataclass(frozen=True)
class SeedRangeTestCase:
    """A seed range and what the generated projects must cover together."""

    description: str
    seeds: range
    expected_features: frozenset[str]
    expected_distinct_projects: int
    expected_min_invalid: int
    expected_max_invalid: int


@dataclass(frozen=True)
class FailureCorpusTestCase:
    """The failure corpus and the breadth it must keep."""

    description: str
    expected_min_cases: int
    expected_min_codes: int
    expected_min_families: int


@dataclass(frozen=True)
class FailureCaseWriteTestCase:
    """One failure case and the files its written project must contain."""

    description: str
    case: FailureCase
    expected_files: tuple[str, ...]


@dataclass(frozen=True)
class FeatureBlockWindowTestCase:
    """A window of consecutive seeds and the feature blocks it must force."""

    description: str
    seeds: range
    expected_blocks: frozenset[str]


@dataclass(frozen=True)
class RareFeatureSeedTestCase:
    """A seed carrying a rare block and the outcome it must declare."""

    description: str
    seed: int
    expected_feature: str
    expected_error_code: str
    expected_succeeding_commands: tuple[str, ...]
    other_seeds: tuple[int, ...]


@dataclass(frozen=True)
class CoverageFlagUsageTestCase:
    """A coverage requirement given without what it needs, and the usage error it reports."""

    description: str
    arguments: tuple[str, ...]
    expected_message: str


@dataclass(frozen=True)
class ExcludedFeaturePairTestCase:
    description: str
    seeds: range
    pair: tuple[str, str]
    expected_any_with_both: bool
