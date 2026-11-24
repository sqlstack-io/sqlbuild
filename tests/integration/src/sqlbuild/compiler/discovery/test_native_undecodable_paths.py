"""Callers that tolerate bad files fault only the files whose read paths are not UTF-8."""

from __future__ import annotations

import os
from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import ProjectPathError, discover_project_inputs
from tests.integration.src.sqlbuild.compiler.discovery._test_types import (
    UndecodableDeclarationNameTestCase,
    UndecodablePathToleranceTestCase,
)
from tests.integration.src.sqlbuild.compiler.discovery.helpers import (
    description_inputs_outcome,
    selected_contract_outcome,
    tolerant_scope_fault_outcome,
    write_project,
)

_MODEL: bytes = b"MODEL ();\nSELECT 1 AS id"
_TEST: bytes = b'TEST (name "keeps_orders");\nSELECT 1\n'
_SOURCE: bytes = b"sources:\n  - name: raw_orders\n"
_FILES: tuple[tuple[str, bytes], ...] = (
    ("sql/models/orders.sql", _MODEL),
    (os.fsdecode(b"sql/models/caf\xe9.sql"), _MODEL),
    (os.fsdecode(b"sql/models/notes\xe9.txt"), b"x"),
    ("tests/unit/orders.sql", _TEST),
    (os.fsdecode(b"tests/unit/old\xe9/orders.sql"), _TEST),
    ("sql/sources/raw.yml", _SOURCE),
)
_BAD_SOURCE: tuple[str, bytes] = (os.fsdecode(b"sql/sources/caf\xe9.yml"), _SOURCE)
_RENAME: str = " is not valid UTF-8; rename it so SQLBuild can read it"
_ORDER_STATUS: bytes = b"ENUM (name order_status, members [PLACED]);\n"
_TIER: bytes = b"ENUM (name tier, members [GOLD]);\n"
_READERS: dict[str, Callable[..., tuple[object, ...]]] = {
    "scope": tolerant_scope_fault_outcome,
    "descriptions": description_inputs_outcome,
    "selected contracts": selected_contract_outcome,
}


@pytest.mark.parametrize(
    "test_case",
    [
        UndecodablePathToleranceTestCase(
            description="scope discovery faults each unreadable path and keeps the rest",
            reader="scope",
            files=(*_FILES, _BAD_SOURCE),
            expected_models=("sql/models/orders.sql",),
            expected_sources=("sql/sources/raw.yml",),
            expected_faults=(
                (_FILES[1][0], f"Project path ./sql/models/caf\\xe9.sql{_RENAME}"),
                (_BAD_SOURCE[0], f"Project path ./sql/sources/caf\\xe9.yml{_RENAME}"),
                (_FILES[4][0], f"Project path ./tests/unit/old\\xe9/orders.sql{_RENAME}"),
            ),
        ),
        UndecodablePathToleranceTestCase(
            description="a valid name is not blamed for a raw sibling with the same lossy name",
            reader="scope",
            files=(
                ("sql/models/orders.sql", _MODEL),
                ("sql/models/st\ufffd.sql", b"MODEL ();\nSELECT 2 AS id"),
                (os.fsdecode(b"sql/models/st\xe8.sql"), b"garbage"),
            ),
            expected_models=("sql/models/orders.sql", "sql/models/st\ufffd.sql"),
            expected_sources=(),
            expected_faults=(
                (
                    os.fsdecode(b"sql/models/st\xe8.sql"),
                    f"Project path ./sql/models/st\\xe8.sql{_RENAME}",
                ),
            ),
        ),
        UndecodablePathToleranceTestCase(
            description="model descriptions skip an unreadable model path",
            reader="descriptions",
            files=(*_FILES, _BAD_SOURCE),
            expected_models=("sql/models/orders.sql",),
            expected_sources=(),
        ),
        UndecodablePathToleranceTestCase(
            description="selected contracts never read an unselected model path",
            reader="selected contracts",
            files=_FILES,
            expected_models=("sql/models/orders.sql",),
            expected_sources=("sql/sources/raw.yml",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_undecodable_paths_when_discovering_tolerantly_then_each_read_path_faults_alone(
    test_case: UndecodablePathToleranceTestCase, tmp_path: Path
) -> None:
    write_project(project_dir=tmp_path, files=test_case.files)

    outcome: tuple[object, ...] = _READERS[test_case.reader](project_dir=tmp_path)

    assert outcome == (
        test_case.expected_models,
        test_case.expected_sources,
        test_case.expected_faults,
    ), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        UndecodableDeclarationNameTestCase(
            description="an enum file whose name is not UTF-8",
            relative_path=b"sql/enums/st\xe9.sql",
            contents=_ORDER_STATUS,
            expected_path="sql/enums/st\\xe9.sql",
        ),
        UndecodableDeclarationNameTestCase(
            description="a scoped enum directory whose name is not UTF-8",
            relative_path=b"sql/models/a\xe8/_enums/y.sql",
            contents=_TIER,
            expected_path="sql/models/a\\xe8/_enums/y.sql",
        ),
        UndecodableDeclarationNameTestCase(
            description="a SQL hook whose name is not UTF-8",
            relative_path=b"sql/hooks/sql/refresh_\xff.sql",
            contents=b"HOOK ();\nSELECT 1\n",
            expected_path="sql/hooks/sql/refresh_\\xff.sql",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_declaration_file_named_with_invalid_utf8_when_discovering_then_d016_names_it(
    test_case: UndecodableDeclarationNameTestCase, tmp_path: Path
) -> None:
    write_project(
        project_dir=tmp_path,
        files=(
            ("sql/models/orders.sql", _MODEL),
            (os.fsdecode(test_case.relative_path), test_case.contents),
        ),
    )

    with pytest.raises(ProjectPathError) as raised:
        discover_project_inputs(project_dir=tmp_path)

    assert (raised.value.code, str(raised.value)) == (
        "D016",
        f"Project path {tmp_path / test_case.expected_path}{_RENAME}",
    ), test_case.description


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
