"""Failure messages show authored text as written and escape only raw path names."""

from __future__ import annotations

import os
from pathlib import Path

import pytest

from sqlbuild._native import DiscoveryFileFault
from tests.integration.src.sqlbuild.compiler.discovery._test_types import (
    FailureTextTestCase,
    TolerantFailureTextTestCase,
)
from tests.integration.src.sqlbuild.compiler.discovery.helpers import (
    stage_outcome,
    tolerant_declaration_outcome,
    write_project,
)

_MODEL: tuple[str, bytes] = ("sql/models/orders.sql", b"MODEL ();\nSELECT 1 AS order_id")
_HOOK: tuple[str, bytes] = ("sql/hooks/sql/refresh.sql", b"HOOK (descri\x00ption 'x');\nSELECT 1")
_CONSTANT: tuple[str, bytes] = ("sql/constants/limits.sql", b"CONSTANT (name cap, valu\x00e 1);")


@pytest.mark.parametrize(
    "test_case",
    [
        FailureTextTestCase(
            description="a NUL in a model header key",
            files=(("sql/models/orders.sql", b"MODEL (descri\x00ption 'x');\nSELECT 1"),),
            expected_error_type="ModelSqlParseError",
            expected_message_suffix="has unsupported keys: descri\x00ption",
        ),
        FailureTextTestCase(
            description="a NUL in a SQL hook header key",
            files=(_MODEL, _HOOK),
            expected_error_type="SqlHookParseError",
            expected_message_suffix="has unsupported keys: descri\x00ption",
        ),
        FailureTextTestCase(
            description="a NUL in a constant key",
            files=(_MODEL, _CONSTANT),
            expected_error_type="DeclarationParseError",
            expected_message_suffix="constant has unknown keys: valu\x00e",
        ),
        FailureTextTestCase(
            description="a NUL in an audit header key",
            files=(
                _MODEL,
                ("sql/audits/generic/positive.sql", b"AUDIT (sever\x00ity warn);\nSELECT 1"),
            ),
            expected_error_type="SqlAuditParseError",
            expected_message_suffix="has unsupported keys: sever\x00ity",
        ),
        FailureTextTestCase(
            description="a NUL in a SQL function header key",
            files=(
                _MODEL,
                (
                    "sql/functions/sql/cents.sql",
                    b'FUNCTION (arguments (), returns INTEGER, descri\x00ption "x");\nSELECT 1',
                ),
            ),
            expected_error_type="ModelSqlParseError",
            expected_message_suffix="has unsupported keys: descri\x00ption",
        ),
        FailureTextTestCase(
            description="a raw entry name in a declaration group",
            files=(_MODEL, (os.fsdecode(b"sql/models/orders/_sqlbuild/caf\xe9.txt"), b"x")),
            expected_error_type="DeclarationParseError",
            expected_message_suffix="unsupported entries: sql/models/orders/_sqlbuild/caf\\xe9.txt",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_project_when_discovering_then_authored_text_is_shown(
    test_case: FailureTextTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    write_project(project_dir=tmp_path, files=test_case.files)

    outcome: tuple[object, ...] = stage_outcome(project_dir=tmp_path)

    assert outcome[1] == test_case.expected_error_type
    assert str(outcome[2]).endswith(test_case.expected_message_suffix)


@pytest.mark.parametrize(
    "test_case",
    [
        TolerantFailureTextTestCase(
            description="NUL header keys are faulted with their authored text",
            files=(_MODEL, _HOOK, _CONSTANT),
            expected_fault_keys=("descri\x00ption", "valu\x00e"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_nul_header_keys_when_discovering_tolerantly_then_faults_match(
    test_case: TolerantFailureTextTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    write_project(project_dir=tmp_path, files=test_case.files)
    native: tuple[object, tuple[DiscoveryFileFault, ...], tuple[DiscoveryFileFault, ...]] = (
        tolerant_declaration_outcome(project_dir=tmp_path)
    )

    assert (
        tuple(sorted(fault.message.rsplit("keys: ", 1)[1] for fault in (*native[1], *native[2])))
        == test_case.expected_fault_keys
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
