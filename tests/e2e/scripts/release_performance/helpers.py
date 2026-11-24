"""Fixtures for release performance comparisons with per-side benchmark generators."""

from __future__ import annotations

import shutil
from pathlib import Path

BASELINE_MARKER: str = ".BASELINE_MARKER"
_MARKING_WRAPPER: str = """

_unmarked_write_pristine_projects = write_pristine_projects


def write_pristine_projects(*, root: Path, inspection_models: int, build_models: int) -> Path:
    pristine: Path = _unmarked_write_pristine_projects(
        root=root, inspection_models=inspection_models, build_models=build_models
    )
    for project in pristine.iterdir():
        (project / ".BASELINE_MARKER").write_text("baseline", encoding="utf-8")
    return pristine
"""
_DENSE_MARKING_WRAPPER: str = """

_unmarked_write_dense_compile_project = write_dense_compile_project


def write_dense_compile_project(*, project_dir: Path, model_count: int) -> None:
    _unmarked_write_dense_compile_project(project_dir=project_dir, model_count=model_count)
    (project_dir / ".BASELINE_MARKER").write_text("baseline", encoding="utf-8")
"""


def write_marked_baseline_source(*, repo_root: Path, destination: Path) -> Path:
    """Copy this checkout's generators and mark every project the copy generates."""

    for name in ("scripts", "tests"):
        _ = shutil.copytree(
            repo_root / name, destination / name, ignore=shutil.ignore_patterns("__pycache__")
        )
    benchmark: Path = destination / "scripts" / "release_performance" / "_helpers" / "benchmark.py"
    _ = benchmark.write_text(
        benchmark.read_text(encoding="utf-8") + _MARKING_WRAPPER, encoding="utf-8"
    )
    dense: Path = (
        destination / "scripts" / "cold_compile_performance" / "_helpers" / "dense_project.py"
    )
    _ = dense.write_text(
        dense.read_text(encoding="utf-8") + _DENSE_MARKING_WRAPPER, encoding="utf-8"
    )
    return destination
