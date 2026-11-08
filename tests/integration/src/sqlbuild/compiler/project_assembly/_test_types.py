from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class GeneratedAssemblyParityTestCase:
    """Seeded generated projects whose resource assembly is checked natively."""

    description: str
    seed: int
    count: int
    model_count: int
    environment: dict[str, str]
    expected_minimum_environment_reads: int


@dataclass(frozen=True)
class AssemblyDeferralTestCase:
    """A project whose model or hook SQL the native parser cannot read."""

    description: str
    files: dict[str, str]
    expected_warning: str


@dataclass(frozen=True)
class EdgeAssemblyTestCase:
    """Projects whose assembly raises or reads Python-only values, against Python's golden."""

    description: str
    projects: tuple[tuple[str, dict[str, str], dict[str, object] | None], ...]
    expected_raised: int


@dataclass(frozen=True)
class WindowsEnvironmentTestCase:
    """A seed schema reading ENV through a mapping that upper-cases keys, as Windows does."""

    description: str
    environment: dict[str, str]
    seed_schema: str
    expected_schema: str


@dataclass(frozen=True)
class OptOutTestCase:
    """Models opting out of required SQL analysis, and which ones parse so Python rejects them."""

    description: str
    models: dict[str, str]
    expected_rejected: dict[str, bool]


@dataclass(frozen=True)
class TruncatedHookTestCase:
    """A project whose hook SQL ends mid-expression, which the dialect's parser reports."""

    description: str
    files: dict[str, str]
    dialect: str
    expected_message_fragment: str
