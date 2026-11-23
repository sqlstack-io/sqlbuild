"""End-to-end coverage for parallel format determinism and the format result cache."""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

from scripts.format_performance._helpers.model_project import write_model_format_project
from sqlbuild import _native as native_module
from tests.e2e.src.sqlbuild.cli.commands.main.format._test_types import (
    FormatCacheEndToEndTestCase,
    FormatParallelDeterminismTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.format.helpers import (
    FORMAT_CACHE_UNFORMATTED_ORDERS,
    model_tree,
    write_mixed_format_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_SERIAL: dict[str, str] = {"RAYON_NUM_THREADS": "1"}
_PARALLEL: dict[str, str] = {"RAYON_NUM_THREADS": "4"}


@pytest.mark.parametrize(
    "test_case",
    [
        FormatParallelDeterminismTestCase(
            description="check report", arguments=("--check",), expected_returncode=1
        ),
        FormatParallelDeterminismTestCase(
            description="json report", arguments=("--check", "--json"), expected_returncode=1
        ),
        FormatParallelDeterminismTestCase(
            description="diff", arguments=("--diff",), expected_returncode=1
        ),
        FormatParallelDeterminismTestCase(
            description="written files", arguments=(), expected_returncode=1
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_mixed_project_when_formatting_serially_and_in_parallel_then_results_match(
    test_case: FormatParallelDeterminismTestCase,
    tmp_path: Path,
) -> None:
    serial_dir: Path = tmp_path / "serial"
    write_mixed_format_project(project_dir=serial_dir, model_count=60)
    parallel_dir: Path = tmp_path / "parallel"
    _ = shutil.copytree(serial_dir, parallel_dir)
    command: tuple[str, ...] = ("--no-color", "format", "--no-cache", *test_case.arguments)

    serial: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=serial_dir, command=command, env=_SERIAL
    )
    parallel: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=parallel_dir, command=command, env=_PARALLEL
    )

    assert serial.returncode == test_case.expected_returncode, serial.stderr
    assert parallel.returncode == test_case.expected_returncode, parallel.stderr
    assert parallel.stdout.replace(str(parallel_dir), "<project>") == serial.stdout.replace(
        str(serial_dir), "<project>"
    )
    assert model_tree(project_dir=parallel_dir) == model_tree(project_dir=serial_dir)


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCacheEndToEndTestCase(
            description="generated orders project",
            model_count=40,
            edited_model="orders_00007.sql",
            expected_warm_summary="40 files checked, 0 changed",
            expected_edited_summary="40 files checked, 1 changed",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_formatted_project_when_checking_warm_edited_and_damaged_then_results_stay_exact(
    test_case: FormatCacheEndToEndTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = tmp_path / "project"
    write_model_format_project(project_dir=project_dir, model_count=test_case.model_count)
    store: Path = (
        native_module.compiler_cache_directory(project_dir)
        / native_module.FORMAT_RESULT_STORE_FILE_NAME
    )
    check: tuple[str, ...] = ("--no-color", "format", "--check")

    formatted: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "format")
    )
    cold: subprocess.CompletedProcess[str] = run_sqb(project_dir=project_dir, command=check)
    stored: bytes = store.read_bytes()
    warm: subprocess.CompletedProcess[str] = run_sqb(project_dir=project_dir, command=check)
    _ = (project_dir / "sql" / "models" / test_case.edited_model).write_text(
        FORMAT_CACHE_UNFORMATTED_ORDERS, encoding="utf-8"
    )
    edited: subprocess.CompletedProcess[str] = run_sqb(project_dir=project_dir, command=check)
    _ = store.write_bytes(b"not a native store")
    damaged: subprocess.CompletedProcess[str] = run_sqb(project_dir=project_dir, command=check)
    uncached: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=(*check, "--no-cache")
    )

    assert formatted.returncode == 0, formatted.stderr
    assert cold.returncode == 0, cold.stderr
    assert stored
    assert warm.returncode == 0, warm.stderr
    assert warm.stdout == cold.stdout
    assert test_case.expected_warm_summary in warm.stderr
    assert edited.returncode == 1, edited.stderr
    assert test_case.edited_model in edited.stdout
    assert test_case.expected_edited_summary in edited.stderr
    assert damaged.returncode == 1, damaged.stderr
    assert damaged.stdout == edited.stdout
    assert uncached.stdout == edited.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
