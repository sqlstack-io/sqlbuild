"""Integration coverage for the format result cache: clean files are skipped until they change."""

from __future__ import annotations

from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    FormatCacheFaultTestCase,
    FormatCacheInvalidationTestCase,
    FormatCacheReuseTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    check_format_with_work,
    damage_format_result_store,
    edit_cached_orders_model,
    rebuild_formatter,
    widen_format_line_width,
    write_formatted_cache_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCacheReuseTestCase(
            description="check skips every file a previous check found clean",
            arguments=(),
            expected_hits=3,
            expected_misses=0,
            expected_formatted_files=0,
        ),
        FormatCacheReuseTestCase(
            description="diff reuses the same clean results",
            arguments=("--diff",),
            expected_hits=3,
            expected_misses=0,
            expected_formatted_files=0,
        ),
        FormatCacheReuseTestCase(
            description="no-cache formats every file again",
            arguments=("--no-cache",),
            expected_hits=0,
            expected_misses=0,
            expected_formatted_files=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_clean_checked_project_when_checking_again_then_cached_files_are_not_formatted(
    test_case: FormatCacheReuseTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_formatted_cache_project(project_dir=tmp_path)
    _ = capsys.readouterr()
    cold_exit, cold_work = check_format_with_work(project_dir=tmp_path)
    cold: CaptureResult[str] = capsys.readouterr()

    warm_exit, warm_work = check_format_with_work(
        project_dir=tmp_path, arguments=test_case.arguments
    )
    warm: CaptureResult[str] = capsys.readouterr()

    assert cold_exit == 0
    assert cold_work.get("format_cache_misses", 0) == 3
    assert warm_exit == 0
    assert warm_work.get("format_cache_hits", 0) == test_case.expected_hits
    assert warm_work.get("format_cache_misses", 0) == test_case.expected_misses
    assert warm_work.get("format_lint_files", 0) == test_case.expected_formatted_files
    assert warm.out == cold.out
    assert "3 files checked, 0 changed" in warm.err


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCacheInvalidationTestCase(
            description="an edited file is formatted again and reported",
            change=edit_cached_orders_model,
            expected_hits=2,
            expected_misses=1,
            expected_exit_code=1,
            expected_recheck_hits=2,
            expected_output_fragment="orders.sql",
        ),
        FormatCacheInvalidationTestCase(
            description="changed format settings format every file again",
            change=widen_format_line_width,
            expected_hits=0,
            expected_misses=3,
            expected_exit_code=0,
            expected_recheck_hits=3,
        ),
        FormatCacheInvalidationTestCase(
            description="another formatter build formats every file again",
            change=rebuild_formatter,
            expected_hits=0,
            expected_misses=3,
            expected_exit_code=0,
            expected_recheck_hits=3,
        ),
        FormatCacheInvalidationTestCase(
            description="a damaged store is ignored",
            change=damage_format_result_store,
            expected_hits=0,
            expected_misses=3,
            expected_exit_code=0,
            expected_recheck_hits=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cached_clean_project_when_inputs_change_then_affected_files_are_formatted_again(
    test_case: FormatCacheInvalidationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_formatted_cache_project(project_dir=tmp_path)
    assert check_format_with_work(project_dir=tmp_path)[0] == 0
    _ = capsys.readouterr()
    test_case.change(tmp_path, monkeypatch)

    exit_code, work = check_format_with_work(project_dir=tmp_path)
    captured: CaptureResult[str] = capsys.readouterr()
    rechecked_exit, rechecked_work = check_format_with_work(project_dir=tmp_path)

    assert exit_code == test_case.expected_exit_code
    assert work.get("format_cache_hits", 0) == test_case.expected_hits
    assert work.get("format_cache_misses", 0) == test_case.expected_misses
    assert test_case.expected_output_fragment in captured.out
    assert rechecked_exit == test_case.expected_exit_code
    assert rechecked_work.get("format_cache_hits", 0) == test_case.expected_recheck_hits


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCacheFaultTestCase(
            description="a missing description is reported on every check",
            removed_text='description "Products in the catalog.", ',
            expected_code="description-present",
            expected_hits=2,
            expected_misses=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_file_with_fault_when_checking_twice_then_fault_is_reported_both_times(
    test_case: FormatCacheFaultTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_formatted_cache_project(project_dir=tmp_path)
    products: Path = tmp_path / "sql" / "models" / "products.sql"
    _ = products.write_text(
        products.read_text(encoding="utf-8").replace(test_case.removed_text, ""),
        encoding="utf-8",
    )
    _ = capsys.readouterr()

    first_exit, first_work = check_format_with_work(project_dir=tmp_path)
    first: CaptureResult[str] = capsys.readouterr()
    second_exit, second_work = check_format_with_work(project_dir=tmp_path)
    second: CaptureResult[str] = capsys.readouterr()

    assert first_exit == second_exit == 1
    assert test_case.expected_code in first.out
    assert second.out == first.out
    assert first_work.get("format_cache_misses", 0) == 3
    assert second_work.get("format_cache_hits", 0) == test_case.expected_hits
    assert second_work.get("format_cache_misses", 0) == test_case.expected_misses


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
