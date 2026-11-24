"""E2E coverage of BigQuery contracts, where every variable-length text type is STRING."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    BigQueryStringContractTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = (
    'name = "shop"\nadapter = "bigquery"\ndefault_target = "dev"\n\n'
    '[targets.dev]\ndatabase = "warehouse"\nschema = "analytics"\n\n'
    "[scopes]\nenforce_placement = false\n"
)
_MODEL: str = (
    'MODEL (\n  description "Order notes.",\n  contract enforced,\n'
    "  columns (note (type {declared_type})),\n);\n\n"
    "SELECT CAST('gift' AS {cast_type}) AS note\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryStringContractTestCase(
            description="a sized VARCHAR fills a STRING of that length",
            declared_type="STRING(10)",
            cast_type="VARCHAR(10)",
            expected_exit_code=0,
        ),
        BigQueryStringContractTestCase(
            description="a STRING fills a STRING",
            declared_type="STRING",
            cast_type="STRING",
            expected_exit_code=0,
        ),
        BigQueryStringContractTestCase(
            description="a different length is still a mismatch",
            declared_type="STRING(10)",
            cast_type="STRING(5)",
            expected_exit_code=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_string_contract_when_compiling_then_text_types_compare_as_string(
    test_case: BigQueryStringContractTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "sql/models/order_notes.sql": _MODEL.format(
                declared_type=test_case.declared_type, cast_type=test_case.cast_type
            ),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile")
    )

    assert compiled.returncode == test_case.expected_exit_code, compiled.stdout + compiled.stderr
