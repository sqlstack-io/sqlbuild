"""Prepare identical benchmark projects per version and time interleaved command runs."""

from __future__ import annotations

import json
import os
import shutil
import signal
import subprocess
import sys
from pathlib import Path
from typing import cast

from scripts.cold_compile_performance._helpers.dense_project import write_dense_compile_project
from scripts.compile_performance_ratio._helpers.edit import apply_one_model_edit
from scripts.compile_performance_ratio._helpers.measure import run_checkout_generator
from scripts.release_performance._helpers.versions import library_paths, setup_timeout_error
from scripts.release_performance.constants import (
    BASELINE_GENERATOR_ENTRY,
    BUILD_PROJECT,
    BUILD_WARMUPS,
    COMMAND_TIMEOUT_SECONDS,
    COMPILE_COMMAND,
    DENSE_GENERATOR_ENTRY,
    DENSE_PROJECT,
    DENSE_WARMUPS,
    EXCLUDED_ENVIRONMENT_KEYS,
    EXCLUDED_ENVIRONMENT_PREFIX,
    INSPECTION_PROJECT,
    INSPECTION_WARMUPS,
    PRISTINE_DIRECTORY,
    SETUP_COMMAND_TIMEOUT_SECONDS,
    STDERR_TAIL_CHARACTERS,
    TARGET_DIRECTORY,
    TIME_FORMAT,
)
from scripts.release_performance.exceptions import ReleasePerformanceError
from scripts.release_performance.models import (
    BenchmarkCommand,
    CommandComparison,
    CommandSample,
    InstalledVersion,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    write_build_benchmark_project,
    write_inspection_benchmark_project,
)

_MIB: int = 1024 * 1024
_TEMPLATE_SUFFIX: str = "-template"


def write_pristine_projects(*, root: Path, inspection_models: int, build_models: int) -> Path:
    """Generate the inspection and build benchmarks once so every version reads the same files."""

    pristine: Path = root / PRISTINE_DIRECTORY
    write_inspection_benchmark_project(
        project_dir=pristine / INSPECTION_PROJECT, model_count=inspection_models
    )
    write_build_benchmark_project(project_dir=pristine / BUILD_PROJECT, model_count=build_models)
    return pristine


def write_baseline_pristine_projects(
    *, source: Path, python: Path, root: Path, inspection_models: int, build_models: int
) -> Path:
    """Generate the baseline's benchmarks with its own generator and installed `sqlbuild`."""

    _run_baseline_generator(
        source=source,
        python=python,
        entry=BASELINE_GENERATOR_ENTRY,
        arguments=(str(root), str(inspection_models), str(build_models)),
        description="benchmark",
    )
    return root / PRISTINE_DIRECTORY


def write_dense_project(*, pristine: Path, models: int) -> None:
    """Generate the dense all-rules benchmark beside the other pristine projects."""

    write_dense_compile_project(project_dir=pristine / DENSE_PROJECT, model_count=models)


def write_baseline_dense_project(
    *, source: Path, python: Path, pristine: Path, models: int
) -> None:
    """Generate the baseline's dense benchmark with its own generator and installed `sqlbuild`."""

    _run_baseline_generator(
        source=source,
        python=python,
        entry=DENSE_GENERATOR_ENTRY,
        arguments=(str(pristine / DENSE_PROJECT), str(models)),
        description="dense benchmark",
    )


def prepare_version_projects(
    *, version: InstalledVersion, pristine: Path, root: Path
) -> dict[str, Path]:
    """Copy the pristine projects for one version and warm its caches with untimed runs."""

    project_warmups: tuple[tuple[str, tuple[tuple[str, ...], ...]], ...] = (
        (INSPECTION_PROJECT, INSPECTION_WARMUPS),
        (DENSE_PROJECT, DENSE_WARMUPS),
        (BUILD_PROJECT, BUILD_WARMUPS),
    )
    projects: dict[str, Path] = {
        name: root / version.label / name for name, _warmups in project_warmups
    }
    logs: Path = root / version.label / "logs"
    logs.mkdir(parents=True, exist_ok=True)
    for name, warmups in project_warmups:
        _ = shutil.copytree(pristine / name, projects[name])
        for index, sqb_args in enumerate(warmups):
            output: Path = _run_sqb(
                version=version,
                project_dir=projects[name],
                sqb_args=sqb_args,
                log_prefix=logs / f"warmup-{name}-{index}",
            )[1]
            if sqb_args[0] == COMPILE_COMMAND:
                _assert_clean_compile(version=version, output=output)
    _ = shutil.copytree(
        projects[BUILD_PROJECT], projects[BUILD_PROJECT].with_name(BUILD_PROJECT + _TEMPLATE_SUFFIX)
    )
    return projects


