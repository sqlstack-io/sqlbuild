"""Judge what one compile of a delta compile series did."""

from __future__ import annotations

import json
import re
from itertools import zip_longest
from pathlib import Path
from typing import Any

from scripts.delta_compile_series.constants import (
    AFTER_FAILED_RULES,
    COMPILE_REUSE_HIT_LINE,
    COMPILE_TIMINGS_PATTERN,
    DELTA_COMPILE_LINE,
    DELTA_FALLBACK_PATTERN,
    DELTA_MISMATCH_PATTERN,
    DELTA_VERIFIED_LINE,
    DURATION_PATTERN,
    LOG_DIAGNOSTICS_FILE,
    LOG_RUNS_GLOB,
    MAX_CACHE_LOG_LINES,
    MISMATCH_EXIT_CODE,
    SQL_TEST_CACHE_LOG_PREFIX,
    STALE_PROBLEMS,
)
from scripts.delta_compile_series.models import DeltaCompileEdit, DeltaCompileOutcome

_STREAMS: tuple[str, ...] = ("exit code", "stdout", "stderr")


def fallback_reason(*, outcome: DeltaCompileOutcome) -> str | None:
    """Why the compile fell back from a delta, when it says."""

    reasons: list[str] = re.findall(DELTA_FALLBACK_PATTERN, outcome.stderr, flags=re.MULTILINE)
    return reasons[0] if reasons else None


def judged(
    *,
    outcome: DeltaCompileOutcome,
    edit: DeltaCompileEdit,
    reverted: bool,
    others_edited: bool,
    after_failure: bool = False,
) -> tuple[bool, str | None]:
    """Whether the compile was a verified delta, and what was unexpected about it, if anything."""

    mismatches: list[str] = re.findall(DELTA_MISMATCH_PATTERN, outcome.stderr, flags=re.MULTILINE)
    if outcome.returncode == MISMATCH_EXIT_CODE or mismatches:
        return False, f"mismatch: {mismatches}"
    if COMPILE_REUSE_HIT_LINE in outcome.stderr:
        return False, None if reverted else "replayed a compile after an edit"
    reason: str | None = fallback_reason(outcome=outcome)
    if others_edited and reason is not None and STALE_PROBLEMS in reason:
        return False, None
    if after_failure and reason == AFTER_FAILED_RULES:
        return False, None
    if edit.fallback_reason is None:
        verified: bool = DELTA_VERIFIED_LINE in outcome.stderr
        return verified, None if verified else f"fell back: {reason}"
    if reverted and after_failure and reason is not None:
        return False, None
    if reason is None or edit.fallback_reason not in reason:
        return False, f"expected a fallback for {edit.fallback_reason!r}, got {reason!r}"
    return False, None


def _seen(*, outcome: DeltaCompileOutcome, project_dir: Path, other_dir: Path) -> tuple[str, ...]:
    """What a user sees of a compile, without durations, timings and the delta's own line."""

    def shown(text: str) -> str:
        text = COMPILE_TIMINGS_PATTERN.sub("", text.replace(str(other_dir), str(project_dir)))
        kept: list[str] = [line for line in text.splitlines() if DELTA_COMPILE_LINE not in line]
        return DURATION_PATTERN.sub("", "\n".join(kept))

    return (str(outcome.returncode), shown(outcome.stdout), shown(outcome.stderr))


def full_compile_difference(
    *,
    outcome: DeltaCompileOutcome,
    full: DeltaCompileOutcome,
    project_dir: Path,
    full_project_dir: Path,
) -> str | None:
    """The first line a user sees differently than after a full compile of a copy, if any."""

    if COMPILE_REUSE_HIT_LINE in outcome.stderr:
        return None

    seen: tuple[str, ...] = _seen(outcome=outcome, project_dir=project_dir, other_dir=project_dir)
    expected: tuple[str, ...] = _seen(
        outcome=full, project_dir=project_dir, other_dir=full_project_dir
    )
    for stream, left, right in zip(_STREAMS, seen, expected, strict=True):
        if left != right:
            first: tuple[str, str] = next(
                pair
                for pair in zip_longest(left.splitlines(), right.splitlines(), fillvalue="")
                if pair[0] != pair[1]
            )
            return f"{stream} differs from a full compile: {first[0]!r} vs {first[1]!r}"
    return None


def log_runs(project: Path) -> frozenset[Path]:
    """The per-invocation log directories a project holds."""

    return frozenset(path for path in project.glob(LOG_RUNS_GLOB) if path.is_dir())


def sql_test_cache_lines(*, project: Path, earlier: frozenset[Path]) -> str:
    """Cache-miss reasons and record keys logged since `earlier`, never SQL text or environment."""

    lines: list[str] = []
    for run in sorted(log_runs(project) - earlier):
        try:
            text: str = (run / LOG_DIAGNOSTICS_FILE).read_text(encoding="utf-8", errors="replace")
            rows: list[str] = text.splitlines()
        except OSError:
            continue
        for row in rows:
            try:
                record: Any = json.loads(row)
            except ValueError:
                continue
            message: Any = record.get("message") if isinstance(record, dict) else None
            if isinstance(message, str) and message.startswith(SQL_TEST_CACHE_LOG_PREFIX):
                lines.append(message)
    shown: list[str] = lines[:MAX_CACHE_LOG_LINES]
    return " | ".join(shown) if shown else "none logged"
