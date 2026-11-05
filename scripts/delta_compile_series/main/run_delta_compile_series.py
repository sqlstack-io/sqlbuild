"""Run a seeded series of edits, compiling and judging after each one."""

from __future__ import annotations

import random
import shutil
from collections.abc import Callable
from pathlib import Path
from typing import Any

from scripts.delta_compile_series._helpers.edits import edited_file
from scripts.delta_compile_series._helpers.evidence import cache_state, keep_comparison_failure
from scripts.delta_compile_series._helpers.outcomes import full_compile_difference, judged
from scripts.delta_compile_series.constants import (
    DELTA_COMPILE_LINE,
    DELTA_WEIGHT,
    EDITS,
    FALLBACK_WEIGHT,
    FULL_COPY_SUFFIX,
    REVERT_PROBABILITY,
)
from scripts.delta_compile_series.models import (
    DeltaCompileEdit,
    DeltaCompileOutcome,
    DeltaCompileSeries,
)


def _compared(
    *,
    project_dir: Path,
    compile_project: Callable[[], DeltaCompileOutcome],
    compile_in_full: Callable[[Path], DeltaCompileOutcome],
    diagnostics_dir: Path | None,
    seed: int,
    step: str,
    case: str,
) -> tuple[DeltaCompileOutcome, str | None]:
    """Compile, and compile a copy of the project as it was in full; how what they print differs."""

    copy: Path = project_dir.with_name(project_dir.name + FULL_COPY_SUFFIX)
    shutil.rmtree(copy, ignore_errors=True)
    _ = shutil.copytree(project_dir, copy, symlinks=True)
    before: dict[str, Any] = (
        {"delta": cache_state(project_dir), "full": cache_state(copy)} if diagnostics_dir else {}
    )
    outcome: DeltaCompileOutcome = compile_project()
    after: dict[str, Any] = {"delta": cache_state(project_dir)} if diagnostics_dir else {}
    full: DeltaCompileOutcome = compile_in_full(copy)
    if diagnostics_dir:
        after["full"] = cache_state(copy)
    problem: str | None = full_compile_difference(
        outcome=outcome, full=full, project_dir=project_dir, full_project_dir=copy
    )
    if problem is not None and diagnostics_dir is not None:
        try:
            _ = keep_comparison_failure(
                destination=diagnostics_dir,
                seed=seed,
                step=step,
                case=case,
                project=project_dir,
                copy=copy,
                outcome=outcome,
                full=full,
                before=before,
                after=after,
            )
        except OSError:
            pass
    return outcome, problem


def run_delta_compile_series(
    *,
    project_dir: Path,
    seed: int,
    steps: int,
    compile_project: Callable[[], DeltaCompileOutcome],
    compile_in_full: Callable[[Path], DeltaCompileOutcome] | None = None,
    diagnostics_dir: Path | None = None,
    case: str = "",
) -> DeltaCompileSeries:
    """Edit or revert `steps` times, compiling after each, and judge or compare every compile."""

    rng: random.Random = random.Random(seed)
    weights: list[int] = [
        FALLBACK_WEIGHT if edit.fallback_reason else DELTA_WEIGHT for edit in EDITS
    ]
    pending: list[tuple[Path, str, DeltaCompileEdit]] = []
    described: list[str] = []
    unexpected: list[str] = []
    deltas: int = 0
    failing: bool = compile_project().returncode != 0
    for index in range(steps):
        reverted: bool = bool(pending) and (failing or rng.random() < REVERT_PROBABILITY)
        if reverted:
            path, original, edit = pending.pop()
            path.write_text(original, encoding="utf-8")
        else:
            edit: DeltaCompileEdit = rng.choices(EDITS, weights=weights)[0]
            path, original = edited_file(project_dir=project_dir, edit=edit, rng=rng)
            pending.append((path, original, edit))
        step: str = f"step {index}: {'revert ' if reverted else ''}{edit.kind} in {path.name}"
        described.append(step)
        if compile_in_full is None:
            outcome: DeltaCompileOutcome = compile_project()
            others_edited: bool = any(other != path for other, _, _ in pending)
            delta, problem = judged(
                outcome=outcome,
                edit=edit,
                reverted=reverted,
                others_edited=others_edited,
                after_failure=failing,
            )
        else:
            outcome, problem = _compared(
                project_dir=project_dir,
                compile_project=compile_project,
                compile_in_full=compile_in_full,
                diagnostics_dir=diagnostics_dir,
                seed=seed,
                step=step,
                case=case,
            )
            delta: bool = DELTA_COMPILE_LINE in outcome.stderr
        deltas += int(delta)
        if problem is not None:
            unexpected.append(f"{step}: {problem}")
        failing = outcome.returncode != 0
    return DeltaCompileSeries(steps=tuple(described), deltas=deltas, unexpected=tuple(unexpected))
