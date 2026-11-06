"""Test-case models for the release performance comparison."""

from dataclasses import dataclass

from scripts.release_performance.models import BenchmarkCommand, CommandComparison, SkippedCommand


@dataclass(frozen=True)
class ConfiguredRssLimitTestCase:
    description: str
    command: BenchmarkCommand
    expected_rss_ratio: float


@dataclass(frozen=True)
class MetricVerdictTestCase:
    description: str
    comparison: CommandComparison
    expected_regressed: tuple[bool, bool, bool]


@dataclass(frozen=True)
class ConfiguredTimeLimitTestCase:
    description: str
    command: str
    wall_ratio: float
    expected_regressed: bool


@dataclass(frozen=True)
class SkipReasonTestCase:
    description: str
    minimum_version: str | None
    baseline_version: str
    candidate_version: str
    expected_reason: str | None


@dataclass(frozen=True)
class ComparisonMarkdownTestCase:
    description: str
    commands: tuple[CommandComparison, ...]
    skipped: tuple[SkippedCommand, ...]
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class PreviousVersionTestCase:
    description: str
    candidate: str
    versions: tuple[str, ...]
    expected_version: str | None


@dataclass(frozen=True)
class ReleaseVersionsTestCase:
    description: str
    files: tuple[dict[str, object], ...]
    expected_versions: tuple[str, ...]


@dataclass(frozen=True)
class CompatibleWheelsTestCase:
    description: str
    machine: str
    system: str
    expected_filenames: tuple[str, ...]


@dataclass(frozen=True)
class ChooseBaselineTestCase:
    description: str
    candidate: str
    files: tuple[dict[str, object], ...]
    same_version: bool
    expected_baseline: str | None


@dataclass(frozen=True)
class BaselineGeneratorTestCase:
    description: str
    generator: str
    expected_projects: tuple[str, ...]
    expected_error_fragments: tuple[str, ...]


@dataclass(frozen=True)
class BaselineDenseGeneratorTestCase:
    description: str
    generator: str
    expected_markers: tuple[str, ...]
    expected_error_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ReleaseSourceTestCase:
    description: str
    tag: str
    requested_version: str
    expected_files: tuple[str, ...]
    expected_error_fragments: tuple[str, ...]
