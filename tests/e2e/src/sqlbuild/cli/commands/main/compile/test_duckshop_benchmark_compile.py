"""E2E guard that the DuckShop benchmark project compiles cleanly with this checkout's sqlbuild."""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    BenchmarkCoreCopyCompileTestCase,
    BenchmarkProjectCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import REPO_ROOT, run_sqb

_LOCAL_ONLY_PATHS: tuple[str, ...] = (
    "target",
    "logs",
    ".sqlbuild",
    "sqlbuild_local.toml",
    "__pycache__",
)


@pytest.mark.parametrize(
    "test_case",
    [
        BenchmarkProjectCompileTestCase(
            description="duckshop",
            project_dir=REPO_ROOT / "benchmarks" / "duckshop" / "project",
            expected_errors=0,
            expected_warnings=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_benchmark_project_when_compiling_then_reports_no_errors_or_warnings(
    test_case: BenchmarkProjectCompileTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / test_case.description
    shutil.copytree(
        test_case.project_dir,
        project_dir,
        ignore=shutil.ignore_patterns(*_LOCAL_ONLY_PATHS),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    report: dict[str, Any] = json.loads(result.stdout)
    summary: dict[str, int] = report["summary"]
    diagnostics: list[dict[str, Any]] = report["diagnostics"]
    assert summary["errors"] == test_case.expected_errors, diagnostics
    assert summary["warnings"] == test_case.expected_warnings, diagnostics
    assert summary["models"] > 0
    assert summary["tests"] > 0


@pytest.mark.parametrize(
    "test_case",
    [
        BenchmarkCoreCopyCompileTestCase(
            description="duckshop_core",
            copy_script=REPO_ROOT / "benchmarks" / "duckshop" / "scripts" / "core_project.py",
            expected_absent_folder="/edge_cases/",
            expected_errors=0,
            expected_warnings=0,
            expected_compiled_checksum=(
                "1ff1d0955ea3839b3c969707fd47ca9a4d7d7216753759d23911836f6ea5879d"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_benchmark_core_copy_when_compiling_then_leaves_out_the_excluded_area(
    test_case: BenchmarkCoreCopyCompileTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / test_case.description
    copied: subprocess.CompletedProcess[str] = subprocess.run(
        [sys.executable, str(test_case.copy_script), str(project_dir)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert copied.returncode == 0, copied.stdout + copied.stderr

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    report: dict[str, Any] = json.loads(result.stdout)
    assert report["summary"]["errors"] == test_case.expected_errors, report["diagnostics"]
    assert report["summary"]["warnings"] == test_case.expected_warnings, report["diagnostics"]
    model_paths: str = "\n".join(model["relative_path"] for model in report["resources"]["models"])
    assert model_paths
    assert test_case.expected_absent_folder not in model_paths
    checksum: subprocess.CompletedProcess[str] = subprocess.run(
        [sys.executable, str(test_case.copy_script), "--checksum", str(project_dir)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert checksum.returncode == 0, checksum.stdout + checksum.stderr
    assert checksum.stdout.strip() == test_case.expected_compiled_checksum