def compare_command(
    *,
    command: BenchmarkCommand,
    versions: tuple[InstalledVersion, InstalledVersion],
    projects: dict[str, dict[str, Path]],
    root: Path,
    runs: int,
) -> CommandComparison:
    """Time baseline and candidate alternately runs times each and keep every sample."""

    samples: dict[str, list[CommandSample]] = {version.label: [] for version in versions}
    for run in range(runs):
        for version in versions:
            project_dir: Path = projects[version.label][command.project]
            if command.project == BUILD_PROJECT:
                _reset_build_project(project_dir=project_dir)
            if command.removes_target:
                shutil.rmtree(project_dir / TARGET_DIRECTORY, ignore_errors=True)
            if command.edits_model:
                _ = apply_one_model_edit(project_dir=project_dir, revision=run)
            sample: CommandSample = _run_sqb(
                version=version,
                project_dir=project_dir,
                sqb_args=command.sqb_args,
                log_prefix=root / version.label / "logs" / "measured",
            )[0]
            samples[version.label].append(sample)
            print(
                f"{command.name} run {run + 1}/{runs} {version.label} {version.version}: "
                f"wall {sample.wall_seconds:.2f}s, CPU {sample.cpu_seconds:.2f}s, "
                f"peak RSS {sample.peak_rss_bytes / _MIB:.0f} MiB",
                file=sys.stderr,
                flush=True,
            )
    baseline, candidate = versions
    return CommandComparison(
        name=command.name,
        baseline=tuple(samples[baseline.label]),
        candidate=tuple(samples[candidate.label]),
        max_time_ratio=command.max_time_ratio,
        max_rss_ratio=command.max_rss_ratio,
    )


def _run_baseline_generator(
    *, source: Path, python: Path, entry: str, arguments: tuple[str, ...], description: str
) -> None:
    paths: tuple[Path, ...] = library_paths(python=python)
    try:
        completed: subprocess.CompletedProcess[str] = run_checkout_generator(
            checkout=source,
            python=Path(sys.executable).absolute(),
            entry=entry,
            arguments=arguments,
            environment=_environment(),
            library_paths=paths,
            timeout_seconds=SETUP_COMMAND_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        raise setup_timeout_error(phase=f"Baseline {description} generation") from None
    if completed.returncode != 0:
        raise ReleasePerformanceError(
            f"Baseline {description} generation with the generator in {source} failed with exit "
            f"{completed.returncode}:\n{completed.stderr[-STDERR_TAIL_CHARACTERS:]}"
        )


def _reset_build_project(*, project_dir: Path) -> None:
    shutil.rmtree(project_dir, ignore_errors=True)
    _ = shutil.copytree(project_dir.with_name(BUILD_PROJECT + _TEMPLATE_SUFFIX), project_dir)


def _assert_clean_compile(*, version: InstalledVersion, output: Path) -> None:
    payload: dict[str, object] = cast(dict[str, object], json.loads(output.read_bytes()))
    diagnostics: list[object] = cast(list[object], payload.get("diagnostics", []))
    if diagnostics:
        raise ReleasePerformanceError(
            f"The benchmark is not valid for {version.label} {version.version}: compile reported "
            f"{len(diagnostics)} diagnostics, first {json.dumps(diagnostics[0])[:500]}"
        )


def _run_sqb(
    *,
    version: InstalledVersion,
    project_dir: Path,
    sqb_args: tuple[str, ...],
    log_prefix: Path,
) -> tuple[CommandSample, Path]:
    measurement: Path = log_prefix.with_suffix(".time")
    output: Path = log_prefix.with_suffix(".out")
    errors: Path = log_prefix.with_suffix(".err")
    command: list[str] = [
        "/usr/bin/time",
        "--quiet",
        "--output",
        str(measurement),
        "--format",
        TIME_FORMAT,
        str(version.sqb),
        "--project-dir",
        str(project_dir),
        "--no-color",
        *sqb_args,
    ]
    with output.open("wb") as stdout, errors.open("wb") as stderr:
        with subprocess.Popen(
            command, stdout=stdout, stderr=stderr, env=_environment(), start_new_session=True
        ) as process:
            try:
                returncode: int = process.wait(timeout=COMMAND_TIMEOUT_SECONDS)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                _ = process.wait()
                raise ReleasePerformanceError(
                    f"{version.label} {version.version} `sqb {' '.join(sqb_args)}` exceeded "
                    f"{COMMAND_TIMEOUT_SECONDS:.0f}s"
                ) from None
    if returncode != 0:
        tail: str = errors.read_text(encoding="utf-8", errors="replace")[-STDERR_TAIL_CHARACTERS:]
        raise ReleasePerformanceError(
            f"{version.label} {version.version} `sqb {' '.join(sqb_args)}` exited with "
            f"{returncode}. If the command needs a feature newer than this version, declare "
            f"minimum_version for it in scripts/release_performance/constants.py.\n{tail}"
        )
    wall, rss_kib, user, system = measurement.read_text(encoding="utf-8").split()[-4:]
    return (
        CommandSample(
            wall_seconds=float(wall),
            cpu_seconds=float(user) + float(system),
            peak_rss_bytes=int(rss_kib) * 1024,
        ),
        output,
    )


def _environment() -> dict[str, str]:
    return {
        key: value
        for key, value in os.environ.items()
        if key not in EXCLUDED_ENVIRONMENT_KEYS and not key.startswith(EXCLUDED_ENVIRONMENT_PREFIX)
    }
