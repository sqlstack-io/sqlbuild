"""Install both versions, time the benchmark commands and report the release verdict."""

from __future__ import annotations

import dataclasses
import json
import os
import sys
import tempfile
import time
from pathlib import Path

from scripts.compile_performance_ratio._helpers.report import cpu_model
from scripts.compile_performance_ratio.exceptions import BenchmarkEditError
from scripts.release_performance._helpers.benchmark import (
    compare_command,
    prepare_version_projects,
    write_baseline_dense_project,
    write_baseline_pristine_projects,
    write_dense_project,
    write_pristine_projects,
)
from scripts.release_performance._helpers.report import (
    append_summary,
    comparison_markdown,
    error_markdown,
)
from scripts.release_performance._helpers.verdict import (
    all_verdicts,
    regression_messages,
    skip_reason,
)
from scripts.release_performance._helpers.versions import (
    existing_sqb,
    install_published,
    install_wheel,
    installed_version,
    release_source,
    resolve_baseline,
)
from scripts.release_performance.constants import (
    BASELINE_GENERATED_DIRECTORY,
    BASELINE_LABEL,
    BASELINE_PUBLICATION_WAIT_SECONDS,
    BASELINE_SOURCE_DIRECTORY,
    BENCHMARK_COMMANDS,
    CANDIDATE_LABEL,
    PROGRESS_HEADING,
    RELEASE_TAG_PREFIX,
    REPO_ROOT,
)
from scripts.release_performance.exceptions import ReleasePerformanceError
from scripts.release_performance.models import (
    CommandComparison,
    ComparisonOptions,
    InstalledVersion,
    ReleaseComparison,
    RunnerContext,
    SkippedCommand,
)


def run_release_comparison(*, options: ComparisonOptions) -> int:
    """Compare the candidate with the baseline release and enforce the release limits."""

    summary_value: str | None = os.environ.get("GITHUB_STEP_SUMMARY")
    summary: Path | None = Path(summary_value) if summary_value else None
    try:
        if options.work_dir is not None:
            options.work_dir.mkdir(parents=True, exist_ok=True)
            comparison: ReleaseComparison = _compare(
                options=options, root=options.work_dir, summary=summary
            )
        else:
            with tempfile.TemporaryDirectory(prefix="sqlbuild-release-performance-") as root:
                comparison = _compare(options=options, root=Path(root), summary=summary)
    except (ReleasePerformanceError, BenchmarkEditError, OSError) as error:
        print(f"Release performance comparison failed: {error}", file=sys.stderr)
        _ = append_summary(path=summary, markdown=error_markdown(message=str(error)))
        return 1
    markdown: str = comparison_markdown(comparison=comparison)
    print(markdown)
    _ = append_summary(path=summary, markdown=markdown)
    if options.output is not None:
        _write_evidence(path=options.output, comparison=comparison)
    failures: tuple[str, ...] = regression_messages(commands=comparison.commands)
    if failures:
        print("Release performance regression: " + "; ".join(failures), file=sys.stderr)
        return 1
    print("Release performance within the limits of the previous release", file=sys.stderr)
    return 0


