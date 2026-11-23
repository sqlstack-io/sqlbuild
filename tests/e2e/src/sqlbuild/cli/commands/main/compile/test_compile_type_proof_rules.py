"""E2E tests for type-proof rules when SQL analysis is disabled."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    TypeProofRuleCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SKIPPED_NOTE: str = (
    "note: skipped type-proof rules (SQBRCONTRACT105) because SQL analysis is disabled"
)
_PROJECT_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n\n'
    '[rules]\nselect = ["SQBRCONTRACT105"]\n'
)
_ORDERS_MODEL: str = (
    "MODEL (\n"
    '  description "Orders",\n'
    "  contract enforced,\n"
    "  columns (\n"
    "    order_id (type INTEGER, nullable false),\n"
    "    status (type VARCHAR),\n"
    "  ),\n"
    ");\n\n"
    "WITH final AS (\n"
    "  SELECT 1 AS order_id, 'placed' AS status\n"
    "  UNION ALL\n"
    "  SELECT 2 AS order_id, 'shipped' AS status\n"
    ")\n"
    "SELECT order_id, status FROM final\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        TypeProofRuleCompileTestCase(
            description="analysis enabled evaluates type-proof rules",
            project_toml=_PROJECT_TOML,
            extra_args=(),
            expected_returncode=1,
            expected_rule_findings=4,
            expected_note_count=0,
        ),
        TypeProofRuleCompileTestCase(
            description="no-sql-analysis flag skips type-proof rules",
            project_toml=_PROJECT_TOML,
            extra_args=("--no-sql-analysis",),
            expected_returncode=0,
            expected_rule_findings=0,
            expected_note_count=1,
        ),
        TypeProofRuleCompileTestCase(
            description="project setting skips type-proof rules",
            project_toml=_PROJECT_TOML + "\n[settings]\nsql_analysis = false\n",
            extra_args=(),
            expected_returncode=0,
            expected_rule_findings=0,
            expected_note_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_typed_contract_when_compiling_then_type_proof_rules_follow_sql_analysis(
    test_case: TypeProofRuleCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": test_case.project_toml,
            "sql/models/orders.sql": _ORDERS_MODEL,
        },
    )

    text_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--no-cache", *test_case.extra_args),
        project_dir=project_dir,
    )
    json_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json", "--no-cache", *test_case.extra_args),
        project_dir=project_dir,
    )

    text_output: str = text_result.stdout + text_result.stderr
    assert text_result.returncode == test_case.expected_returncode, text_output
    assert text_result.stderr.count(_SKIPPED_NOTE) == test_case.expected_note_count
    assert _SKIPPED_NOTE not in text_result.stdout
    assert text_result.stdout.count("error[SQBRCONTRACT105]") == test_case.expected_rule_findings
    assert text_result.stdout.rstrip().splitlines()[-1].startswith("  Wrote:"), text_output
    payload: dict[str, Any] = json.loads(json_result.stdout)
    rule_codes: list[object] = [diagnostic["code"] for diagnostic in payload["diagnostics"]]
    assert json_result.returncode == test_case.expected_returncode
    assert rule_codes.count("SQBRCONTRACT105") == test_case.expected_rule_findings
    assert json_result.stderr.count(_SKIPPED_NOTE) == test_case.expected_note_count


@pytest.mark.parametrize(
    "test_case",
    (
        TypeProofRuleCompileTestCase(
            description="build without SQL analysis skips type-proof rules",
            project_toml=_PROJECT_TOML,
            extra_args=("--no-sql-analysis",),
            expected_returncode=0,
            expected_rule_findings=0,
            expected_note_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_typed_contract_when_building_without_sql_analysis_then_type_proof_rules_skip(
    test_case: TypeProofRuleCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": test_case.project_toml,
            "sql/models/orders.sql": _ORDERS_MODEL,
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", *test_case.extra_args),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_returncode, output
    assert output.count("SQBRCONTRACT105") == test_case.expected_note_count
    assert output.count(_SKIPPED_NOTE.removeprefix("note: ")) == test_case.expected_note_count
