"""Native contract validation on generated projects and on inputs Python once answered."""

from __future__ import annotations

import random
from pathlib import Path

import pytest

from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.contracts._test_types import (
    FormerlyDeferredContractTestCase,
    GeneratedContractTestCase,
    UnknownDialectContractTestCase,
)
from tests.integration.src.sqlbuild.compiler.contracts.helpers import (
    ContractView,
    NativeContractRecord,
    compiled_contract_project,
    contract_diagnostics,
    diagnostic_view,
    perturbed_project,
    record_native_outcomes,
    with_declared_type,
)
from tests.integration.src.sqlbuild.compiler.helpers import (
    GoldenEntry,
    golden_differences,
    golden_entry,
    golden_name,
    read_golden,
)


@pytest.mark.parametrize(
    "test_case",
    [
        GeneratedContractTestCase(
            description="enforced, implicit, typed, nullability and dynamic contracts",
            seed=20261009,
            variants=60,
            dialects=(None, "duckdb", "postgres", "snowflake", "bigquery", "databricks", "tsql"),
            expected_minimum_native=800,
            expected_minimum_typed_comparisons=300,
            expected_minimum_diagnostics=2000,
            expected_codes=frozenset({"K001", "K002", "K003", "K004", "K005", "K006", "K011"}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_generated_contracts_when_validating_then_every_contract_family_is_answered(
    test_case: GeneratedContractTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    rng: random.Random = random.Random(test_case.seed)
    record: NativeContractRecord = record_native_outcomes(monkeypatch=monkeypatch)
    project_dir: Path = tmp_path / "project"
    base: CompiledProjectView = compiled_contract_project(project_dir=project_dir)
    golden: list[GoldenEntry] = []
    for _ in range(test_case.variants):
        project: CompiledProjectView = perturbed_project(project=base, rng=rng)
        golden.append(
            golden_entry(
                "contracts",
                {
                    str(dialect): diagnostic_view(
                        diagnostics=contract_diagnostics(project=project, dialect=dialect),
                        project_dir=project_dir,
                    )
                    for dialect in test_case.dialects
                },
            )
        )

    assert (
        golden_differences(
            read_golden(golden_name("contracts_generated", test_case.description)), golden
        )
        == []
    )
    assert (
        record.statuses["native"] >= test_case.expected_minimum_native,
        record.statuses["typed_comparisons"] >= test_case.expected_minimum_typed_comparisons,
        record.statuses["native_diagnostics"] >= test_case.expected_minimum_diagnostics,
        set(record.codes) >= test_case.expected_codes,
    ) == (True, True, True, True), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        FormerlyDeferredContractTestCase(
            description="a declared type outside ASCII upper-cases as Python does",
            declared_type="TÉXT",
            dialect="duckdb",
            expected_diagnostics=(
                (
                    "K002",
                    "customer_totals",
                    "total_amount",
                    "column 'total_amount' inferred as DOUBLE but declared type is DECIMAL(18,2)",
                ),
                (
                    "K005",
                    "stg_orders",
                    "customer_id",
                    "column 'customer_id' is not declared in enforced contract for model 'stg_orders'",
                ),
                (
                    "K005",
                    "stg_orders",
                    "status",
                    "column 'status' is not declared in enforced contract for model 'stg_orders'",
                ),
                (
                    "K002",
                    "stg_orders",
                    "order_id",
                    "column 'order_id' inferred as INTEGER but declared type is TÉXT",
                ),
                (
                    "K002",
                    "stg_orders",
                    "amount",
                    "column 'amount' inferred as DOUBLE but declared type is TÉXT",
                ),
            ),
        ),
        FormerlyDeferredContractTestCase(
            description="a shipped dialect without a native type parser",
            declared_type="INTEGER",
            dialect="tsql",
            expected_diagnostics=(
                (
                    "K002",
                    "customer_totals",
                    "total_amount",
                    "column 'total_amount' inferred as DOUBLE but declared type is DECIMAL(18,2)",
                ),
                (
                    "K005",
                    "stg_orders",
                    "customer_id",
                    "column 'customer_id' is not declared in enforced contract for model 'stg_orders'",
                ),
                (
                    "K005",
                    "stg_orders",
                    "status",
                    "column 'status' is not declared in enforced contract for model 'stg_orders'",
                ),
                (
                    "K002",
                    "stg_orders",
                    "amount",
                    "column 'amount' inferred as DOUBLE but declared type is INTEGER",
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_input_python_once_answered_when_validating_then_native_diagnostics_are_returned(
    test_case: FormerlyDeferredContractTestCase, tmp_path: Path
) -> None:
    project: CompiledProjectView = with_declared_type(
        project=compiled_contract_project(project_dir=tmp_path / "project"),
        declared_type=test_case.declared_type,
    )

    diagnostics: ContractView = contract_diagnostics(project=project, dialect=test_case.dialect)

    assert (
        tuple(
            (diagnostic.code, diagnostic.resource_name, diagnostic.column_name, diagnostic.message)
            for diagnostic in diagnostics
        )
        == test_case.expected_diagnostics
    ), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        UnknownDialectContractTestCase(
            description="a dialect no adapter ships raises the wheel's error",
            dialect="mysql",
            expected_error="Unknown dialect: mysql",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_dialect_when_validating_typed_contracts_then_python_error_is_raised(
    test_case: UnknownDialectContractTestCase, tmp_path: Path
) -> None:
    project: CompiledProjectView = compiled_contract_project(project_dir=tmp_path / "project")

    with pytest.raises(ValueError, match=test_case.expected_error):
        _ = contract_diagnostics(project=project, dialect=test_case.dialect)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
