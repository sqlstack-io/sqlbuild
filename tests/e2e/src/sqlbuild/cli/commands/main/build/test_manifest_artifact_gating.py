from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ManifestArtifactGatingE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestArtifactGatingE2ETestCase(
            description="build does not write manifest.json; --manifest opts in",
            expected_manifest_after_build=False,
            expected_manifest_after_compile_manifest=True,
            expected_manifest_after_build_manifest=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_direct_project_when_building_then_manifest_requires_compile_manifest_flag(
    test_case: ManifestArtifactGatingE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="manifest_gating",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "manifest_gating"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "warehouse.duckdb"\n'
            ),
            "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS order_id\n",
        },
    )
    manifest_path: Path = project_dir / "target" / "manifest.json"

    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    assert manifest_path.exists() == test_case.expected_manifest_after_build

    compile_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--manifest"),
        project_dir=project_dir,
    )

    assert compile_result.returncode == 0, compile_result.stdout + compile_result.stderr
    assert manifest_path.exists() == test_case.expected_manifest_after_compile_manifest

    manifest_path.unlink()
    build_manifest_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--manifest"),
        project_dir=project_dir,
    )

    assert build_manifest_result.returncode == 0, (
        build_manifest_result.stdout + build_manifest_result.stderr
    )
    assert manifest_path.exists() == test_case.expected_manifest_after_build_manifest
