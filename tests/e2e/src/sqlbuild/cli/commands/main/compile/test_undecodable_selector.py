"""CLI e2e coverage: a selector with undecodable bytes reports S007 in text and JSON mode."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    UndecodableSelectorTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_UNDECODABLE_SELECTOR: str = os.fsdecode(b"orders\xff")


@pytest.mark.parametrize(
    "test_case",
    (
        UndecodableSelectorTestCase(
            description="text output",
            output_flags=(),
            expected_code="S007",
            expected_escaped_selector="orders\\xff",
        ),
        UndecodableSelectorTestCase(
            description="json output",
            output_flags=("--json",),
            expected_code="S007",
            expected_escaped_selector="orders\\xff",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_undecodable_selector_when_compiling_then_unknown_selector_error_is_printed(
    test_case: UndecodableSelectorTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="selectors",
        repo_files={
            "sqlbuild_project.toml": 'name = "selectors"\nadapter = "duckdb"\n',
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.', materialized view);\n\n"
                "SELECT 1 AS order_id\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "compile",
            *test_case.output_flags,
            "--select",
            _UNDECODABLE_SELECTOR,
        ),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert "UnicodeEncodeError" not in result.stderr
    assert test_case.expected_code in result.stdout + result.stderr
    assert test_case.expected_escaped_selector in result.stdout + result.stderr
