"""Typed records for comparing a release candidate with the previous published release."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class BenchmarkCommand:
    name: str
    project: str
    sqb_args: tuple[str, ...]
    minimum_version: str | None = None
    edits_model: bool = False
    removes_target: bool = False
    max_time_ratio: float | None = None
    max_rss_ratio: float | None = None


@dataclass(frozen=True)
class InstalledVersion:
    label: str
    version: str
    sqb: Path
    python: Path


@dataclass(frozen=True)
class CommandSample:
    wall_seconds: float
    cpu_seconds: float
    peak_rss_bytes: int


@dataclass(frozen=True)
class CommandComparison:
    name: str
    baseline: tuple[CommandSample, ...]
    candidate: tuple[CommandSample, ...]
    max_time_ratio: float | None = None
    max_rss_ratio: float | None = None


@dataclass(frozen=True)
class SkippedCommand:
    name: str
    reason: str


@dataclass(frozen=True)
class MetricVerdict:
    command: str
    metric: str
    baseline: float
    candidate: float
    max_ratio: float
    floor: float
    regressed: bool

    @property
    def ratio(self) -> float:
        return self.candidate / self.baseline if self.baseline else float("inf")


@dataclass(frozen=True)
class RunnerContext:
    cpu_model: str
    cpu_count: int
    load_average_before: tuple[float, float, float]
    load_average_after: tuple[float, float, float]


@dataclass(frozen=True)
class ReleaseComparison:
    baseline_version: str
    candidate_version: str
    baseline_generator: str
    runs: int
    runner: RunnerContext
    commands: tuple[CommandComparison, ...]
    skipped: tuple[SkippedCommand, ...]


@dataclass(frozen=True)
class ComparisonOptions:
    candidate: str
    baseline: str | None
    runs: int
    python: str
    inspection_models: int
    build_models: int
    dense_models: int
    work_dir: Path | None
    output: Path | None
    baseline_source: Path | None = None
    same_version_baseline: bool = False
