"""Fixtures for per-side compile performance ratio comparisons."""

from __future__ import annotations

import stat
import sys
from pathlib import Path

MARKER_GENERATOR: str = """from pathlib import Path


def write_benchmark_project(*, kind: str, project_dir: Path, models: int) -> None:
    del models
    (project_dir / "sql" / "models").mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\\nadapter = "duckdb"\\nextra_paths = ["base_marker.txt"]\\n'
    )
    (project_dir / "sql" / "models" / "base_marker.sql").write_text(
        'MODEL (description "Base marker");\\n\\nSELECT 1 AS id\\n'
    )
    (project_dir / "base_marker.txt").write_text(kind)
"""
FAILING_GENERATOR: str = """def write_benchmark_project(*, kind, project_dir, models):
    raise RuntimeError("base generator exploded")
"""
_LOGGING_PYTHON: str = """#!/usr/bin/env bash
project=""
previous=""
for argument in "$@"; do
  if [ "$previous" = "--project-dir" ]; then project="$argument"; fi
  previous="$argument"
done
if [ -n "$project" ]; then
  if [ -f "$project/base_marker.txt" ]; then echo base >> "{log}"; else echo head >> "{log}"; fi
fi
exec "{python}" "$@"
"""


def write_fake_base_root(*, root: Path, generator: str) -> Path:
    """Write a base checkout whose only content is a benchmark generator module."""

    helpers: Path = root / "scripts" / "compile_performance_ratio" / "_helpers"
    helpers.mkdir(parents=True)
    for package in (root / "scripts", helpers.parent, helpers):
        (package / "__init__.py").write_text("", encoding="utf-8")
    (helpers / "measure.py").write_text(generator, encoding="utf-8")
    return root


def write_logging_python(*, path: Path, log: Path) -> Path:
    """Write an interpreter wrapper logging which project each compile received."""

    path.write_text(_LOGGING_PYTHON.format(log=log, python=sys.executable), encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    log.touch()
    return path


def logged_projects(log: Path) -> tuple[str, ...]:
    """Return the project kind each logged compile received, in order."""

    return tuple(log.read_text(encoding="utf-8").split())
