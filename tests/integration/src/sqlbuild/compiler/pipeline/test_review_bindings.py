"""Real compiler regressions for binding boundary review findings."""

import json
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    IdentifierBindingCase,
    NativeCatalogCase,
)
from tests.unit.src.sqlbuild.compiler.sql_analysis.main.helpers import normalize_analysis_sql


@pytest.mark.parametrize(
    "test_case",
    [
        NativeCatalogCase(
            "open USING",
            'SELECT * FROM __source("orders") o JOIN __source("customers") c USING (id)',
        ),
        NativeCatalogCase(
            "open QUALIFY",
            'SELECT o.id, ROW_NUMBER() OVER () AS n FROM __source("orders") o QUALIFY o.quantity > 1',
        ),
        NativeCatalogCase(
            "unregistered relation remains open",
            "SELECT o.id, ROW_NUMBER() OVER () AS n FROM external_orders o QUALIFY o.quantity > 1",
        ),
        NativeCatalogCase(
            "closed QUALIFY",
            'SELECT o.id, ROW_NUMBER() OVER () AS n FROM __source("known") o QUALIFY o.quantity > 1',
            ("B002",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_open_sources_when_validating_clauses_then_only_closed_shapes_prove_missing_columns(
    test_case: NativeCatalogCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
    )
    (tmp_path / "sources").mkdir()
    (tmp_path / "sources/orders.yml").write_text(
        "sources:\n  - name: orders\n    description: Test source orders.\n    table: orders\n  - name: customers\n    description: Test source customers.\n    table: customers\n"
        "  - name: known\n    description: Test source known.\n    table: known\n    contract: enforced\n    columns:\n      - name: id\n        type: INTEGER\n"
    )
    (tmp_path / "models").mkdir()
    (tmp_path / "models/report.sql").write_text(
        "MODEL (description 'Test model report.', materialized view);\n" + test_case.sql
    )
    result: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache", "--json"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert tuple(item["code"] for item in payload["diagnostics"]) == test_case.expected_codes
    assert result == int(bool(test_case.expected_codes))


@pytest.mark.parametrize(
    "test_case",
    [
        NativeCatalogCase(
            f"nested markers depth {depth}",
            'SELECT * FROM __table_fn("table_fn__orders")('
            + '__udf("udf__quantity")(' * depth
            + "1"
            + ")" * depth
            + ")",
        )
        for depth in (1, 2, 4, 8, 16, 32)
    ],
    ids=lambda case: case.description,
)
def test_given_nested_interpolations_when_normalizing_then_outer_edits_own_their_arguments(
    test_case: NativeCatalogCase,
) -> None:
    assert normalize_analysis_sql(sql=test_case.sql, dialect="duckdb") == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        NativeCatalogCase(
            "nested function compile",
            'SELECT * FROM __table_fn("table_fn__orders")(__udf("udf__quantity")(1))',
        )
    ],
    ids=lambda case: case.description,
)
def test_given_nested_function_calls_when_compiling_then_native_normalization_does_not_panic(
    test_case: NativeCatalogCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
    )
    functions: Path = tmp_path / "functions/sql"
    functions.mkdir(parents=True)
    (functions / "table_fn__orders.sql").write_text(
        "FUNCTION (description 'Test function table_fn__orders.', arguments (x INTEGER), returns table (id INTEGER)); SELECT x AS id"
    )
    (functions / "udf__quantity.sql").write_text(
        "FUNCTION (description 'Test function udf__quantity.', arguments (x INTEGER), returns INTEGER); x + 1"
    )
    (tmp_path / "models").mkdir()
    (tmp_path / "models/orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized view); " + test_case.sql
    )
    assert main(["--project-dir", str(tmp_path), "compile", "--no-cache", "--json"]) == 0
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert tuple(item["code"] for item in payload["diagnostics"]) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case",
    [
        IdentifierBindingCase("Snowflake exact quoted", "snowflake", '"customer"', '"customer"'),
        IdentifierBindingCase(
            "Snowflake quoted mismatch", "snowflake", '"customer"', '"CUSTOMER"', ("B002",)
        ),
        IdentifierBindingCase(
            "Snowflake unquoted uppercase", "snowflake", "customer", '"CUSTOMER"'
        ),
        IdentifierBindingCase("DuckDB quoted insensitive", "duckdb", '"Customer"', '"CUSTOMER"'),
        IdentifierBindingCase("Postgres quoted exact", "postgres", '"Customer"', '"Customer"'),
        IdentifierBindingCase(
            "Postgres quoted mismatch", "postgres", '"Customer"', '"customer"', ("B002",)
        ),
        IdentifierBindingCase("Postgres unquoted lowercase", "postgres", "Customer", '"customer"'),
        IdentifierBindingCase(
            "Postgres unknown column", "postgres", "customer", "missing", ("B002",)
        ),
        IdentifierBindingCase(
            "MotherDuck quoted insensitive", "motherduck", '"Customer"', '"CUSTOMER"'
        ),
        IdentifierBindingCase(
            "MotherDuck unknown column", "motherduck", "customer", "missing", ("B002",)
        ),
        IdentifierBindingCase(
            "SQL Server unknown column", "sqlserver", "customer", "missing", ("B002",)
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_inferred_output_when_binding_downstream_then_preserves_dialect_identifier_identity(
    test_case: IdentifierBindingCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{test_case.dialect}"\n[rules]\nselect = []\n'
    )
    (tmp_path / "models").mkdir()
    header: str = "MODEL (description 'Test model.', materialized view, database warehouse, schema analytics);\n"
    (tmp_path / "models/orders.sql").write_text(header + f"SELECT 1 AS {test_case.projection}")
    (tmp_path / "models/report.sql").write_text(
        header + f'SELECT {test_case.reference} FROM __ref("orders")'
    )
    result: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache", "--json"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert tuple(item["code"] for item in payload["diagnostics"]) == test_case.expected_codes
    assert result == int(bool(test_case.expected_codes))


@pytest.mark.parametrize(
    "test_case",
    [
        NativeCatalogCase(
            "accepted numeric strings",
            'MODEL (description "Test model.", materialized view, columns (quantity (audits [accepted_values (values ["1", "2"])]))); SELECT 1 AS quantity',
        ),
        NativeCatalogCase(
            "relationship case",
            'MODEL (description "Test model.", materialized view, columns (customer_id (audits [relationships (to __ref("customers"), field customer_id)]))); SELECT 1 AS customer_id',
        ),
        NativeCatalogCase(
            "relationship coercion",
            "MODEL (description 'Test model.', materialized view, columns (customer_id (audits [relationships (to __ref(\"customers\"), field customer_id)]))); SELECT CAST(1 AS BIGINT) AS customer_id",
        ),
        NativeCatalogCase(
            "relationship text against an integer key, which DuckDB cannot compare",
            "MODEL (description 'Test model.', materialized view, columns (customer_id (audits [relationships (to __ref(\"customers\"), field customer_id)]))); SELECT '1' AS customer_id",
            expected_codes=("B301", "B217"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_metadata_predicates_when_compiling_then_allows_dialect_coercion_and_case_resolution(
    test_case: NativeCatalogCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
    )
    (tmp_path / "models").mkdir()
    (tmp_path / "models/orders.sql").write_text(test_case.sql)
    (tmp_path / "models/customers.sql").write_text(
        "MODEL (description 'Test model customers.', materialized view); SELECT 1 AS CUSTOMER_ID"
    )
    result: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache", "--json"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert (result, tuple(item["code"] for item in payload["diagnostics"])) == (
        int(bool(test_case.expected_codes)),
        test_case.expected_codes,
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
