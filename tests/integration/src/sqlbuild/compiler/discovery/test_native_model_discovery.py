"""Native discovery reads SQL model files, failures and project inputs."""

from __future__ import annotations

import os
from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredSqlModelFile,
    directory_snapshot_scope,
    discover_model_files,
    discover_python_module_files,
)
from tests.integration.src.sqlbuild.compiler.discovery._test_types import (
    NativeRuntimeTestCase,
    SharedSnapshotTestCase,
)
from tests.integration.src.sqlbuild.compiler.discovery.helpers import (
    FailureCapture,
    write_project,
)

_VALID_MODEL: bytes = b"MODEL (materialized table);\nSELECT order_id, total AS amount FROM orders"


@pytest.mark.parametrize(
    "test_case",
    [
        NativeRuntimeTestCase(
            description="the running Python and a UTF-8 root run natively",
            project_name="orders",
            expected_models=1,
        ),
        NativeRuntimeTestCase(
            description="a non-UTF-8 project root fails showing the path lossily",
            project_name=os.fsdecode(b"orders\xff"),
            expected_models=0,
            expected_error="ProjectPathError",
            expected_message="orders\\xff is not valid UTF-8; move the project to a directory",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_runtime_and_root_when_discovering_natively_then_supported_runs_and_others_fail(
    test_case: NativeRuntimeTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / test_case.project_name
    write_project(project_dir=project_dir, files=(("sql/models/orders.sql", _VALID_MODEL),))
    capture: FailureCapture = FailureCapture()
    models: list[DiscoveredSqlModelFile] = []

    with capture:
        models.extend(
            discover_model_files(
                project_dir=project_dir,
                extract_implicit_alias_columns=True,
                extract_output_column_locations=True,
            )
        )

    assert (
        len(models),
        type(capture.failure).__name__,
        test_case.expected_message in str(capture.failure),
    ) == (test_case.expected_models, test_case.expected_error, True), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        SharedSnapshotTestCase(
            description="a Python file created after the model walk stays unseen",
            created_file="sql/models/marts/late.py",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_native_model_walk_when_globbing_models_later_in_the_pass_then_the_walk_is_shared(
    test_case: SharedSnapshotTestCase, tmp_path: Path
) -> None:
    write_project(project_dir=tmp_path, files=(("sql/models/marts/orders.sql", _VALID_MODEL),))
    with directory_snapshot_scope(project_dir=tmp_path):
        _ = discover_model_files(
            project_dir=tmp_path,
            extract_implicit_alias_columns=True,
            extract_output_column_locations=True,
        )
        _ = (tmp_path / test_case.created_file).write_text("VALUE = 1\n", encoding="utf-8")
        matches: tuple[Path, ...] = discover_python_module_files(
            project_dir=tmp_path, root="models", public_only=False
        )

    assert tuple(path.relative_to(tmp_path).as_posix() for path in matches) == (
        test_case.expected_matches
    )
