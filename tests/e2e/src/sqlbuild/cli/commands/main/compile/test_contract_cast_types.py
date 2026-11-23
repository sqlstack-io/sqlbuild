"""E2E coverage of outer casts that establish a contract type, compared as the dialect does."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    ContractCastTypeTestCase,
    TypedCallContractTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = (
    'name = "shop"\nadapter = "snowflake"\ndefault_target = "dev"\n\n'
    '[targets.dev]\ndatabase = "warehouse"\nschema = "analytics"\n\n'
    '[scopes]\nenforce_placement = false\n\n[rules]\nselect = ["SQBRCONTRACT105"]\n'
)
_STATUS_LABEL_MACRO: str = (
    "MACRO (\n"
    '  description "Map a status code to a label.",\n'
    "  arguments (code VARCHAR),\n"
    "  returns VARCHAR,\n"
    ");\n\n"
    "CASE @code WHEN 'a' THEN 'ACTIVE' WHEN 'c' THEN 'CLOSED' END\n"
)
_ORDERS_MODEL: str = (
    'MODEL (description "Orders.");\n\n'
    "SELECT CAST(1 AS INTEGER) AS order_id, CAST('a' AS VARCHAR) AS code\n"
)
_LABELS_MODEL: str = (
    "MODEL (\n"
    '  description "Order status labels.",\n'
    "  contract enforced,\n"
    "  columns (\n"
    "    order_id (type INTEGER),\n"
    "    status_label (type VARCHAR),\n"
    "  ),\n"
    ");\n\n"
    "SELECT\n"
    "  o.order_id,\n"
    "  {projection} AS status_label\n"
    'FROM __ref("orders") AS o\n'
    "WHERE o.order_id > 0\n"
)

_TYPED_MACRO: str = (
    "MACRO (\n"
    '  description "Label a status code.",\n'
    "  arguments (code VARCHAR),\n"
    "  returns {returns},\n"
    ");\n\n"
    "{body}\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCastTypeTestCase(
            description="a cast to the maximum VARCHAR length is the declared VARCHAR",
            projection="CAST(@sql.status_label(o.code) AS VARCHAR(16777216))",
            expected_exit_code=0,
            expected_fragments=("0 errors",),
        ),
        ContractCastTypeTestCase(
            description="a cast to STRING is the declared VARCHAR",
            projection="CAST(@sql.status_label(o.code) AS STRING)",
            expected_exit_code=0,
            expected_fragments=("0 errors",),
        ),
        ContractCastTypeTestCase(
            description="a narrower cast names both types, points at the cast and asks to change it",
            projection="CAST(@sql.status_label(o.code) AS VARCHAR(10))",
            expected_exit_code=1,
            expected_fragments=(
                'error[SQBRCONTRACT105]: output "status_label" has an outer cast to VARCHAR(10),'
                " which is not its declared contract type VARCHAR",
                "--> sql/models/labels.sql:12:3",
                "help: Change the outer cast to CAST(... AS VARCHAR).",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_outer_cast_on_contract_output_when_compiling_then_types_compare_as_snowflake_does(
    test_case: ContractCastTypeTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "sql/macros/sql/status_label.sql": _STATUS_LABEL_MACRO,
            "sql/models/orders.sql": _ORDERS_MODEL,
            "sql/models/labels.sql": _LABELS_MODEL.format(projection=test_case.projection),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile")
    )
    report: str = compiled.stdout + compiled.stderr

    assert compiled.returncode == test_case.expected_exit_code, report
    assert [fragment in report for fragment in test_case.expected_fragments] == [
        True for _ in test_case.expected_fragments
    ], report


@pytest.mark.parametrize(
    "test_case",
    [
        TypedCallContractTestCase(
            description="a body cast to its returns type establishes the contract type",
            returns="VARCHAR",
            body="CAST(UPPER(@code) AS STRING)",
            projection="@sql.status_label(o.code)",
            expected_exit_code=0,
            expected_fragments=("0 errors",),
        ),
        TypedCallContractTestCase(
            description="a double-colon body cast establishes the contract type",
            returns="VARCHAR",
            body="LEFT(@code, 3)::VARCHAR",
            projection="@sql.status_label(o.code)",
            expected_exit_code=0,
            expected_fragments=("0 errors",),
        ),
        TypedCallContractTestCase(
            description="a body without a cast is flagged",
            returns="VARCHAR",
            body="'ACTIVE'",
            projection="@sql.status_label(o.code)",
            expected_exit_code=1,
            expected_fragments=(
                'error[SQBRCONTRACT105]: output "status_label" is a call to SQL macro '
                "status_label(), which declares returns VARCHAR, but its body isn't cast to it",
                "--> sql/models/labels.sql:12:3",
                "help: Cast the result in the macro or at the call",
            ),
        ),
        TypedCallContractTestCase(
            description="a body cast to another type than the contract is flagged",
            returns="VARCHAR",
            body="CAST(@code AS VARCHAR(6))",
            projection="@sql.status_label(o.code)",
            expected_exit_code=1,
            expected_fragments=(
                'output "status_label" has an outer cast to VARCHAR(6), which is not its declared '
                "contract type VARCHAR",
            ),
        ),
        TypedCallContractTestCase(
            description="a returns TYPE OF call is flagged",
            returns="TYPE OF code",
            body="LEFT(@code, 3)",
            projection="@sql.status_label(o.code)",
            expected_exit_code=1,
            expected_fragments=("whose returns TYPE OF does not set the column's physical type",),
        ),
        TypedCallContractTestCase(
            description="a returns TYPE OF call with an outer cast is accepted",
            returns="TYPE OF code",
            body="LEFT(@code, 3)",
            projection="CAST(@sql.status_label(o.code) AS VARCHAR)",
            expected_exit_code=0,
            expected_fragments=("0 errors",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_typed_macro_call_on_contract_output_when_compiling_then_only_a_body_cast_counts(
    test_case: TypedCallContractTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "sql/macros/sql/status_label.sql": _TYPED_MACRO.format(
                returns=test_case.returns, body=test_case.body
            ),
            "sql/models/orders.sql": _ORDERS_MODEL,
            "sql/models/labels.sql": _LABELS_MODEL.format(projection=test_case.projection),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile")
    )
    report: str = compiled.stdout + compiled.stderr

    assert compiled.returncode == test_case.expected_exit_code, report
    assert [fragment in report for fragment in test_case.expected_fragments] == [
        True for _ in test_case.expected_fragments
    ], report
