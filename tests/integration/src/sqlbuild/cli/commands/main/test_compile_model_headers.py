"""Real CLI and DuckDB coverage for header columns read natively before the ordered model loop."""

from __future__ import annotations

from pathlib import Path

import duckdb
import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    FirstModelErrorTestCase,
    HeaderColumnAuditBuildTestCase,
    TemplatedHeaderDiagnosticsTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    compile_diagnostics,
    diagnostics_by_model,
    write_header_project,
)

PROJECT_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[vars]\nlabel = "Order amount"\n\n'
    'amount_type = "VARCHAR"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
HOOKED_MODEL: str = (
    "MODEL (description 'Hooked orders.', pre_hooks [inline_sql('SELECT 1')], "
    "columns (id (type INTEGER)));\nSELECT CAST(1 AS INTEGER) AS id\n"
)
MACRO_COLUMN_MODEL: str = (
    "MODEL (description 'Orders.', columns (id (type INTEGER, description '@py.cents(id)')));\n"
    "SELECT CAST(1 AS INTEGER) AS id\n"
)
DUPLICATE_COLUMN_MODEL: str = (
    "MODEL (description 'Orders.', columns (id (type INTEGER), ID (type INTEGER)));\n"
    "SELECT CAST(1 AS INTEGER) AS id\n"
)
NULLABLE_NOT_NULL_MODEL: str = (
    "MODEL (description 'Orders.', columns (id (nullable true, audits [not_null])));\n"
    "SELECT CAST(1 AS INTEGER) AS id\n"
)
UNKNOWN_TEMPLATE_MODEL: str = (
    "MODEL (description 'Orders.', columns (id (type INTEGER, description '${missing}')));\n"
    "SELECT CAST(1 AS INTEGER) AS id\n"
)
CONTRACT_MODEL: str = (
    "MODEL (description 'Orders.', contract enforced, columns (\n"
    "  id (type INTEGER, nullable false, audits [not_null, accepted_values (values [1, 2])]),\n"
    "  amount (type {amount_type}, description {description}),\n"
    "));\n"
    "SELECT CAST(1 AS INTEGER) AS id, CAST(2.0 AS DOUBLE) AS amount\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        FirstModelErrorTestCase(
            description="a macro in a column stops before a later duplicate column",
            models={
                "a_orders": HOOKED_MODEL,
                "b_orders": MACRO_COLUMN_MODEL,
                "c_orders": DUPLICATE_COLUMN_MODEL,
            },
            expected_message="model config field 'columns.id.description' does not allow macros",
        ),
        FirstModelErrorTestCase(
            description="a duplicate column stops before a later macro",
            models={
                "a_orders": HOOKED_MODEL,
                "b_orders": DUPLICATE_COLUMN_MODEL,
                "c_orders": MACRO_COLUMN_MODEL,
            },
            expected_message=(
                "sql/models/b_orders.sql model has duplicate column 'ID' "
                "(column names are case-insensitive)"
            ),
        ),
        FirstModelErrorTestCase(
            description="a nullable not_null column stops before a later duplicate column",
            models={"b_orders": NULLABLE_NOT_NULL_MODEL, "c_orders": DUPLICATE_COLUMN_MODEL},
            expected_message=(
                "sql/models/b_orders.sql column 'id' cannot set nullable = true and audit not_null"
            ),
        ),
        FirstModelErrorTestCase(
            description="a templated column read by Python stops before a later duplicate column",
            models={"b_orders": UNKNOWN_TEMPLATE_MODEL, "c_orders": DUPLICATE_COLUMN_MODEL},
            expected_message="model config references unknown variable 'missing'",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_several_failing_models_when_compiling_then_the_first_in_model_order_is_reported(
    test_case: FirstModelErrorTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    write_header_project(tmp_path, test_case.models, PROJECT_TOML)

    exit_code, diagnostics = compile_diagnostics(tmp_path, capsys)

    assert exit_code == 1
    assert [item["message"] for item in diagnostics] == [test_case.expected_message]


@pytest.mark.parametrize(
    "test_case",
    (
        TemplatedHeaderDiagnosticsTestCase(
            description="a templated column description",
            amount_type="VARCHAR",
            amount_description="'${label}'",
            expected_location={
                "path": "sql/models/a_orders.sql",
                "line": 3,
                "column": 3,
                "end_line": 3,
                "end_column": 9,
            },
        ),
        TemplatedHeaderDiagnosticsTestCase(
            description="a templated column type",
            amount_type="'${amount_type}'",
            amount_description="'Order amount'",
            expected_location={
                "path": "sql/models/a_orders.sql",
                "line": 3,
                "column": 3,
                "end_line": 3,
                "end_column": 9,
            },
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_native_and_templated_header_columns_when_compiling_then_diagnostics_agree(
    test_case: TemplatedHeaderDiagnosticsTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_header_project(
        tmp_path,
        {
            "a_orders": CONTRACT_MODEL.format(amount_type="VARCHAR", description="'Order amount'"),
            "b_orders": CONTRACT_MODEL.format(
                amount_type=test_case.amount_type, description=test_case.amount_description
            ),
        },
        PROJECT_TOML,
    )

    exit_code, diagnostics = compile_diagnostics(tmp_path, capsys)

    by_model: dict[str, str] = diagnostics_by_model(diagnostics)
    assert exit_code == 1
    assert sorted(by_model) == ["a_orders", "b_orders"]
    assert by_model["a_orders"] == by_model["b_orders"]
    assert diagnostics[0]["location"] == test_case.expected_location


@pytest.mark.parametrize(
    "test_case",
    (
        HeaderColumnAuditBuildTestCase(
            description="native column audits pass",
            accepted_values="[1, 2]",
            expected_exit_code=0,
            expected_failed_models=(),
        ),
        HeaderColumnAuditBuildTestCase(
            description="a native accepted_values audit fails the build",
            accepted_values="[2, 3]",
            expected_exit_code=1,
            expected_failed_models=("b_orders",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_native_header_column_audits_when_building_then_duckdb_runs_them(
    test_case: HeaderColumnAuditBuildTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    contract: str = CONTRACT_MODEL.format(amount_type="DOUBLE", description="'Order amount'")
    write_header_project(
        tmp_path,
        {
            "a_orders": contract,
            "b_orders": contract.replace("[1, 2]", test_case.accepted_values),
        },
        PROJECT_TOML,
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "--no-color", "build"])
    output: str = "".join(capsys.readouterr())

    assert exit_code == test_case.expected_exit_code, output
    for name in test_case.expected_failed_models:
        assert f"final audit for '{name}' failed" in output
    with duckdb.connect(str(tmp_path / "orders.duckdb"), read_only=True) as connection:
        assert connection.execute("SELECT id, amount FROM a_orders").fetchall() == [(1, 2.0)]
