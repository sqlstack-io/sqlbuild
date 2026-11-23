"""Scope E2E subprocess helpers."""

import shutil
import statistics
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from time import perf_counter

from sqlbuild._native import compiler_cache_directory
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import REPO_ROOT

SCOPE_CACHE_DIRECTORY_NAME: str = "declaration-scopes-v2"
SCOPE_CACHE_FILENAME: str = "scope-index.json"


def run_scope_alias(
    *,
    alias: str,
    project_dir: Path,
    args: tuple[str, ...],
    timeout_seconds: float | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            str(Path(sys.executable).with_name(alias)),
            "--project-dir",
            str(project_dir),
            "scope",
            *args,
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
        timeout=timeout_seconds,
    )


def write_scope_performance_project(
    *, project_dir: Path, model_count: int, domain_count: int
) -> None:
    """Write a valid domain-shaped project with descendant-public macros."""

    project_dir.mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "scope_performance"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    for domain in range(domain_count):
        first_model_index: int = domain * model_count // domain_count
        next_model_index: int = (domain + 1) * model_count // domain_count
        models_in_domain: int = next_model_index - first_model_index
        root_model_count: int = min(10, models_in_domain)
        domain_dir: Path = project_dir / "sql" / "models" / f"domain_{domain:03d}"
        macros_dir: Path = domain_dir / "sql" / "macros" / "python"
        macros_dir.mkdir(parents=True)
        macro_source: str = "\n\n".join(
            f"def domain_{domain:03d}_identity_{offset:02d}(expression: str) -> str:\n"
            "    return expression\n"
            for offset in range(10)
        )
        (macros_dir / "identity.py").write_text(macro_source, encoding="utf-8")
        for offset in range(root_model_count):
            model_index: int = first_model_index + offset
            _write_scope_model(
                model_dir=domain_dir,
                model_index=model_index,
                macro_name=f"domain_{domain:03d}_identity_{offset:02d}",
            )
        detail_dir: Path = domain_dir / "detail"
        detail_dir.mkdir()
        for offset in range(root_model_count, models_in_domain):
            model_index = first_model_index + offset
            _write_scope_model(
                model_dir=detail_dir,
                model_index=model_index,
                macro_name=f"domain_{domain:03d}_identity_{offset % 10:02d}",
            )


def write_unicode_macro_project(*, project_dir: Path, macro_names: tuple[str, ...]) -> None:
    """Write one model and a global macro file defining each named macro."""

    (project_dir / "sql" / "models").mkdir(parents=True)
    (project_dir / "sql" / "macros" / "python").mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "scope_unicode"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    (project_dir / "sql" / "models" / "orders.sql").write_text(
        'MODEL(description "Test model orders.");\nSELECT 1 AS id\n', encoding="utf-8"
    )
    macro_source: str = "\n\n".join(
        f"def {name}(expression: str) -> str:\n    return expression\n" for name in macro_names
    )
    (project_dir / "sql" / "macros" / "python" / "nodes" / "names.py").write_text(
        macro_source, encoding="utf-8"
    )


def _write_scope_model(*, model_dir: Path, model_index: int, macro_name: str) -> None:
    (model_dir / f"model_{model_index:05d}.sql").write_text(
        f'MODEL(description "Test model.");\nSELECT @py.{macro_name}("id") AS id\n',
        encoding="utf-8",
    )


def measure_cold_scope_commands(
    *, project_dir: Path, target: str, sample_count: int, timeout_seconds: float
) -> tuple[float, ...]:
    """Measure successful scope commands with no persistent cache."""

    return _measure_scope_commands(
        project_dir=project_dir,
        target=target,
        sample_count=sample_count,
        prepare_sample=_clear_scope_cache,
        timeout_seconds=timeout_seconds,
    )


def measure_warm_scope_commands(
    *, project_dir: Path, target: str, sample_count: int, timeout_seconds: float
) -> tuple[float, ...]:
    """Measure successful scope commands against the persistent cache."""

    return _measure_scope_commands(
        project_dir=project_dir,
        target=target,
        sample_count=sample_count,
        prepare_sample=_preserve_scope_cache,
        timeout_seconds=timeout_seconds,
    )


def _measure_scope_commands(
    *,
    project_dir: Path,
    target: str,
    sample_count: int,
    prepare_sample: Callable[[Path], None],
    timeout_seconds: float,
) -> tuple[float, ...]:
    measurements: list[float] = []
    for _ in range(sample_count):
        prepare_sample(project_dir)
        started: float = perf_counter()
        result: subprocess.CompletedProcess[str] = run_scope_alias(
            alias="sqb",
            project_dir=project_dir,
            args=(target, "--json"),
            timeout_seconds=timeout_seconds,
        )
        measurements.append(perf_counter() - started)
        result.check_returncode()
    return tuple(measurements)


def _clear_scope_cache(project_dir: Path) -> None:
    shutil.rmtree(project_dir / "target", ignore_errors=True)


def _preserve_scope_cache(project_dir: Path) -> None:
    del project_dir


def median_seconds(values: tuple[float, ...]) -> float:
    """Return the median benchmark duration."""

    return statistics.median(values)


def scope_cache_path(*, project_dir: Path) -> Path:
    """Return the persistent Scope Explorer cache path."""

    return compiler_cache_directory(project_dir) / SCOPE_CACHE_DIRECTORY_NAME / SCOPE_CACHE_FILENAME
