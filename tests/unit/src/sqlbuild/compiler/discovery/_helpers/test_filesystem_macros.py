"""Tests for macro file discovery."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredMacroFile,
    discover_macro_files,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    DiscoverMacroFilesTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverMacroFilesTestCase(
            description="discovers Python macro modules but skips package initializers",
            files={
                "sql/macros/python/__init__.py": "raise RuntimeError('must not load')\n",
                "sql/macros/python/orders.py": "def order_columns(): return 'order_id'\n",
                "sql/macros/python/nested/__init__.py": "raise RuntimeError('must not load')\n",
                "sql/macros/python/nested/dates.py": "def current_date(): return 'CURRENT_DATE'\n",
            },
            expected_relative_paths=("sql/macros/python/nested/dates.py", "sql/macros/python/orders.py"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_files_when_discovering_then_skips_package_initializers(
    test_case: DiscoverMacroFilesTestCase,
    tmp_path: Path,
) -> None:
    relative_path: str
    contents: str
    for relative_path, contents in test_case.files.items():
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    result: tuple[DiscoveredMacroFile, ...] = discover_macro_files(project_dir=tmp_path)

    assert (
        tuple(file.relative_path.as_posix() for file in result) == test_case.expected_relative_paths
    )
