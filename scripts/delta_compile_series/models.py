"""Edits, compile outcomes and series results of delta compile series."""

from __future__ import annotations

from dataclasses import dataclass

from scripts.delta_compile_series.types import EditTarget


@dataclass(frozen=True)
class DeltaCompileEdit:
    """One edit a series can make, and the fallback reason it must give when not a delta."""

    kind: str
    pattern: str
    replacement: str
    target: EditTarget = EditTarget.MODELS
    fallback_reason: str | None = None


@dataclass(frozen=True)
class DeltaCompileOutcome:
    """One compile's exit code and output."""

    returncode: int
    stdout: str
    stderr: str
    command: tuple[str, ...] = ()
    environment: tuple[tuple[str, str], ...] = ()


@dataclass(frozen=True)
class DeltaCompileSeries:
    """What a seeded series found: each step, the deltas it took and anything unexpected."""

    steps: tuple[str, ...]
    deltas: int
    unexpected: tuple[str, ...]


@dataclass(frozen=True)
class DeltaCompileSequence:
    """What a fixed sequence of edits printed: which compiles were deltas and the last report."""

    delta_lines: tuple[bool, ...]
    replayed_last: bool
    last_report: str
    full_report: str
