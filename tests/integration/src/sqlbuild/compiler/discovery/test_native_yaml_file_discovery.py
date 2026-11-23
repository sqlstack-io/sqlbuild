"""Source and schema YAML files loaded natively, each failing on its own."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.discovery._test_types import (
    NativeYamlLoadTestCase,
)
from tests.integration.src.sqlbuild.compiler.discovery.helpers import (
    native_yaml_tags_and_values,
    write_project,
)

_BIG_DECIMAL: str = "9" * 4000
_BIG_HEX: str = "0x" + "f" * 40


@pytest.mark.parametrize(
    "test_case",
    [
        NativeYamlLoadTestCase(
            description="a big decimal integer loads exactly as a value and an explicit key",
            files=(
                (
                    "sql/sources/a.yml",
                    (
                        f"sources:\n  - name: orders\n    meta:\n      big: {_BIG_DECIMAL}\n"
                        f"      ? {_BIG_DECIMAL}\n      : key\n"
                    ).encode(),
                ),
            ),
            expected_native_tags=("ok",),
            expected_values=(
                {
                    "sources": [
                        {
                            "name": "orders",
                            "meta": {"big": int(_BIG_DECIMAL), int(_BIG_DECIMAL): "key"},
                        }
                    ]
                },
            ),
        ),
        NativeYamlLoadTestCase(
            description="a hexadecimal integer beyond 64 bits fails",
            files=(("sql/sources/a.yml", f"sources: []\nmeta: {{v: {_BIG_HEX}}}\n".encode()),),
            expected_native_tags=("error",),
            expected_values=(),
        ),
        NativeYamlLoadTestCase(
            description="an invalid file fails on its own and a later file still loads",
            files=(
                ("sql/sources/a.yml", b"sources: [\n"),
                ("sql/sources/b.yml", b"sources: []\nmeta: {v: on, w: 0o17}\n"),
            ),
            expected_native_tags=("error", "ok"),
            expected_values=({"sources": [], "meta": {"v": "on", "w": 15}},),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_yaml_files_when_loading_natively_then_each_file_loads_or_fails_alone(
    test_case: NativeYamlLoadTestCase, tmp_path: Path
) -> None:
    write_project(project_dir=tmp_path, files=test_case.files)

    tags, values = native_yaml_tags_and_values(
        project_dir=tmp_path, relative_paths=[path for path, _contents in test_case.files]
    )

    assert (tags, values) == (
        list(test_case.expected_native_tags),
        list(test_case.expected_values),
    ), test_case.description


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
