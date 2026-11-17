from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import pytest


@dataclass(frozen=True)
class BackgroundSqlTestPlanningTestCase:
    description: str
    enabled: bool
    expected_planner_calls: int
    expected_error: str | None = None


@dataclass(frozen=True)
class MigrationFingerprintCacheTestCase:
    description: str
    requests: tuple[tuple[str, dict[str, str], str], ...]
    expected_computations: int


@dataclass(frozen=True)
class PersistedMigrationFingerprintTestCase:
    description: str
    between_runs: Callable[[Path, pytest.MonkeyPatch], None]
    expected_second_run_computations: int


@dataclass(frozen=True)
class FixtureColumnInferencesTestCase:
    description: str
    requests: tuple[tuple[str, str], ...]
    expected_inferences: int


@dataclass(frozen=True)
class PlanningExitTestCase:
    description: str
    script: str
    expected_returncode: int
    expected_output: str


@dataclass(frozen=True)
class DialectAttributeTestCase:
    description: str
    attribute: str
    arguments: dict[str, object]
    expected: object


@dataclass(frozen=True)
class DialectRefusalTestCase:
    description: str
    attribute: str
