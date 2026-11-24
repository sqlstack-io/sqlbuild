"""Integration coverage for writing formatted files back in path order."""

from __future__ import annotations

import os
import re
import stat
import sys
from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    FormatWriteFailureTestCase,
)

_UNFORMATTED_SQL: str = (
    'MODEL (description "Orders placed by customers.", materialized table);\n'
    "select   order_id,   customer_id from __source('shop', 'orders')\n"
)


@pytest.mark.skipif(
    sys.platform == "win32" or os.geteuid() == 0,
    reason="needs a directory whose permissions the current user cannot override",
)
@pytest.mark.parametrize(
    "test_case",
    [
        FormatWriteFailureTestCase(
            description="a failed first write leaves every later file as authored",
            directories=("a_locked", "b_open"),
            locked_directory="a_locked",
            expected_replaced=(False, False),
            expected_error=OSError,
            expected_status="Formatting SQL  ERROR",
        ),
        FormatWriteFailureTestCase(
            description="files before a failed write are replaced and later ones are not",
            directories=("a_open", "b_locked", "c_open", "d_open"),
            locked_directory="b_locked",
            expected_replaced=(True, False, False, False),
            expected_error=OSError,
            expected_status="Formatting SQL  ERROR",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unwritable_file_when_formatting_then_later_files_are_left_as_authored(
    test_case: FormatWriteFailureTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    files: list[Path] = []
    directory: str
    for directory in test_case.directories:
        (tmp_path / "sql" / "models" / directory).mkdir(parents=True)
        model: Path = tmp_path / "sql" / "models" / directory / "orders.sql"
        model.write_text(_UNFORMATTED_SQL, encoding="utf-8")
        files.append(model)
    locked: Path = tmp_path / "sql" / "models" / test_case.locked_directory
    locked.chmod(stat.S_IRUSR | stat.S_IXUSR)
    try:
        with pytest.raises(test_case.expected_error, match=re.escape(str(locked / "orders.sql"))):
            _ = main(["--project-dir", str(tmp_path), "format", "--no-cache"])
    finally:
        locked.chmod(stat.S_IRWXU)

    assert test_case.expected_status in capsys.readouterr().err
    assert (
        tuple(path.read_text(encoding="utf-8") != _UNFORMATTED_SQL for path in files)
        == test_case.expected_replaced
    )
    assert [path.name for path in (tmp_path / "sql" / "models").rglob(".*")] == []


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
