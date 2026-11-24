"""Staged cold-compile artifacts publish safely beside existing trees and concurrent cleanup."""

from __future__ import annotations

import os
import shutil
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest
from filelock import FileLock, Timeout

from sqlbuild._native import PreparedCompileArtifacts, StagedArtifactsChangedError, WrittenTarget
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.unit.src.sqlbuild.cli.commands.main.compile._test_types import (
    StagedPublishTestCase,
    StagingInterleavingTestCase,
    TamperedStagedPublishTestCase,
)
from tests.unit.src.sqlbuild.cli.commands.main.compile.helpers import (
    StagingEveryProject,
    build_static_target_writer_project,
    read_target_files,
    staged_then_published,
    write_relative_files,
)

_MODEL_FILE: str = "compiled/models/staging/orders.sql"
_FUNCTION_FILE: str = "compiled/functions/sql/is_completed_order.sql"
_PUBLISHED_FILES: dict[str, str] = {
    _MODEL_FILE: "SELECT 2 AS order_id\n",
    _FUNCTION_FILE: (
        "CREATE OR REPLACE MACRO analytics.is_completed_order(order_status) AS (\n"
        "order_status = 'completed'\n"
        ")\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        StagedPublishTestCase(
            description="new files join an existing tree and replace its stale files",
            existing_files={"compiled/models/obsolete.sql": "SELECT 0\n"},
            expected_files=_PUBLISHED_FILES,
            expected_removed=("compiled/models/obsolete.sql",),
        ),
        StagedPublishTestCase(
            description="a cold tree is published whole",
            existing_files={},
            expected_files=_PUBLISHED_FILES,
            expected_removed=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_staged_artifacts_when_publishing_then_target_holds_exactly_the_staged_files(
    test_case: StagedPublishTestCase, tmp_path: Path
) -> None:
    target: Path = tmp_path / "target"
    write_relative_files(root=target, files=test_case.existing_files)

    written: WrittenTarget | None = staged_then_published(target=target)

    assert written is not None
    assert written.target_dir == target
    assert read_target_files(target, test_case.expected_files) == test_case.expected_files
    assert not any((target / relative).exists() for relative in test_case.expected_removed)
    assert sorted(path.name for path in target.rglob("*.tmp")) == []
    assert sorted(path.name for path in target.glob(".sqlbuild-staging-*")) == []


@pytest.mark.parametrize(
    "test_case",
    [
        TamperedStagedPublishTestCase(
            description="a staged file deleted after preparation",
            existing_files={"compiled/models/obsolete.sql": "SELECT 0\n"},
            removed_staged_files=(_MODEL_FILE,),
            expected_kept_files={"compiled/models/obsolete.sql": "SELECT 0\n"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_changed_staging_tree_when_publishing_then_nothing_stale_is_deleted(
    test_case: TamperedStagedPublishTestCase, tmp_path: Path
) -> None:
    target: Path = tmp_path / "target"
    write_relative_files(root=target, files=test_case.existing_files)

    def remove_staged(root: Path) -> None:
        for relative in test_case.removed_staged_files:
            for staged in root.glob(f".sqlbuild-staging-*/{relative}"):
                staged.unlink()

    with pytest.raises(StagedArtifactsChangedError):
        _ = staged_then_published(target=target, before_publish=remove_staged)

    assert read_target_files(target, test_case.expected_kept_files) == (
        test_case.expected_kept_files
    )
    assert not (target / _MODEL_FILE).exists()


@pytest.mark.parametrize(
    "test_case",
    [
        StagingInterleavingTestCase(
            description="cleanup during staging creation keeps the live compile's tree",
            existing_files={
                ".sqlbuild-staging-abandoned/compiled/models/orders.sql": "SELECT 0\n",
                "compiled/models/obsolete.sql": "SELECT 0\n",
            },
            expected_files=_PUBLISHED_FILES,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cleanup_interleaved_with_staging_when_compiling_then_live_staging_survives(
    test_case: StagingInterleavingTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    target: Path = tmp_path / "target"
    write_relative_files(root=target, files=test_case.existing_files)
    survived: list[bool] = []
    remove_tree: Callable[..., None] = shutil.rmtree

    class CleanedWhileStaging(StagingEveryProject):
        def create_staging_directory(self, staging_dir: Path) -> None:
            super().create_staging_directory(staging_dir)
            PreparedCompileArtifacts(enabled=True).remove_abandoned_staging(target_dir=target)
            survived.append((staging_dir / ".owner").is_file())

    def remove_while_locked(path: Any, *args: Any, **kwargs: Any) -> None:
        with pytest.raises(Timeout):
            FileLock(f"{os.fspath(path)}.lock", timeout=0).acquire()
        remove_tree(path, *args, **kwargs)

    monkeypatch.setattr(shutil, "rmtree", remove_while_locked)

    with CleanedWhileStaging(enabled=True) as artifacts:
        artifacts.remove_abandoned_staging(target_dir=target)
        artifacts.start(
            project=build_static_target_writer_project(),
            adapter=DuckDbAdapter(),
            target_dir=target,
            stage_artifacts=True,
        )
        written: WrittenTarget | None = artifacts.publish(target_dir=target, manifest=None)

    assert survived == [True]
    assert written is not None
    assert read_target_files(target, test_case.expected_files) == test_case.expected_files
    assert not (target / "compiled" / "sql" / "models" / "obsolete.sql").exists()
    assert sorted(path.name for path in target.glob(".sqlbuild-staging-*")) == []


@pytest.mark.parametrize(
    "test_case",
    [
        TamperedStagedPublishTestCase(
            description="the owner marker removed from a live staging tree",
            existing_files={"compiled/models/obsolete.sql": "SELECT 0\n"},
            removed_staged_files=(".owner",),
            expected_kept_files={"compiled/models/obsolete.sql": "SELECT 0\n"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_staging_owner_lost_when_publishing_then_compile_fails_without_deleting(
    test_case: TamperedStagedPublishTestCase, tmp_path: Path
) -> None:
    target: Path = tmp_path / "target"
    write_relative_files(root=target, files=test_case.existing_files)

    with StagingEveryProject(enabled=True) as artifacts:
        artifacts.start(
            project=build_static_target_writer_project(),
            adapter=DuckDbAdapter(),
            target_dir=target,
            stage_artifacts=True,
        )
        _ = artifacts.planning_diagnostics()
        for relative in test_case.removed_staged_files:
            for staged in target.glob(f".sqlbuild-staging-*/{relative}"):
                staged.unlink()
        with pytest.raises(StagedArtifactsChangedError):
            artifacts.publish(target_dir=target, manifest=None)

    assert read_target_files(target, test_case.expected_kept_files) == (
        test_case.expected_kept_files
    )
    assert not (target / _MODEL_FILE).exists()


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-vv"]))
