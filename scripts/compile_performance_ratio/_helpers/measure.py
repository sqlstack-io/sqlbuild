"""Generate benchmark projects and time alternating base and head compiles."""

from __future__ import annotations

import json
import os
import resource
import shutil
import statistics
import subprocess
import sys
import time
from collections.abc import Callable
from functools import partial
from pathlib import Path

from scripts.cold_compile_performance._helpers.dense_project import write_dense_compile_project
from scripts.compile_performance_ratio._helpers.edit import apply_one_model_edit
from scripts.compile_performance_ratio.constants import (
    ANALYSED_MODEL_COUNTERS,
    ANALYSIS_CACHE_BYPASSES,
    ANALYSIS_CACHE_MISSES,
    BASE_GENERATOR_ENTRY,
    BASE_LABEL,
    COLD_MODE,
    COMPILE_ENTRY,
    COMPILE_TIMINGS_KEY,
    COMPILED_TREES,
    DENSE_KIND,
    EDIT_MODE,
    ERROR_TAIL_CHARACTERS,
    EXCLUDED_ENVIRONMENT_KEYS,
    EXCLUDED_ENVIRONMENT_PREFIX,
    FRESH_AUDIT_SHARE,
    FRESH_FUNCTION_SHARE,
    FRESH_MACRO_SHARE,
    FRESH_SEED_SHARE,
    FRESH_SOURCE_SHARE,
    FRESH_TEST_SHARE,
    HEAD_LABEL,
    MODE_TITLES,
    PYTHONPATH_KEY,
    REPORTED_PHASES,
    TARGET_DIRECTORY,
    UNCACHED_MODE,
    WARM_MODE,
)
from scripts.compile_performance_ratio.exceptions import CompileComparisonError
from scripts.compile_performance_ratio.models import CompileComparison, CompileRun
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    write_semantic_compile_project,
)


def write_benchmark_project(*, kind: str, project_dir: Path, models: int) -> None:
    """Write the dense or fresh benchmark project used by both builds."""

    if kind == DENSE_KIND:
        write_dense_compile_project(project_dir=project_dir, model_count=models)
        return
    write_semantic_compile_project(
        project_dir=project_dir,
        model_count=models,
        source_count=max(1, round(models * FRESH_SOURCE_SHARE)),
        seed_count=max(1, round(models * FRESH_SEED_SHARE)),
        function_count=max(1, round(models * FRESH_FUNCTION_SHARE)),
        macro_count=max(1, round(models * FRESH_MACRO_SHARE)),
        test_count=max(1, round(models * FRESH_TEST_SHARE)),
        audit_count=max(1, round(models * FRESH_AUDIT_SHARE)),
    )


def run_checkout_generator(
    *,
    checkout: Path,
    python: Path,
    entry: str,
    arguments: tuple[str, ...],
    environment: dict[str, str],
    library_paths: tuple[Path, ...],
) -> subprocess.CompletedProcess[str]:
    """Run another checkout's generator with `python`, importing `library_paths` before its own."""

    return subprocess.run(
        [str(python), "-c", entry, *arguments],
        cwd=checkout,
        capture_output=True,
        text=True,
        env={
            **environment,
            PYTHONPATH_KEY: os.pathsep.join(str(path) for path in (checkout, *library_paths)),
        },
        check=False,
    )


def write_base_benchmark_project(
    *, base_root: Path, python: Path, kind: str, project_dir: Path, models: int
) -> None:
    """Write the base project with the base checkout's own generator, run by `python`."""

    completed: subprocess.CompletedProcess[str] = run_checkout_generator(
        checkout=base_root,
        python=python,
        entry=BASE_GENERATOR_ENTRY,
        arguments=(kind, str(project_dir), str(models)),
        environment=_compile_environment(),
        library_paths=(),
    )
    if completed.returncode != 0:
        raise CompileComparisonError(
            f"base project generation in {base_root} failed with exit {completed.returncode}: "
            f"{completed.stderr[-ERROR_TAIL_CHARACTERS:]}"
        )


