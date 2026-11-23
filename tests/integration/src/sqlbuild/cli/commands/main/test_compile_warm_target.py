"""End-to-end coverage for warm compile target reconciliation."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.commands.main._test_helpers import (
    changed_artifacts,
    compile_project,
    compiled_artifacts,
    pruned_entries,
    stamp_artifacts,
    write_stale_target_entries,
    write_warm_target_project,
)
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    WarmTargetEditTestCase,
    WarmTargetProjectConfigEditTestCase,
    WarmTargetTestCase,
)

_UNCHANGED_MTIME_NS: int = 1_000_000_000


@pytest.mark.parametrize(
    "test_case",
    [
        WarmTargetTestCase("compile_cache_enabled", compile_args=()),
        WarmTargetTestCase("compile_cache_disabled", compile_args=("--no-cache",)),
    ],
    ids=lambda case: case.description,
)
def test_given_warm_target_with_stale_entries_when_compiling_then_preserves_unchanged_and_prunes(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], test_case: WarmTargetTestCase
) -> None:
    project_dir: Path = tmp_path / "project"
    write_warm_target_project(project_dir=project_dir)
    first_exit_code: int = compile_project(
        project_dir=project_dir, compile_args=test_case.compile_args
    )
    compiled_dir: Path = project_dir / "target" / "compiled"
    managed: tuple[Path, ...] = compiled_artifacts(compiled_dir=compiled_dir)
    stamp_artifacts(artifacts=managed, mtime_ns=_UNCHANGED_MTIME_NS)
    stale: dict[str, Path] = write_stale_target_entries(
        compiled_dir=compiled_dir, outside_dir=tmp_path / "shared"
    )

    second_exit_code: int = compile_project(
        project_dir=project_dir, compile_args=test_case.compile_args
    )
    _ = capsys.readouterr()

    assert (first_exit_code, second_exit_code) == (
        test_case.expected_exit_code,
        test_case.expected_exit_code,
    )
    assert changed_artifacts(compiled_dir=compiled_dir, mtime_ns=_UNCHANGED_MTIME_NS) == (
        test_case.expected_changed
    )
    assert compiled_artifacts(compiled_dir=compiled_dir) == managed
    assert pruned_entries(entries=stale, names=("legacy", "empty")) == test_case.expected_pruned
    assert stale["link"].is_symlink() is test_case.expected_link_kept
    assert stale["outside_file"].is_file() is test_case.expected_link_kept


@pytest.mark.parametrize(
    "test_case",
    [
        WarmTargetEditTestCase(
            "one_model_edited",
            edited_path="sql/models/inventory.sql",
            edited_contents="MODEL (description 'Test model inventory.');\nSELECT 3 AS item_id\n",
            expected_changed=("sql/models/inventory.sql",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_one_edited_model_when_compiling_warm_then_rewrites_only_its_artifacts(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], test_case: WarmTargetEditTestCase
) -> None:
    write_warm_target_project(project_dir=tmp_path)
    _ = compile_project(project_dir=tmp_path, compile_args=())
    compiled_dir: Path = tmp_path / "target" / "compiled"
    stamp_artifacts(
        artifacts=compiled_artifacts(compiled_dir=compiled_dir), mtime_ns=_UNCHANGED_MTIME_NS
    )
    (tmp_path / test_case.edited_path).write_text(test_case.edited_contents, encoding="utf-8")

    exit_code: int = compile_project(project_dir=tmp_path, compile_args=())
    _ = capsys.readouterr()

    assert exit_code == test_case.expected_exit_code
    assert changed_artifacts(compiled_dir=compiled_dir, mtime_ns=_UNCHANGED_MTIME_NS) == (
        test_case.expected_changed
    )


@pytest.mark.parametrize(
    "test_case",
    [
        WarmTargetProjectConfigEditTestCase(
            "unused_var_added",
            edited_project_config=('name = "shop"\nadapter = "duckdb"\n\n[vars]\nrevision = "1"\n'),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_config_edit_when_compiling_twice_then_rewrites_no_unchanged_artifact(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    test_case: WarmTargetProjectConfigEditTestCase,
) -> None:
    write_warm_target_project(project_dir=tmp_path)
    _ = compile_project(project_dir=tmp_path, compile_args=())
    compiled_dir: Path = tmp_path / "target" / "compiled"
    stamp_artifacts(
        artifacts=compiled_artifacts(compiled_dir=compiled_dir), mtime_ns=_UNCHANGED_MTIME_NS
    )
    (tmp_path / "sqlbuild_project.toml").write_text(
        test_case.edited_project_config, encoding="utf-8"
    )

    edit_exit_code: int = compile_project(project_dir=tmp_path, compile_args=())
    edit_changed: tuple[str, ...] = changed_artifacts(
        compiled_dir=compiled_dir, mtime_ns=_UNCHANGED_MTIME_NS
    )
    after_edit_exit_code: int = compile_project(project_dir=tmp_path, compile_args=())
    _ = capsys.readouterr()

    assert (edit_exit_code, after_edit_exit_code) == test_case.expected_exit_codes
    assert edit_changed == test_case.expected_edit_changed
    assert changed_artifacts(compiled_dir=compiled_dir, mtime_ns=_UNCHANGED_MTIME_NS) == (
        test_case.expected_after_edit_changed
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
