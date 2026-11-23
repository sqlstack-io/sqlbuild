"""Builders for same-runner compile comparison test data."""

from functools import partial
from pathlib import Path

from scripts.compile_performance_ratio._helpers.measure import (
    check_matches_uncached,
    compiled_tree,
)
from scripts.compile_performance_ratio.constants import EDIT_MODE
from scripts.compile_performance_ratio.models import CompileComparison, CompileRun


def comparison(
    *, mode: str, wall: tuple[float, float], cpu: tuple[float, float]
) -> CompileComparison:
    """Build a dense 3000-model comparison from (base, head) wall and CPU medians."""

    return CompileComparison(
        kind="dense",
        models=3000,
        mode=mode,
        base_wall_seconds=wall[0],
        head_wall_seconds=wall[1],
        base_cpu_seconds=cpu[0],
        head_cpu_seconds=cpu[1],
        base_timings_ms={"total_ms": wall[0] * 1000},
        head_timings_ms={"total_ms": wall[1] * 1000},
    )


def write_model_files(*, project_dir: Path, model_files: dict[str, str]) -> None:
    """Write model files at project-relative paths."""

    for relative, contents in model_files.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")


def read_model_files(*, project_dir: Path) -> dict[str, str]:
    """Read every model file keyed by its project-relative path."""

    return {
        path.relative_to(project_dir).as_posix(): path.read_text(encoding="utf-8")
        for path in sorted((project_dir / "sql" / "models").rglob("*.sql"))
    }


def report_run(report: str) -> CompileRun:
    """A head compile run that reported `report`."""

    return CompileRun(label="head", wall_seconds=1.0, cpu_seconds=1.0, timings_ms={}, report=report)


def check_against_uncached(
    *,
    project_dir: Path,
    reports: tuple[str, str],
    files: tuple[dict[str, str], dict[str, str]],
) -> None:
    """Check an incremental compile's report and files against an uncached compile's."""

    write_model_files(project_dir=project_dir, model_files=files[0])
    check_matches_uncached(
        incremental=report_run(reports[0]),
        compiled=compiled_tree(project_dir=project_dir),
        uncached=partial(
            _uncached_run, project_dir=project_dir, model_files=files[1], report=reports[1]
        ),
        project_dir=project_dir,
        mode=EDIT_MODE,
    )


def _uncached_run(*, project_dir: Path, model_files: dict[str, str], report: str) -> CompileRun:
    write_model_files(project_dir=project_dir, model_files=model_files)
    return report_run(report)