def compare_builds(
    *,
    kind: str,
    models: int,
    base_project_dir: Path,
    head_project_dir: Path,
    base_python: Path,
    head_python: Path,
    runs: int,
    modes: tuple[str, ...],
    compile_args: tuple[str, ...] = (),
) -> tuple[CompileComparison, ...]:
    """Alternate base and head compiles per mode; warm and edit reuse the cold project."""

    builds: tuple[tuple[str, Path, Path], ...] = (
        (BASE_LABEL, base_python, base_project_dir),
        (HEAD_LABEL, head_python, head_project_dir),
    )
    comparisons: list[CompileComparison] = []
    cache_primed: bool = False
    for mode in modes:
        print(f"Comparing {MODE_TITLES[mode]} over {runs} alternating runs", file=sys.stderr)
        if mode == COLD_MODE:
            for label, python, project_dir in builds:
                _ = _compile_once(
                    label=label,
                    python=python,
                    project_dir=project_dir,
                    mode=mode,
                    compile_args=compile_args,
                )
            cache_primed = False
        elif not cache_primed:
            for label, python, project_dir in builds:
                _ = _compile_once(
                    label=label,
                    python=python,
                    project_dir=project_dir,
                    mode=WARM_MODE,
                    compile_args=compile_args,
                )
            cache_primed = True
        results: dict[str, list[CompileRun]] = {BASE_LABEL: [], HEAD_LABEL: []}
        for revision in range(runs):
            for label, python, project_dir in builds:
                if mode == EDIT_MODE:
                    _ = apply_one_model_edit(project_dir=project_dir, revision=revision)
                run: CompileRun = _compile_once(
                    label=label,
                    python=python,
                    project_dir=project_dir,
                    mode=mode,
                    compile_args=compile_args,
                )
                check_cache_use(run=run, mode=mode, models=models)
                results[label].append(run)
        if mode != COLD_MODE:
            _, head_python, head_dir = builds[1]
            check_matches_uncached(
                incremental=results[HEAD_LABEL][-1],
                compiled=compiled_tree(project_dir=head_dir),
                uncached=partial(
                    _compile_once,
                    label=HEAD_LABEL,
                    python=head_python,
                    project_dir=head_dir,
                    mode=UNCACHED_MODE,
                    compile_args=compile_args,
                ),
                project_dir=head_dir,
                mode=mode,
            )
            cache_primed = False
        comparisons.append(
            _comparison(
                kind=kind,
                models=models,
                mode=mode,
                base=results[BASE_LABEL],
                head=results[HEAD_LABEL],
            )
        )
    return tuple(comparisons)


def _comparison(
    *, kind: str, models: int, mode: str, base: list[CompileRun], head: list[CompileRun]
) -> CompileComparison:
    return CompileComparison(
        kind=kind,
        models=models,
        mode=mode,
        base_wall_seconds=statistics.median(run.wall_seconds for run in base),
        head_wall_seconds=statistics.median(run.wall_seconds for run in head),
        base_cpu_seconds=statistics.median(run.cpu_seconds for run in base),
        head_cpu_seconds=statistics.median(run.cpu_seconds for run in head),
        base_timings_ms=median_phases(runs=base),
        head_timings_ms=median_phases(runs=head),
    )


def check_cache_use(*, run: CompileRun, mode: str, models: int) -> None:
    """Fail a bypassing full compile, a warm run with misses, or an edit that analysed nothing."""

    misses: int | None = run.analysis_cache_misses
    if misses is None or mode == COLD_MODE:
        return
    bypasses: int = run.analysis_cache_bypasses or 0
    delta: bool = run.analysed_models is not None and run.analysed_models < models
    if mode == EDIT_MODE and delta:
        if misses + bypasses == 0:
            raise CompileComparisonError(
                f"{run.label} one-model edit delta compile analysed no model afresh, so the "
                "edit did not invalidate the edited model"
            )
        return
    if bypasses != 0:
        raise CompileComparisonError(
            f"{run.label} {mode} compile bypassed the analysis cache for {bypasses} models, so "
            "it did not measure a cached compile"
        )
    if mode == WARM_MODE and misses != 0:
        raise CompileComparisonError(
            f"{run.label} warm compile missed the analysis cache for {misses} models, so it "
            "did not measure an unchanged warm compile"
        )
    if mode == EDIT_MODE and misses == 0:
        raise CompileComparisonError(
            f"{run.label} one-model edit compile reported no analysis cache miss, so the edit "
            "did not invalidate the edited model"
        )