def _compare(*, options: ComparisonOptions, root: Path, summary: Path | None) -> ReleaseComparison:
    _ = append_summary(path=summary, markdown=PROGRESS_HEADING)
    _progress(summary=summary, message=f"Installing candidate {options.candidate}")
    candidate: InstalledVersion = _install(
        label=CANDIDATE_LABEL,
        spec=options.candidate,
        root=root,
        python=options.python,
        wait_seconds=0.0,
    )
    baseline_spec: str = options.baseline or resolve_baseline(
        candidate=candidate.version, same_version=options.same_version_baseline
    )
    _progress(summary=summary, message=f"Installing baseline {baseline_spec}")
    baseline: InstalledVersion = _install(
        label=BASELINE_LABEL,
        spec=baseline_spec,
        root=root,
        python=options.python,
        wait_seconds=BASELINE_PUBLICATION_WAIT_SECONDS,
    )
    _progress(
        summary=summary,
        message=f"Comparing candidate {candidate.version} with baseline {baseline.version}",
    )
    load_before: tuple[float, float, float] = os.getloadavg()
    _progress(summary=summary, message="Generating candidate benchmark projects")
    pristine: dict[str, Path] = {
        CANDIDATE_LABEL: write_pristine_projects(
            root=root,
            inspection_models=options.inspection_models,
            build_models=options.build_models,
        )
    }
    baseline_generator: str = (
        str(options.baseline_source)
        if options.baseline_source is not None
        else RELEASE_TAG_PREFIX + baseline.version
    )
    write_dense_project(pristine=pristine[CANDIDATE_LABEL], models=options.dense_models)
    _progress(
        summary=summary, message=f"Generating baseline benchmark projects with {baseline_generator}"
    )
    baseline_source: Path = (
        options.baseline_source.absolute()
        if options.baseline_source is not None
        else release_source(
            version=baseline.version,
            repo_dir=REPO_ROOT,
            destination=root / BASELINE_SOURCE_DIRECTORY,
        )
    )
    pristine[BASELINE_LABEL] = write_baseline_pristine_projects(
        source=baseline_source,
        python=baseline.python,
        root=root / BASELINE_GENERATED_DIRECTORY,
        inspection_models=options.inspection_models,
        build_models=options.build_models,
    )
    write_baseline_dense_project(
        source=baseline_source,
        python=baseline.python,
        pristine=pristine[BASELINE_LABEL],
        models=options.dense_models,
    )
    versions: tuple[InstalledVersion, InstalledVersion] = (baseline, candidate)
    projects: dict[str, dict[str, Path]] = {}
    for version in versions:
        _progress(summary=summary, message=f"Warming caches for {version.label} {version.version}")
        projects[version.label] = prepare_version_projects(
            version=version, pristine=pristine[version.label], root=root
        )
    measured: list[CommandComparison] = []
    skipped: list[SkippedCommand] = []
    for command in BENCHMARK_COMMANDS:
        reason: str | None = skip_reason(
            command=command, baseline_version=baseline.version, candidate_version=candidate.version
        )
        if reason is not None:
            print(f"Skipping {command.name}: {reason}", file=sys.stderr)
            skipped.append(SkippedCommand(name=command.name, reason=reason))
            continue
        _progress(summary=summary, message=f"Timing `{command.name}`")
        measured.append(
            compare_command(
                command=command, versions=versions, projects=projects, root=root, runs=options.runs
            )
        )
    return ReleaseComparison(
        baseline_version=baseline.version,
        candidate_version=candidate.version,
        baseline_generator=baseline_generator,
        runs=options.runs,
        runner=RunnerContext(
            cpu_model=cpu_model(),
            cpu_count=os.cpu_count() or 0,
            load_average_before=load_before,
            load_average_after=os.getloadavg(),
        ),
        commands=tuple(measured),
        skipped=tuple(skipped),
    )


def _progress(*, summary: Path | None, message: str) -> None:
    print(message, file=sys.stderr, flush=True)
    _ = append_summary(
        path=summary, markdown=f"- {time.strftime('%H:%M:%S', time.gmtime())} UTC: {message}"
    )


def _install(
    *, label: str, spec: str, root: Path, python: str, wait_seconds: float
) -> InstalledVersion:
    path: Path = Path(spec)
    venv_dir: Path = root / "venvs" / label
    if path.is_dir():
        sqb: Path = existing_sqb(venv_dir=path)
    elif spec.endswith(".whl"):
        sqb = install_wheel(wheel=path, venv_dir=venv_dir, python=python)
    else:
        sqb = install_published(
            version=spec, venv_dir=venv_dir, python=python, wait_seconds=wait_seconds
        )
    return InstalledVersion(
        label=label, version=installed_version(sqb=sqb), sqb=sqb, python=sqb.with_name("python")
    )


def _write_evidence(*, path: Path, comparison: ReleaseComparison) -> None:
    evidence: dict[str, object] = {
        **dataclasses.asdict(comparison),
        "verdicts": [
            dataclasses.asdict(verdict) for verdict in all_verdicts(commands=comparison.commands)
        ],
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    _ = path.write_text(json.dumps(evidence, indent=2, default=str) + "\n", encoding="utf-8")
