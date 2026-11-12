from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    ContractValidationResult,
    DynamicColumnContractProof,
    DynamicColumnFamilyProof,
    InferredNullability,
    SchemaDynamicColumnFamily,
    SourceLocation,
    evaluate_model_contracts,
)
from sqlbuild.adapter.contract.types import TypeDialect
from tests.unit.src.sqlbuild.compiler.contracts._test_types import (
    ContractLocationTestCase,
    ContractMissingDeclarationsTestCase,
    ContractValidationTestCase,
    DynamicContractValidationTestCase,
)
from tests.unit.src.sqlbuild.compiler.contracts.helpers import make_contract_project


@pytest.mark.parametrize(
    "test_case",
    (
        ContractValidationTestCase(
            description="declared untyped column exists",
            declared_columns=(("order_id", None),),
            inferred_columns=(("order_id", None),),
            type_enforcement=None,
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
        ContractValidationTestCase(
            description="declared untyped column is missing",
            declared_columns=(("customer_id", None),),
            inferred_columns=(("order_id", None),),
            type_enforcement=None,
            expected_codes=("K001",),
            expected_severities=("error",),
            expected_messages=(
                "declared column 'customer_id' was not found in statically inferred output "
                "for model 'orders'",
            ),
        ),
        ContractValidationTestCase(
            description="explicit mode treats unspecified columns as metadata",
            declared_columns=(("customer_id", None),),
            inferred_columns=(("order_id", None),),
            type_enforcement=None,
            check_declared_columns=False,
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
        ContractValidationTestCase(
            description="contract none overrides implicit mode",
            declared_columns=(("customer_id", None),),
            inferred_columns=(("order_id", None),),
            type_enforcement=None,
            contract="none",
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
        ContractValidationTestCase(
            description="enforced contract overrides explicit mode",
            declared_columns=(("customer_id", None),),
            inferred_columns=(),
            type_enforcement=None,
            contract="enforced",
            check_declared_columns=False,
            expected_codes=("K001",),
            expected_severities=("error",),
            expected_messages=(
                "enforced contract column 'customer_id' was not found in statically inferred "
                "output for model 'orders'",
            ),
        ),
        ContractValidationTestCase(
            description="unresolved star does not prove an enforced contract column is missing",
            declared_columns=(("customer_id", None),),
            inferred_columns=(),
            type_enforcement=None,
            contract="enforced",
            check_declared_columns=False,
            fast_lineage_has_star=True,
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
        ContractValidationTestCase(
            description="type enforcement remains active in explicit mode",
            declared_columns=(("amount_cents", "INTEGER"),),
            inferred_columns=(("amount_cents", "VARCHAR"),),
            type_enforcement=True,
            check_declared_columns=False,
            expected_codes=("K002",),
            expected_severities=("error",),
            expected_messages=(
                "column 'amount_cents' inferred as VARCHAR but declared type is INTEGER",
            ),
        ),
        ContractValidationTestCase(
            description="type enforcement remains active with contract none",
            declared_columns=(("amount_cents", "INTEGER"),),
            inferred_columns=(("amount_cents", "VARCHAR"),),
            type_enforcement=True,
            contract="none",
            expected_codes=("K002",),
            expected_severities=("error",),
            expected_messages=(
                "column 'amount_cents' inferred as VARCHAR but declared type is INTEGER",
            ),
        ),
        ContractValidationTestCase(
            description="declared typed column matches inferred type",
            declared_columns=(("order_id", "INTEGER"),),
            inferred_columns=(("order_id", "INT"),),
            type_enforcement=True,
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
        ContractValidationTestCase(
            description="typed mismatch with enforcement is error",
            declared_columns=(("amount_cents", "INTEGER"),),
            inferred_columns=(("amount_cents", "VARCHAR"),),
            type_enforcement=True,
            expected_codes=("K002",),
            expected_severities=("error",),
            expected_messages=(
                "column 'amount_cents' inferred as VARCHAR but declared type is INTEGER",
            ),
        ),
        ContractValidationTestCase(
            description="typed mismatch without enforcement is warning",
            declared_columns=(("amount_cents", "INTEGER"),),
            inferred_columns=(("amount_cents", "VARCHAR"),),
            type_enforcement=False,
            expected_codes=("K002",),
            expected_severities=("warning",),
            expected_messages=(
                "column 'amount_cents' inferred as VARCHAR but declared type is INTEGER",
            ),
        ),
        ContractValidationTestCase(
            description="unknown inferred type with enforcement is warning",
            declared_columns=(("amount_cents", "INTEGER"),),
            inferred_columns=(("amount_cents", None),),
            type_enforcement=True,
            expected_codes=("K003",),
            expected_severities=("warning",),
            expected_messages=(
                "column 'amount_cents' type could not be proven against declared INTEGER",
            ),
        ),
        ContractValidationTestCase(
            description="unknown inferred type without enforcement is ignored",
            declared_columns=(("amount_cents", "INTEGER"),),
            inferred_columns=(("amount_cents", None),),
            type_enforcement=False,
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
        ContractValidationTestCase(
            description="extra inferred columns are allowed",
            declared_columns=(("order_id", None),),
            inferred_columns=(("order_id", None), ("extra_column", None)),
            type_enforcement=None,
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
        ContractValidationTestCase(
            description="extra inferred columns fail with enforced contract",
            declared_columns=(("order_id", None),),
            inferred_columns=(("order_id", None), ("extra_column", None)),
            type_enforcement=None,
            contract="enforced",
            expected_codes=("K005",),
            expected_severities=("error",),
            expected_messages=(
                "column 'extra_column' is not declared in enforced contract for model 'orders'",
            ),
        ),
        ContractValidationTestCase(
            description="same columns in different order pass with enforced contract",
            declared_columns=(("order_id", None), ("customer_id", None)),
            inferred_columns=(("customer_id", None), ("order_id", None)),
            type_enforcement=None,
            contract="enforced",
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
        ContractValidationTestCase(
            description="typed mismatch with enforced contract is error without type enforcement",
            declared_columns=(("amount_cents", "INTEGER"),),
            inferred_columns=(("amount_cents", "VARCHAR"),),
            type_enforcement=False,
            contract="enforced",
            expected_codes=("K002",),
            expected_severities=("error",),
            expected_messages=(
                "column 'amount_cents' inferred as VARCHAR but declared type is INTEGER",
            ),
        ),
        ContractValidationTestCase(
            description="not null audit ignores unknown inferred nullability",
            declared_columns=(("order_id", None),),
            not_null_audit_columns=("order_id",),
            inferred_columns=(("order_id", None),),
            inferred_nullability_by_column={"order_id": InferredNullability.UNKNOWN},
            type_enforcement=None,
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
        ContractValidationTestCase(
            description="not null audit does not declare schema nullability",
            declared_columns=(("customer_name", None),),
            not_null_audit_columns=("customer_name",),
            inferred_columns=(("customer_name", None),),
            inferred_nullability_by_column={"customer_name": InferredNullability.NULLABLE},
            type_enforcement=None,
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
        ContractValidationTestCase(
            description="declared nullable false fails when output is proven nullable",
            declared_columns=(("customer_name", None),),
            declared_nullable_by_column={"customer_name": False},
            inferred_columns=(("customer_name", None),),
            inferred_nullability_by_column={"customer_name": InferredNullability.NULLABLE},
            type_enforcement=None,
            expected_codes=("K004",),
            expected_severities=("error",),
            expected_messages=("column 'customer_name' is declared non-null but may be nullable",),
        ),
        ContractValidationTestCase(
            description="declared nullable true does not require non-null output",
            declared_columns=(("customer_name", None),),
            declared_nullable_by_column={"customer_name": True},
            inferred_columns=(("customer_name", None),),
            inferred_nullability_by_column={"customer_name": InferredNullability.NULLABLE},
            type_enforcement=None,
            expected_codes=(),
            expected_severities=(),
            expected_messages=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_compiled_project_when_validating_contracts_then_returns_expected_diagnostics(
    test_case: ContractValidationTestCase,
) -> None:
    result: ContractValidationResult = evaluate_model_contracts(
        project=make_contract_project(
            declared_columns=test_case.declared_columns,
            inferred_columns=test_case.inferred_columns,
            type_enforcement=test_case.type_enforcement,
            not_null_audit_columns=test_case.not_null_audit_columns,
            declared_nullable_by_column=test_case.declared_nullable_by_column,
            inferred_nullability_by_column=test_case.inferred_nullability_by_column,
            fast_lineage_has_star=test_case.fast_lineage_has_star,
            contract=test_case.contract,
            check_declared_columns=test_case.check_declared_columns,
        ),
        dialect=TypeDialect.DUCKDB,
    )

    assert tuple(diagnostic.code for diagnostic in result.diagnostics) == test_case.expected_codes
    assert (
        tuple(str(diagnostic.severity) for diagnostic in result.diagnostics)
        == test_case.expected_severities
    )
    assert (
        tuple(diagnostic.message for diagnostic in result.diagnostics)
        == test_case.expected_messages
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ContractLocationTestCase(
            description="uses declared column location",
            column_name="customer_id",
            path=Path("models/orders.sql"),
            line=4,
            column=5,
            expected_path=Path("models/orders.sql"),
            expected_line=4,
            expected_column=5,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_column_location_when_validating_contracts_then_diagnostic_uses_location(
    test_case: ContractLocationTestCase,
) -> None:
    location: SourceLocation = SourceLocation(
        path=test_case.path,
        line=test_case.line,
        column=test_case.column,
    )
    result: ContractValidationResult = evaluate_model_contracts(
        project=make_contract_project(
            declared_columns=((test_case.column_name, None),),
            inferred_columns=(("order_id", None),),
            type_enforcement=None,
            column_locations={test_case.column_name: location},
        ),
        dialect=TypeDialect.DUCKDB,
    )

    assert len(result.diagnostics) == 1
    assert result.diagnostics[0].location == location
    assert result.diagnostics[0].path == test_case.expected_path
    assert result.diagnostics[0].line == test_case.expected_line
    assert result.diagnostics[0].column == test_case.expected_column


@pytest.mark.parametrize(
    "test_case",
    (
        ContractLocationTestCase(
            description="deferred unexpected column location",
            column_name="amount",
            path=Path("models/orders.sql"),
            line=4,
            column=3,
            expected_path=Path("models/orders.sql"),
            expected_line=4,
            expected_column=3,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_deferred_output_locations_when_contract_fails_then_locates_authored_projection(
    test_case: ContractLocationTestCase,
) -> None:
    result: ContractValidationResult = evaluate_model_contracts(
        project=make_contract_project(
            declared_columns=(("order_id", "INTEGER"),),
            inferred_columns=(("order_id", "INTEGER"), ("amount", "INTEGER")),
            type_enforcement=True,
            contract="enforced",
            authored_sql=(
                "MODEL (description 'Test model.', columns (order_id (type INTEGER)));\n"
                "SELECT\n"
                "  1 AS order_id,\n"
                "  2 AS amount\n"
            ),
        ),
        dialect=TypeDialect.DUCKDB,
    )

    assert len(result.diagnostics) == 1
    assert result.diagnostics[0].code == "K005"
    assert result.diagnostics[0].location is not None
    assert result.diagnostics[0].location.path == test_case.expected_path
    assert result.diagnostics[0].location.line == test_case.expected_line
    assert result.diagnostics[0].location.column == test_case.expected_column


@pytest.mark.parametrize(
    "test_case",
    (
        ContractMissingDeclarationsTestCase(
            description="enforced contract requires declared columns",
            contract="enforced",
            expected_codes=("K006",),
            expected_messages=("model 'orders' has contract enforced but declares no columns",),
        ),
        ContractMissingDeclarationsTestCase(
            description="none contract allows missing declarations",
            contract="none",
            expected_codes=(),
            expected_messages=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_contract_without_declared_columns_when_validating_then_returns_expected_diagnostics(
    test_case: ContractMissingDeclarationsTestCase,
) -> None:
    result: ContractValidationResult = evaluate_model_contracts(
        project=make_contract_project(
            declared_columns=(),
            inferred_columns=(("order_id", None),),
            type_enforcement=None,
            contract=test_case.contract,
        ),
        dialect=TypeDialect.DUCKDB,
    )

    assert tuple(diagnostic.code for diagnostic in result.diagnostics) == test_case.expected_codes
    assert (
        tuple(diagnostic.message for diagnostic in result.diagnostics)
        == test_case.expected_messages
    )


@pytest.mark.parametrize(
    "test_case",
    (
        DynamicContractValidationTestCase(
            description="unproven family rejects open wildcard",
            declared_type="DECIMAL(12,2)",
            output_proven=False,
            inferred_type=None,
            failure_reason="output is not a dynamic pivot boundary",
            expected_code="K011",
            expected_message=(
                "model 'orders' dynamic column contract is not proven: "
                "output is not a dynamic pivot boundary"
            ),
        ),
        DynamicContractValidationTestCase(
            description="incompatible family type reports mismatch",
            declared_type="VARCHAR",
            output_proven=True,
            inferred_type="DECIMAL(12,2)",
            failure_reason=None,
            expected_code="K002",
            expected_message=(
                "dynamic column family 'category_amounts' inferred as DECIMAL(12,2) "
                "but declared type is VARCHAR"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_dynamic_contract_when_validating_then_reports_expected_diagnostic(
    test_case: DynamicContractValidationTestCase,
) -> None:
    family: SchemaDynamicColumnFamily = SchemaDynamicColumnFamily(
        name="category_amounts",
        pivot_column="category",
        value_column="amount",
        aggregate="MAX",
        type=test_case.declared_type,
    )
    result: ContractValidationResult = evaluate_model_contracts(
        project=make_contract_project(
            declared_columns=(("customer_id", "INTEGER"),),
            inferred_columns=(("customer_id", "INTEGER"),),
            type_enforcement=True,
            contract="enforced",
            dynamic_columns=(family,),
            dynamic_column_contract=DynamicColumnContractProof(
                output_proven=test_case.output_proven,
                families=(
                    DynamicColumnFamilyProof(
                        name="category_amounts",
                        inferred_type=test_case.inferred_type,
                    ),
                ),
                failure_reason=test_case.failure_reason,
            ),
        ),
        dialect=TypeDialect.DUCKDB,
    )

    assert tuple(diagnostic.code for diagnostic in result.diagnostics) == (test_case.expected_code,)
    assert result.diagnostics[0].message == test_case.expected_message
