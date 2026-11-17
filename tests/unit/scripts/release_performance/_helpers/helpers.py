"""Builders for release performance comparison test data."""

import subprocess
import sys
from pathlib import Path

from scripts.release_performance._helpers.benchmark import write_baseline_pristine_projects
from scripts.release_performance._helpers.versions import installed_version
from scripts.release_performance.models import CommandComparison, CommandSample

MIB: int = 1024 * 1024
MARKER_GENERATOR: str = """from pathlib import Path


def write_pristine_projects(*, root: Path, inspection_models: int, build_models: int) -> Path:
    for name, models in (("inspection", inspection_models), ("build", build_models)):
        (root / "pristine" / name).mkdir(parents=True)
        (root / "pristine" / name / "BASELINE_MARKER").write_text(str(models))
    return root / "pristine"
"""
BASELINE_ONLY_SYMBOL: str = "BASELINE_ONLY_RELEASE_SYMBOL"
BASELINE_SQLBUILD_GENERATOR: str = f"""from pathlib import Path

from sqlbuild import {BASELINE_ONLY_SYMBOL}


def write_pristine_projects(*, root: Path, inspection_models: int, build_models: int) -> Path:
    for name in ("inspection", "build"):
        (root / "pristine" / name).mkdir(parents=True)
        (root / "pristine" / name / "BASELINE_MARKER").write_text({BASELINE_ONLY_SYMBOL})
    return root / "pristine"
"""
FAILING_GENERATOR: str = """def write_pristine_projects(*, root, inspection_models, build_models):
    raise RuntimeError("baseline generator exploded")
"""
HANGING_GENERATOR: str = """import time


def write_pristine_projects(*, root, inspection_models, build_models):
    time.sleep(60)
"""
MARKER_DENSE_GENERATOR: str = """from pathlib import Path


def write_dense_compile_project(*, project_dir: Path, model_count: int) -> None:
    project_dir.mkdir(parents=True)
    (project_dir / "BASELINE_MARKER").write_text(str(model_count))
"""
BASELINE_SQLBUILD_DENSE_GENERATOR: str = f"""from pathlib import Path

from sqlbuild import {BASELINE_ONLY_SYMBOL}


def write_dense_compile_project(*, project_dir: Path, model_count: int) -> None:
    project_dir.mkdir(parents=True)
    (project_dir / "BASELINE_MARKER").write_text({BASELINE_ONLY_SYMBOL})
"""
FAILING_DENSE_GENERATOR: str = """def write_dense_compile_project(*, project_dir, model_count):
    raise RuntimeError("baseline dense generator exploded")
"""


def comparison(
    *,
    name: str,
    baseline: tuple[tuple[float, float, int], ...],
    candidate: tuple[tuple[float, float, int], ...],
    max_time_ratio: float | None = None,
    max_rss_ratio: float | None = None,
) -> CommandComparison:
    """Build a comparison from (wall seconds, CPU seconds, peak RSS MiB) samples."""

    return CommandComparison(
        name=name,
        baseline=tuple(_sample(values=values) for values in baseline),
        candidate=tuple(_sample(values=values) for values in candidate),
        max_time_ratio=max_time_ratio,
        max_rss_ratio=max_rss_ratio,
    )


def release_file(*, filename: str) -> dict[str, object]:
    """Build one PEP 691 file entry that is not yanked."""

    return {"filename": filename, "yanked": False, "url": filename}


def _sample(*, values: tuple[float, float, int]) -> CommandSample:
    wall, cpu, rss_mib = values
    return CommandSample(wall_seconds=wall, cpu_seconds=cpu, peak_rss_bytes=rss_mib * MIB)


def write_baseline_source(*, root: Path, generator: str) -> Path:
    """Write a source tree whose only content is a release benchmark generator."""

    helpers: Path = root / "scripts" / "release_performance" / "_helpers"
    helpers.mkdir(parents=True)
    for package in (root / "scripts", helpers.parent, helpers):
        _ = (package / "__init__.py").write_text("", encoding="utf-8")
    _ = (helpers / "benchmark.py").write_text(generator, encoding="utf-8")
    return root


def write_baseline_dense_source(*, root: Path, generator: str) -> Path:
    """Write a source tree whose only content is a dense benchmark generator."""

    helpers: Path = root / "scripts" / "cold_compile_performance" / "_helpers"
    helpers.mkdir(parents=True)
    for package in (root / "scripts", helpers.parent, helpers):
        _ = (package / "__init__.py").write_text("", encoding="utf-8")
    _ = (helpers / "dense_project.py").write_text(generator, encoding="utf-8")
    return root


def write_baseline_environment(*, root: Path) -> Path:
    """Create a virtual environment whose sqlbuild defines a symbol this checkout lacks."""

    _ = subprocess.run(
        [sys.executable, "-m", "venv", "--without-pip", str(root)], check=True, capture_output=True
    )
    python: Path = root / "bin" / "python"
    purelib: str = subprocess.run(
        [str(python), "-c", "import sysconfig; print(sysconfig.get_path('purelib'))"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    package: Path = Path(purelib) / "sqlbuild"
    package.mkdir(parents=True)
    _ = (package / "__init__.py").write_text(
        f"{BASELINE_ONLY_SYMBOL} = {BASELINE_ONLY_SYMBOL!r}\n", encoding="utf-8"
    )
    return python


def tagged_repository(*, root: Path, tag: str) -> Path:
    """Create a one-commit git repository with a tracked file and the given tag."""

    root.mkdir(parents=True)
    _ = (root / "release.txt").write_text(tag, encoding="utf-8")
    for command in (
        ("init", "--quiet"),
        ("add", "release.txt"),
        ("-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "--quiet", "-m", "r"),
        ("tag", tag),
    ):
        _ = subprocess.run(["git", "-C", str(root), *command], check=True, capture_output=True)
    return root


def generate_with_hanging_generator(root: Path) -> object:
    """Generate baseline projects with a generator that never returns."""

    return write_baseline_pristine_projects(
        source=write_baseline_source(root=root / "source", generator=HANGING_GENERATOR),
        python=write_baseline_environment(root=root / "venv"),
        root=root / "generated",
        inspection_models=30,
        build_models=10,
    )


def read_hanging_sqb_version(root: Path) -> object:
    """Read the version of an sqb that never exits."""

    sqb: Path = root / "sqb"
    _ = sqb.write_text("#!/bin/sh\nexec sleep 60\n", encoding="utf-8")
    sqb.chmod(0o755)
    return installed_version(sqb=sqb)