def compiled_tree(*, project_dir: Path) -> dict[str, bytes]:
    """Every compiled and failed-compile artifact's bytes by its path under `target/`."""

    target: Path = project_dir / TARGET_DIRECTORY
    artifacts: dict[str, bytes] = {}
    for tree in COMPILED_TREES:
        artifacts.update(
            {
                path.relative_to(target).as_posix(): path.read_bytes()
                for path in sorted((target / tree).rglob("*"))
                if path.is_file()
            }
        )
    return artifacts


def check_matches_uncached(
    *,
    incremental: CompileRun,
    compiled: dict[str, bytes],
    uncached: Callable[[], CompileRun],
    project_dir: Path,
    mode: str,
) -> None:
    """Fail when an uncached compile of the same inputs reports or writes anything else."""

    reference: CompileRun = uncached()
    reference_compiled: dict[str, bytes] = compiled_tree(project_dir=project_dir)
    differing: list[str] = sorted(
        path
        for path in compiled.keys() | reference_compiled.keys()
        if compiled.get(path) != reference_compiled.get(path)
    )
    if incremental.report != reference.report or differing:
        raise CompileComparisonError(
            f"{incremental.label} {mode} compile differs from an uncached compile of the same "
            f"project: report {'differs' if incremental.report != reference.report else 'equal'}, "
            f"compiled artifacts differing {differing[:5]}"
        )


def _compile_once(
    *,
    label: str,
    python: Path,
    project_dir: Path,
    mode: str,
    compile_args: tuple[str, ...],
) -> CompileRun:
    cache_args: tuple[str, ...] = ()
    if mode == COLD_MODE:
        shutil.rmtree(project_dir / "target", ignore_errors=True)
    if mode in {COLD_MODE, UNCACHED_MODE}:
        cache_args = ("--no-cache",)
    environment: dict[str, str] = _compile_environment()
    before: resource.struct_rusage = resource.getrusage(resource.RUSAGE_CHILDREN)
    started: float = time.perf_counter()
    completed: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(python),
            "-c",
            COMPILE_ENTRY,
            "--project-dir",
            str(project_dir),
            "--no-color",
            "compile",
            "--json",
            *cache_args,
            *compile_args,
        ],
        capture_output=True,
        text=True,
        env=environment,
        check=False,
    )
    wall_seconds: float = time.perf_counter() - started
    after: resource.struct_rusage = resource.getrusage(resource.RUSAGE_CHILDREN)
    if completed.returncode != 0:
        raise CompileComparisonError(
            f"{label} {mode} compile failed with exit {completed.returncode}: "
            f"{completed.stderr[-ERROR_TAIL_CHARACTERS:]}"
        )
    payload: dict[str, object] = json.loads(completed.stdout)
    timings: object = payload.pop(COMPILE_TIMINGS_KEY, {})
    numeric: dict[str, int] = {
        str(name): int(value)
        for name, value in (timings.items() if isinstance(timings, dict) else ())
        if isinstance(value, (int, float))
    }
    return CompileRun(
        label=label,
        wall_seconds=wall_seconds,
        cpu_seconds=(after.ru_utime - before.ru_utime) + (after.ru_stime - before.ru_stime),
        timings_ms={name: value for name, value in numeric.items() if name in REPORTED_PHASES},
        analysis_cache_misses=numeric.get(ANALYSIS_CACHE_MISSES),
        analysis_cache_bypasses=numeric.get(ANALYSIS_CACHE_BYPASSES),
        analysed_models=(
            sum(numeric[name] for name in ANALYSED_MODEL_COUNTERS)
            if all(name in numeric for name in ANALYSED_MODEL_COUNTERS)
            else None
        ),
        report=json.dumps(payload, sort_keys=True),
    )


def _compile_environment() -> dict[str, str]:
    return {
        key: value
        for key, value in os.environ.items()
        if key not in EXCLUDED_ENVIRONMENT_KEYS and not key.startswith(EXCLUDED_ENVIRONMENT_PREFIX)
    }


def median_phases(*, runs: list[CompileRun]) -> dict[str, float]:
    """Return each phase's median over runs, omitting phases any run did not report."""

    medians: dict[str, float] = {}
    for phase in REPORTED_PHASES:
        values: list[int] = [run.timings_ms[phase] for run in runs if phase in run.timings_ms]
        if values and len(values) == len(runs):
            medians[phase] = statistics.median(values)
    return medians
