"""Integration coverage for formatting SQL that remains compiler-compatible."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    AuthoredSpellingFormatIntegrationTestCase,
    BacktickDialectFormatIntegrationTestCase,
    CanonicalFixtureFormatIntegrationTestCase,
    DescriptionFormatIntegrationTestCase,
    DollarQuoteFormatIntegrationTestCase,
    FormatCompileIntegrationTestCase,
    FormatDescriptionFaultIntegrationTestCase,
    FormatPathArgumentsIntegrationTestCase,
    FormatSafetyIntegrationTestCase,
    FormatScopeIntegrationTestCase,
    FormatterDeclineIntegrationTestCase,
    FromValuesFormatIntegrationTestCase,
    LayoutOnlyFormatIntegrationTestCase,
    LeadingCteCommentFormatIntegrationTestCase,
    LineWidthWrapIntegrationTestCase,
    MixedFromValuesFormatIntegrationTestCase,
    TrailingCommentFormatIntegrationTestCase,
    TypedNullFormatIntegrationTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    query_duckdb_rows,
    run_build,
    write_from_values_format_project,
    write_snowflake_format_test,
)

_UNFORMATTED_ORDERS_SQL: str = (
    'MODEL (description "Test model.", materialized table);\nselect   1 as order_id\n'
)
_LAYOUT_ONLY_BODY: str = 'SELECT ALL\n  order_id id,\n  amount,\nFROM __source("raw_orders") o;\n'


@pytest.mark.parametrize(
    "test_case",
    [
        AuthoredSpellingFormatIntegrationTestCase(
            "Snowflake STARTSWITH is not renamed",
            "snowflake",
            "STARTSWITH(order_code, 'EU')",
            "STARTSWITH(order_code, 'EU')",
        ),
        AuthoredSpellingFormatIntegrationTestCase(
            "Snowflake SUBSTR synonym is kept",
            "snowflake",
            "SUBSTR(order_code, 1, 2)",
            "SUBSTR(order_code, 1, 2)",
        ),
        AuthoredSpellingFormatIntegrationTestCase(
            "Snowflake TIMESTAMPDIFF synonym is kept",
            "snowflake",
            "TIMESTAMPDIFF(day, ordered_at, shipped_at)",
            "TIMESTAMPDIFF(day, ordered_at, shipped_at)",
        ),
        AuthoredSpellingFormatIntegrationTestCase(
            "Snowflake TRY_TO_DECIMAL synonym is kept",
            "snowflake",
            "TRY_TO_DECIMAL(amount_text, 10, 2)",
            "TRY_TO_DECIMAL(amount_text, 10, 2)",
        ),
        AuthoredSpellingFormatIntegrationTestCase(
            "PostgreSQL comma SUBSTRING arguments are kept",
            "postgres",
            "substring(order_code, 1, 2)",
            "SUBSTRING(order_code, 1, 2)",
        ),
        AuthoredSpellingFormatIntegrationTestCase(
            "DuckDB IFNULL and != spellings are kept",
            "duckdb",
            "IFNULL(status, 'open') != 'x'",
            "IFNULL(status, 'open') != 'x'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_authored_function_spelling_when_formatting_then_only_layout_changes(
    test_case: AuthoredSpellingFormatIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Formatting must never rewrite a function name, including to one the warehouse rejects."""
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{test_case.adapter}"\n'
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Order flags");\n'
        f'select   {test_case.authored_expression} as order_flag from __source("raw_orders")\n'
    )
    assert main(["--project-dir", str(tmp_path), "format", "--json"]) == 0
    capsys.readouterr()
    formatted: str = model.read_text()
    assert f"SELECT {test_case.expected_expression} AS order_flag\n" in formatted
    assert main(["--project-dir", str(tmp_path), "format", "--check", "--json"]) == 0
    assert model.read_text() == formatted


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCompileIntegrationTestCase(
            "commented lateral alias", "LATERAL FLATTEN(input => parsed.items, outer => TRUE) f"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_commented_lateral_alias_when_formatting_then_canonical_sql_is_written(
    test_case: FormatCompileIntegrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """A comment plus generated AS used to decline the entire larger query silently."""
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "snowflake"\n')
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    columns: str = ", ".join(f"parsed.order_attribute_{index}" for index in range(47))
    model.write_text(
        'MODEL (description "Order items");\n'
        "WITH parsed AS (SELECT SPLIT(REGEXP_REPLACE(order_text, '[ ]+', ''), ',') AS items, order_id FROM orders "
        "UNION ALL BY NAME SELECT items, order_id FROM customers), "
        "distinct_orders AS (SELECT DISTINCT items, order_id FROM parsed), final AS (\n"
        "-- Expand the order items.\n"
        f"select f.index, f.value, {columns} FROM parsed, LATERAL FLATTEN(input => parsed.items, outer => TRUE) f) "
        "SELECT * FROM final\n"
    )
    assert main(["--project-dir", str(tmp_path), "format", "--check", "--json"]) == 1
    capsys.readouterr()
    assert main(["--project-dir", str(tmp_path), "format", "--json"]) == 0
    capsys.readouterr()
    formatted: str = model.read_text()
    assert "    f.index,\n    f.value,\n    parsed.order_attribute_0," in formatted
    assert "-- Expand the order items." in formatted
    assert test_case.expected_literal in formatted
    assert main(["--project-dir", str(tmp_path), "format", "--check", "--json"]) == 0
    assert model.read_text() == formatted


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCompileIntegrationTestCase(
            description="canonical empty input fixture remains compiler-compatible",
            expected_literal="SELECT * FROM __empty_fixture()",
        ),
        FormatCompileIntegrationTestCase(
            description="contracted star model accepts the canonical empty fixture",
            expected_literal="SELECT * FROM __empty_fixture()",
            upstream_sql=(
                "WITH staged AS (\n"
                "  SELECT CAST(1 AS INTEGER) AS order_id, CAST('new' AS VARCHAR) AS order_label\n"
                ")\n"
                "SELECT * FROM staged\n"
            ),
            selected_rules="[]",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_typed_null_input_when_fixture_formatting_then_canonical_empty_fixture_compiles(
    test_case: FormatCompileIntegrationTestCase,
    tmp_path: Path,
) -> None:
    """Prove formatter output remains valid with uncontrolled-star lint selected."""
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = {test_case.selected_rules}\n'
        '\n[connection]\ndatabase = "orders.duckdb"\n',
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "stg_orders.sql").write_text(
        "MODEL (\n"
        '  description "Staged orders",\n'
        "  contract enforced,\n"
        "  columns (\n"
        "    order_id (type INTEGER, nullable true),\n"
        "    order_label (type VARCHAR, nullable true),\n"
        "  ),\n"
        ");\n\n" + test_case.upstream_sql,
        encoding="utf-8",
    )
    (models / "orders.sql").write_text(
        'MODEL (description "Orders");\n\nSELECT order_id, order_label FROM __ref("stg_orders")\n',
        encoding="utf-8",
    )
    test_file: Path = tmp_path / "tests" / "unit" / "test_orders.sql"
    test_file.parent.mkdir(parents=True)
    test_file.write_text(
        "TEST();\n\n"
        "WITH\n"
        "__ref__stg_orders AS (\n"
        "  SELECT\n"
        "    CAST(NULL AS INTEGER) AS order_id,\n"
        "    CAST(NULL AS VARCHAR) AS order_label\n"
        "  WHERE FALSE\n"
        "),\n"
        "__expected__orders AS (\n"
        "  SELECT CAST(NULL AS INTEGER) AS order_id, CAST(NULL AS VARCHAR) AS order_label\n"
        "  WHERE FALSE\n"
        ")\n"
        "SELECT 1\n",
        encoding="utf-8",
    )

    format_exit: int = main(["--project-dir", str(tmp_path), "format", "--fixtures-only"])
    compile_exit: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache"])
    test_exit: int = main(["--project-dir", str(tmp_path), "test"])

    assert format_exit == 0
    assert compile_exit == 0
    assert test_exit == 0
    assert test_case.expected_literal in test_file.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCompileIntegrationTestCase(
            description="doubled apostrophe remains valid through format and compile",
            expected_literal="'Customer''s order'",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_test_string_when_formatting_then_project_still_compiles(
    test_case: FormatCompileIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders");\nSELECT order_id, order_label FROM __ref("stg_orders")\n',
        encoding="utf-8",
    )
    (model.parent / "stg_orders.sql").write_text(
        'MODEL (description "Staged orders");\n'
        "SELECT 1 AS order_id, 'Customer''s order' AS order_label\n",
        encoding="utf-8",
    )
    test: Path = tmp_path / "tests" / "unit" / "orders_test.sql"
    test.parent.mkdir(parents=True)
    test.write_text(
        "TEST();\n\nWITH __ref__stg_orders AS ("
        "SELECT 1 AS order_id, 'Customer''s order' AS order_label"
        "), __expected__orders AS ("
        "SELECT 1 AS order_id, 'Customer''s order' AS order_label"
        ")\nSELECT 1\n",
        encoding="utf-8",
    )

    format_exit: int = main(["--project-dir", str(tmp_path), "format"])
    compile_exit: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache"])

    assert format_exit == 0
    assert compile_exit == 0
    assert test_case.expected_literal in test.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCompileIntegrationTestCase(
            description="table function arguments survive formatter restoration",
            expected_literal='__table_fn("expand_order")(source_orders.order_id)',
        )
    ],
    ids=lambda case: case.description,
)
def test_given_table_function_argument_when_formatting_then_intrinsic_restores_and_compiles(
    test_case: FormatCompileIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    function: Path = tmp_path / "functions" / "sql" / "expand_order.sql"
    function.parent.mkdir(parents=True)
    function.write_text(
        """FUNCTION (
  description "Test function expand_order.",
  arguments (order_id INTEGER),
  returns table (order_id INTEGER)
);

SELECT order_id
""",
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "expanded_orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Expanded orders");\nselect expanded.order_id from orders as '
        'source_orders cross join __table_fn("expand_order")(source_orders.order_id) as expanded\n',
        encoding="utf-8",
    )

    format_exit: int = main(["--project-dir", str(tmp_path), "format"])
    compile_exit: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache"])

    assert format_exit == 0
    assert compile_exit == 0
    formatted: str = model.read_text(encoding="utf-8")
    assert test_case.expected_literal in formatted


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCompileIntegrationTestCase(
            description="explicit null ordering survives format and compile",
            expected_literal="order_id NULLS LAST",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_explicit_null_ordering_when_formatting_then_clause_is_preserved(
    test_case: FormatCompileIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders");\n'
        'select order_id from __ref("stg_orders") order by order_id nulls last\n',
        encoding="utf-8",
    )
    (model.parent / "stg_orders.sql").write_text(
        'MODEL (description "Staged orders");\nSELECT 1 AS order_id\n',
        encoding="utf-8",
    )

    format_exit: int = main(["--project-dir", str(tmp_path), "format"])
    compile_exit: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache"])

    assert format_exit == 0
    assert compile_exit == 0
    assert test_case.expected_literal in model.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCompileIntegrationTestCase(
            description="exact contract casts retain authored type spellings",
            expected_literal="CAST('one' AS TEXT) AS order_label",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_exact_contract_casts_when_formatting_then_rule_still_passes(
    test_case: FormatCompileIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRCONTRACT105"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (\n"
        '  description "Typed orders",\n'
        "  contract enforced,\n"
        "  columns (\n"
        "    order_label (type TEXT)\n"
        "    order_id (type INTEGER)\n"
        "    observed_at (type TIMESTAMP_NTZ)\n"
        "    amount (type NUMERIC(10, 2))\n"
        "  )\n"
        ");\n\n"
        "select cast('one' as TEXT) as order_label, cast(1 as INTEGER) as order_id, "
        "cast('2026-01-01' as TIMESTAMP_NTZ) as observed_at, "
        "cast(12.50 as NUMERIC(10, 2)) as amount\n",
        encoding="utf-8",
    )

    format_exit: int = main(["--project-dir", str(tmp_path), "format"])
    compile_exit: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache"])

    assert format_exit == 0
    assert compile_exit == 0
    formatted: str = model.read_text(encoding="utf-8")
    assert test_case.expected_literal in formatted


@pytest.mark.parametrize(
    "test_case",
    [
        DescriptionFormatIntegrationTestCase(
            description="configured width wraps and compiles model description",
            line_width=60,
            authored_description=(
                "Builds canonical customer records from every available source while retaining "
                "unmatched customers."
            ),
            expected_formatted_description=(
                'description """\n'
                "    Builds canonical customer records from every available\n"
                "    source while retaining unmatched customers.\n"
                '  """'
            ),
        ),
        DescriptionFormatIntegrationTestCase(
            description="short wrapped description is joined and compiles",
            line_width=100,
            authored_description="Builds order summary.\n    Retains unmatched orders.",
            expected_formatted_description=(
                'description "Builds order summary. Retains unmatched orders."'
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_description_width_when_formatting_then_wrapped_model_compiles(
    test_case: DescriptionFormatIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n[format]\nline_width = {test_case.line_width}\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        f'MODEL (\n  description "{test_case.authored_description}"\n);\nSELECT 1 AS order_id\n',
        encoding="utf-8",
    )

    format_exit: int = main(["--project-dir", str(tmp_path), "format"])
    compile_exit: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache"])

    assert format_exit == 0
    assert compile_exit == 0
    assert test_case.expected_formatted_description in model.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        TrailingCommentFormatIntegrationTestCase(
            description="a trailing line comment on the last line stays settled",
            body="SELECT 1 AS order_id\nFROM orders -- latest orders\n",
            expected_body="SELECT 1 AS order_id\nFROM orders -- latest orders\n",
        ),
        TrailingCommentFormatIntegrationTestCase(
            description="comments after the last clause gain no blank line",
            body="select 1 as order_id from orders\n\n-- first note\n\n-- second note\n",
            expected_body="SELECT 1 AS order_id\nFROM orders -- first note\n-- second note\n",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_trailing_comments_when_formatting_twice_then_second_run_changes_nothing(
    test_case: TrailingCommentFormatIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    header: str = 'MODEL (description "Orders placed by customers.");\n\n'
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(header + test_case.body, encoding="utf-8")

    first_exit: int = main(["--project-dir", str(tmp_path), "format", "--no-cache"])
    formatted_once: str = model.read_text(encoding="utf-8")
    second_exit: int = main(["--project-dir", str(tmp_path), "format", "--no-cache"])
    check_exit: int = main(["--project-dir", str(tmp_path), "format", "--check", "--no-cache"])

    assert (first_exit, second_exit, check_exit) == (0, 0, 0)
    assert formatted_once == header + test_case.expected_body
    assert model.read_text(encoding="utf-8") == formatted_once


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCompileIntegrationTestCase(
            description="schema column remains valid and idempotent",
            expected_literal="SCHEMA (type VARCHAR)",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_schema_column_when_formatting_then_header_remains_valid_and_idempotent(
    test_case: FormatCompileIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "catalog"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "catalog_objects.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (\n"
        '  description "Catalog objects.",\n'
        "  materialized table,\n"
        "  columns (\n"
        "    object_id (type INTEGER),\n"
        "    SCHEMA (type VARCHAR),\n"
        "  ),\n"
        ");\n\n"
        "select 1 as object_id, 'analytics' as \"SCHEMA\"\n",
        encoding="utf-8",
    )

    first_exit: int = main(["--project-dir", str(tmp_path), "format"])
    compile_exit: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache"])
    formatted_once: str = model.read_text(encoding="utf-8")
    second_exit: int = main(["--project-dir", str(tmp_path), "format", "--check"])

    assert first_exit == 0
    assert compile_exit == 0
    assert second_exit == 0
    assert formatted_once == model.read_text(encoding="utf-8")
    assert formatted_once.count(test_case.expected_literal) == 1


@pytest.mark.parametrize(
    "test_case",
    [
        FormatSafetyIntegrationTestCase(
            description="unparseable header is untouched",
            authored_sql="MODEL (description);\nselect 1 as order_id\n",
            expected_fault_code="header-parse",
            expected_exit_code=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unparseable_file_when_formatting_then_file_is_untouched_and_faults(
    test_case: FormatSafetyIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(test_case.authored_sql, encoding="utf-8")

    exit_code: int = main(["--project-dir", str(tmp_path), "format"])

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == test_case.expected_exit_code
    assert test_case.expected_fault_code in captured.out
    assert model.read_text(encoding="utf-8") == test_case.authored_sql


@pytest.mark.parametrize(
    "test_case",
    [
        FormatterDeclineIntegrationTestCase(
            description="unsupported SQL body leaves the whole file unchanged",
            adapter="duckdb",
            authored_body="select from\n",
            expected_exit_code=1,
            expected_code="format-unsafe",
            expected_message="File left unchanged",
        ),
        FormatterDeclineIntegrationTestCase(
            description="a dialect SQLClean does not format yet is left as written with a warning",
            adapter="databricks",
            authored_body="select   order_id from raw_orders\n",
            expected_exit_code=0,
            expected_code="format-unavailable",
            expected_message=(
                "File left as written: SQL formatting is unavailable for the databricks dialect"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_native_formatter_decline_when_formatting_then_whole_file_is_unchanged(
    test_case: FormatterDeclineIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{test_case.adapter}"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (\n"
        '  description "Orders.",    \n'
        "  materialized table,\n"
        ");\n\n"
        f"{test_case.authored_body}",
        encoding="utf-8",
    )

    original: str = model.read_text(encoding="utf-8")
    first_exit: int = main(["--project-dir", str(tmp_path), "format"])
    first_output: str = capsys.readouterr().out
    formatted_once: str = model.read_text(encoding="utf-8")
    second_exit: int = main(["--project-dir", str(tmp_path), "format", "--check"])
    second_output: str = capsys.readouterr().out

    assert first_exit == test_case.expected_exit_code
    assert second_exit == test_case.expected_exit_code
    assert formatted_once == original
    assert test_case.authored_body in formatted_once
    assert test_case.expected_code in first_output
    assert test_case.expected_code in second_output
    assert test_case.expected_message in first_output
    assert "orders.sql" in second_output
    json_exit: int = main(["--project-dir", str(tmp_path), "format", "--check", "--json"])
    json_output: str = capsys.readouterr().out
    assert json_exit == test_case.expected_exit_code
    assert test_case.expected_code in json_output
    assert test_case.expected_message in json_output
    assert json.loads(json_output)
    assert formatted_once == model.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        TypedNullFormatIntegrationTestCase(
            description="inline colon casts reach empty fixture fixed point in one pass",
            fixture_projection=("NULL::VARCHAR AS customer_key, NULL::BIGINT AS order_count"),
            expected_literal="__empty_fixture()",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_inline_typed_null_fixture_when_formatting_then_one_pass_is_idempotent(
    test_case: TypedNullFormatIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "customers"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "customers.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (\n"
        '  description "Customers.",\n'
        "  contract enforced,\n"
        "  columns (\n"
        "    customer_key (type VARCHAR),\n"
        "    order_count (type BIGINT),\n"
        "  ),\n"
        ");\n\n"
        "SELECT 'c1' AS customer_key, 1 AS order_count\n",
        encoding="utf-8",
    )
    test_file: Path = tmp_path / "tests" / "unit" / "test_customers.sql"
    test_file.parent.mkdir(parents=True)
    test_file.write_text(
        "TEST();\n\n"
        "WITH __ref__customers AS (\n"
        f"    SELECT {test_case.fixture_projection}\n"
        "    WHERE FALSE\n"
        "),\n"
        "__expected__customers AS (\n"
        "    SELECT 'c1' AS customer_key, 1 AS order_count\n"
        ")\n"
        "SELECT 1\n",
        encoding="utf-8",
    )

    first_exit: int = main(["--project-dir", str(tmp_path), "format"])
    formatted_once: str = test_file.read_text(encoding="utf-8")
    second_exit: int = main(["--project-dir", str(tmp_path), "format", "--check"])

    assert first_exit == test_case.expected_exit_code
    assert second_exit == test_case.expected_exit_code
    assert test_case.expected_literal in formatted_once
    assert "CAST(NULL" not in formatted_once
    assert formatted_once == test_file.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        CanonicalFixtureFormatIntegrationTestCase(
            description="post-native fixture simplification reaches fixed point in one pass",
            expected_retained_literal="COLUMN2::VARCHAR AS CLUSTER_ID",
            expected_removed_literal="is_archived",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_redundant_typed_null_after_multi_projection_when_formatting_then_is_idempotent(
    test_case: CanonicalFixtureFormatIntegrationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "customer_segments"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "customer_clusters.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (\n"
        '  description "Customer clusters.",\n'
        "  contract enforced,\n"
        "  columns (\n"
        "    customer_key (type VARCHAR),\n"
        "    cluster_id (type VARCHAR),\n"
        "    is_archived (type BOOLEAN),\n"
        "  ),\n"
        ");\n\n"
        "SELECT 'c1' AS customer_key, 'k1' AS cluster_id, FALSE AS is_archived\n",
        encoding="utf-8",
    )
    test_file: Path = tmp_path / "tests" / "unit" / "test_customer_clusters.sql"
    test_file.parent.mkdir(parents=True)
    test_file.write_text(
        "TEST();\n\n"
        "WITH __ref__customer_clusters AS (\n"
        "    SELECT COLUMN1::VARCHAR AS CUSTOMER_KEY, COLUMN2::VARCHAR AS CLUSTER_ID,\n"
        "        NULL::BOOLEAN AS is_archived\n"
        "    FROM VALUES\n"
        "        ('c1', 'k1')\n"
        "),\n"
        "__expected__customer_clusters AS (\n"
        "    SELECT 'c1' AS customer_key, 'k1' AS cluster_id, FALSE AS is_archived\n"
        ")\n"
        "SELECT 1\n",
        encoding="utf-8",
    )

    first_exit: int = main(["--project-dir", str(tmp_path), "format"])
    formatted_once: str = test_file.read_text(encoding="utf-8")
    second_exit: int = main(["--project-dir", str(tmp_path), "format", "--check"])
    fixture_section: str = formatted_once.split("__expected__customer_clusters", maxsplit=1)[0]

    assert first_exit == test_case.expected_exit_code
    assert second_exit == test_case.expected_exit_code
    assert test_case.expected_retained_literal in fixture_section
    assert test_case.expected_removed_literal not in fixture_section
    assert formatted_once == test_file.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        FromValuesFormatIntegrationTestCase(
            description="snowflake preserves unparenthesized values relation",
            adapter="snowflake",
            expected_exit_code=0,
            expected_literal="FROM VALUES",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_from_values_when_formatting_then_form_is_preserved(
    test_case: FromValuesFormatIntegrationTestCase,
    tmp_path: Path,
) -> None:
    project_dir, test_file = write_from_values_format_project(
        tmp_path=tmp_path, adapter=test_case.adapter
    )

    first_exit: int = main(["--project-dir", str(project_dir), "format"])
    formatted_once: str = test_file.read_text(encoding="utf-8")
    second_exit: int = main(["--project-dir", str(project_dir), "format", "--check"])

    assert first_exit == test_case.expected_exit_code
    assert second_exit == test_case.expected_exit_code
    assert test_case.expected_literal in formatted_once
    assert "FROM (VALUES" not in formatted_once
    assert formatted_once == test_file.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        FromValuesFormatIntegrationTestCase(
            description="duckdb values relation remains valid and idempotent",
            adapter="duckdb",
            expected_exit_code=0,
            expected_literal="FROM VALUES",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_duckdb_from_values_when_formatting_then_project_compiles(
    test_case: FromValuesFormatIntegrationTestCase,
    tmp_path: Path,
) -> None:
    project_dir, test_file = write_from_values_format_project(
        tmp_path=tmp_path, adapter=test_case.adapter
    )

    first_exit: int = main(["--project-dir", str(project_dir), "format"])
    compile_exit: int = main(["--project-dir", str(project_dir), "compile", "--no-cache"])
    formatted_once: str = test_file.read_text(encoding="utf-8")
    second_exit: int = main(["--project-dir", str(project_dir), "format", "--check"])

    assert first_exit == test_case.expected_exit_code
    assert compile_exit == test_case.expected_exit_code
    assert second_exit == test_case.expected_exit_code
    assert test_case.expected_literal in formatted_once
    assert formatted_once == test_file.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        MixedFromValuesFormatIntegrationTestCase(
            description="mixed values forms retain positional authorship through real CLI",
            authored_query=(
                'TEST (name "mixed_product_values");\n\n'
                "-- VALUES inside this comment is not a relation.\n"
                "SELECT 'VALUES' AS label FROM (VALUES (1)) AS first_values(id)\n"
                "UNION ALL\n"
                "SELECT 'second' AS label FROM VALUES (2)\n"
            ),
            expected_parenthesized_literal="FROM (VALUES (1)) AS first_values(id)",
            expected_unparenthesized_literal="FROM VALUES (2)",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_mixed_values_forms_when_formatting_then_each_relation_preserves_authorship(
    test_case: MixedFromValuesFormatIntegrationTestCase,
    tmp_path: Path,
) -> None:
    project_dir, test_file = write_snowflake_format_test(
        tmp_path=tmp_path,
        test_sql=test_case.authored_query,
    )

    first_exit: int = main(["--project-dir", str(project_dir), "format"])
    formatted_once: str = test_file.read_text(encoding="utf-8")
    second_exit: int = main(["--project-dir", str(project_dir), "format", "--check"])

    assert first_exit == test_case.expected_exit_code
    assert second_exit == test_case.expected_exit_code
    assert test_case.expected_parenthesized_literal in formatted_once
    assert test_case.expected_unparenthesized_literal in formatted_once
    assert formatted_once == test_file.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        FormatScopeIntegrationTestCase(
            description="default exclusions and path selectors share format scope",
            expected_exclude_exit=1,
            expected_selected_model_exit=0,
            expected_path_with_exclude_exit=1,
            expected_exclude_path_exit=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_format_selectors_when_scoping_then_paths_and_default_exclusions_are_consistent(
    test_case: FormatScopeIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "customers.sql").write_text(
        'MODEL (description "Customers.", materialized table, '
        "columns (customer_id (type INTEGER),));\n"
        "SELECT 1 AS customer_id\n",
        encoding="utf-8",
    )
    (models / "orders.sql").write_text(
        'MODEL (description "Orders.", materialized table, '
        "columns (order_id (type INTEGER),));\n"
        "SELECT customer_id AS order_id\n"
        'FROM __ref("customers")\n',
        encoding="utf-8",
    )
    test_file: Path = tmp_path / "tests" / "unit" / "test_orders.sql"
    test_file.parent.mkdir(parents=True)
    unformatted_test: str = (
        'TEST (name "orders_returns_one_row");\n\n'
        "WITH __ref__customers AS (\n    SELECT 1 AS customer_id\n),\n"
        "__expected__orders AS (\n    SELECT 1 AS order_id\n)\nSELECT 1\n"
    )
    test_file.write_text(unformatted_test, encoding="utf-8")

    exclude_exit: int = main(
        ["--project-dir", str(tmp_path), "format", "--check", "--exclude", "customers"]
    )
    exclude_output: str = capsys.readouterr().err
    selected_model_exit: int = main(
        ["--project-dir", str(tmp_path), "format", "--check", "--select", "orders"]
    )
    selected_model_output: str = capsys.readouterr().err
    path_with_exclude_exit: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "format",
            "--check",
            "--select",
            "path:tests",
            "--exclude",
            "orders",
        ]
    )
    path_with_exclude_output: str = capsys.readouterr().err
    exclude_path_exit: int = main(
        ["--project-dir", str(tmp_path), "format", "--check", "--exclude", "path:tests"]
    )
    exclude_path_output: str = capsys.readouterr().err

    assert exclude_exit == test_case.expected_exclude_exit
    assert "2 files checked" in exclude_output
    assert selected_model_exit == test_case.expected_selected_model_exit
    assert "1 files checked" in selected_model_output
    assert path_with_exclude_exit == test_case.expected_path_with_exclude_exit
    assert "1 files checked" in path_with_exclude_output
    assert exclude_path_exit == test_case.expected_exclude_path_exit
    assert "2 files checked" in exclude_path_output
    assert test_file.read_text(encoding="utf-8") == unformatted_test


@pytest.mark.parametrize(
    "test_case",
    [
        FormatDescriptionFaultIntegrationTestCase(
            description="missing model description fails check and write modes",
            authored_sql="MODEL (materialized table);\nselect 1 as order_id\n",
            expected_exit_code=1,
            expected_code="description-present",
            expected_severity="fault",
            expected_formatted_sql="MODEL (materialized table);\nSELECT 1 AS order_id\n",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_description_when_formatting_then_check_and_write_fault(
    test_case: FormatDescriptionFaultIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(test_case.authored_sql, encoding="utf-8")

    text_exit: int = main(["--project-dir", str(tmp_path), "format", "--check"])
    text_output: str = capsys.readouterr().out
    json_exit: int = main(["--project-dir", str(tmp_path), "format", "--check", "--json"])
    payload: dict[str, object] = json.loads(capsys.readouterr().out)

    assert text_exit == test_case.expected_exit_code
    assert f"error[{test_case.expected_code}]" in text_output
    assert json_exit == test_case.expected_exit_code
    assert payload["faults"] == 1
    assert payload["warnings"] == 0
    violations: object = payload["violations"]
    assert isinstance(violations, list)
    assert violations[0]["code"] == test_case.expected_code
    assert violations[0]["severity"] == test_case.expected_severity

    write_exit: int = main(["--project-dir", str(tmp_path), "format"])
    write_output: str = capsys.readouterr().out
    assert write_exit == test_case.expected_exit_code
    assert f"error[{test_case.expected_code}]" in write_output
    assert model.read_text(encoding="utf-8") == test_case.expected_formatted_sql


@pytest.mark.parametrize(
    "test_case",
    [
        FormatPathArgumentsIntegrationTestCase(
            description="positional file formats only that file",
            arguments=("models/marts/orders.sql",),
            expected_exit_code=0,
            expected_formatted=("models/marts/orders.sql",),
            expected_unchanged=("models/staging/stg_orders.sql",),
            expected_output_fragment="1 files",
        ),
        FormatPathArgumentsIntegrationTestCase(
            description="bare file path in select formats only that file",
            arguments=("--select", "models/staging/stg_orders.sql"),
            expected_exit_code=0,
            expected_formatted=("models/staging/stg_orders.sql",),
            expected_unchanged=("models/marts/orders.sql",),
            expected_output_fragment="1 files",
        ),
        FormatPathArgumentsIntegrationTestCase(
            description="positional folder formats every file below it",
            arguments=("models",),
            expected_exit_code=0,
            expected_formatted=("models/marts/orders.sql", "models/staging/stg_orders.sql"),
            expected_unchanged=(),
            expected_output_fragment="2 files",
        ),
        FormatPathArgumentsIntegrationTestCase(
            description="missing positional path is a clear error",
            arguments=("models/marts/missing.sql",),
            expected_exit_code=1,
            expected_formatted=(),
            expected_unchanged=("models/marts/orders.sql", "models/staging/stg_orders.sql"),
            expected_output_fragment="format path 'models/marts/missing.sql' does not exist",
        ),
        FormatPathArgumentsIntegrationTestCase(
            description="path outside formatted folders lists the valid roots",
            arguments=("sqlbuild_project.toml",),
            expected_exit_code=1,
            expected_formatted=(),
            expected_unchanged=("models/marts/orders.sql", "models/staging/stg_orders.sql"),
            expected_output_fragment="use a path under models/, tests/",
        ),
        FormatPathArgumentsIntegrationTestCase(
            description="unknown path selector root lists the formatter roots",
            arguments=("--select", "path:seeds"),
            expected_exit_code=1,
            expected_formatted=(),
            expected_unchanged=("models/marts/orders.sql", "models/staging/stg_orders.sql"),
            expected_output_fragment="'functions/'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_file_paths_when_formatting_then_only_named_files_change(
    test_case: FormatPathArgumentsIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    for relative_path in ("models/marts/orders.sql", "models/staging/stg_orders.sql"):
        (tmp_path / relative_path).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / relative_path).write_text(_UNFORMATTED_ORDERS_SQL, encoding="utf-8")
    monkeypatch.chdir(tmp_path)

    exit_code: int = main(["--no-color", "format", *test_case.arguments])
    captured: CaptureResult[str] = capsys.readouterr()

    assert exit_code == test_case.expected_exit_code
    assert test_case.expected_output_fragment in captured.out + captured.err
    assert {
        path: (tmp_path / path).read_text(encoding="utf-8") != _UNFORMATTED_ORDERS_SQL
        for path in (*test_case.expected_formatted, *test_case.expected_unchanged)
    } == {
        **dict.fromkeys(test_case.expected_formatted, True),
        **dict.fromkeys(test_case.expected_unchanged, False),
    }


@pytest.mark.parametrize(
    "test_case",
    [
        LeadingCteCommentFormatIntegrationTestCase(
            description="comment block before a middle CTE stays above it and compiles",
            authored_sql=(
                'MODEL (description "Test model.", materialized view);\n\n'
                'WITH\nupstream AS (\n  SELECT *\n  FROM __ref("raw_orders")\n),\n\n'
                "-- Explains the next CTE: line one,\n-- line two.\n"
                "distinct_rows AS (\n    SELECT order_id, customer_id\n    FROM upstream\n"
                "    GROUP BY ALL\n)\n\nSELECT order_id, customer_id FROM distinct_rows\n"
            ),
            expected_fragment="),\n\n-- Explains the next CTE: line one,\n-- line two.\ndistinct_rows AS (",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_leading_cte_comment_when_formatting_then_comment_rule_still_passes(
    test_case: LeadingCteCommentFormatIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN033"]\n',
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "raw_orders.sql").write_text(
        'MODEL (description "Test model raw_orders.", materialized view);\n\nSELECT 1 AS order_id, 2 AS customer_id\n',
        encoding="utf-8",
    )
    model_path: Path = models / "distinct_orders.sql"
    model_path.write_text(test_case.authored_sql, encoding="utf-8")

    format_exit: int = main(["--project-dir", str(tmp_path), "--no-color", "format"])
    formatted_sql: str = model_path.read_text(encoding="utf-8")
    second_format_exit: int = main(["--project-dir", str(tmp_path), "--no-color", "format"])
    compile_exit: int = main(["--project-dir", str(tmp_path), "--no-color", "compile"])
    output: CaptureResult[str] = capsys.readouterr()

    assert (format_exit, second_format_exit, compile_exit) == (0, 0, 0), output.out + output.err
    assert test_case.expected_fragment in formatted_sql
    assert model_path.read_text(encoding="utf-8") == formatted_sql
    assert "SQLCLEAN033" not in output.out + output.err


@pytest.mark.parametrize(
    "test_case",
    [
        BacktickDialectFormatIntegrationTestCase(
            description="databricks apostrophe inside a backtick identifier keeps the macro",
            adapter="databricks",
            expected_literal='@py.label("order_id") as order_id',
        ),
        BacktickDialectFormatIntegrationTestCase(
            description="bigquery apostrophe inside a backtick identifier keeps the macro",
            adapter="bigquery",
            expected_literal='@py.label("order_id") AS order_id',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_backtick_identifier_apostrophe_when_formatting_then_macro_call_still_compiles(
    test_case: BacktickDialectFormatIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{test_case.adapter}"\n', encoding="utf-8"
    )
    macros: Path = tmp_path / "models" / "_macros" / "python"
    macros.mkdir(parents=True)
    (macros / "labels.py").write_text(
        'def label(expression: str) -> str:\n    """Return one column expression."""\n'
        "    return expression\n",
        encoding="utf-8",
    )
    model_path: Path = tmp_path / "models" / "orders.sql"
    model_path.write_text(
        'MODEL (description "Orders", database warehouse, schema analytics);\n\n'
        'select   `customer\'s id` as customer_id, @py.label("order_id") as order_id '
        "from raw_orders\n",
        encoding="utf-8",
    )

    format_exit: int = main(["--project-dir", str(tmp_path), "--no-color", "format"])
    compile_exit: int = main(["--project-dir", str(tmp_path), "--no-color", "compile"])
    output: CaptureResult[str] = capsys.readouterr()

    assert (format_exit, compile_exit) == (0, 0), output.out + output.err
    assert test_case.expected_literal in model_path.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        DollarQuoteFormatIntegrationTestCase(
            description="untagged dollar quote with an apostrophe",
            literal="$$Customer's order$$",
            expected_note="Customer's order",
        ),
        DollarQuoteFormatIntegrationTestCase(
            description="tagged dollar quote with an apostrophe",
            literal="$note$it's shipped$note$",
            expected_note="it's shipped",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dollar_quoted_apostrophe_when_formatting_then_reference_survives_and_builds(
    test_case: DollarQuoteFormatIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\ndefault_target = "prod"\n\n'
        '[connections.local]\ndatabase = "warehouse.duckdb"\n\n'
        '[targets.prod]\nconnection = "local"\nschema = "prod"\n',
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "orders.sql").write_text(
        'MODEL (description "Orders", materialized table);\n\nSELECT 1 AS order_id\n',
        encoding="utf-8",
    )
    notes: Path = models / "order_notes.sql"
    notes.write_text(
        'MODEL (description "Order notes", materialized table);\n\n'
        f'select {test_case.literal} as note, o.order_id from __ref("orders") o\n',
        encoding="utf-8",
    )

    format_exit: int = main(["--project-dir", str(tmp_path), "--no-color", "format"])
    build_exit, build_output = run_build(project_dir=tmp_path, flags=(), capsys=capsys)

    assert (format_exit, build_exit) == (0, 0), build_output
    formatted: str = notes.read_text(encoding="utf-8")
    assert f"  {test_case.literal} AS note," in formatted
    assert 'FROM __ref("orders")' in formatted
    assert query_duckdb_rows(
        db_path=tmp_path / "warehouse.duckdb", sql="SELECT note, order_id FROM prod.order_notes"
    ) == ((test_case.expected_note, 1),)


@pytest.mark.parametrize(
    "test_case",
    [
        LayoutOnlyFormatIntegrationTestCase(f"{adapter} keeps authored tokens", adapter, body)
        for adapter, body in (
            ("duckdb", _LAYOUT_ONLY_BODY),
            ("snowflake", _LAYOUT_ONLY_BODY),
            ("postgres", _LAYOUT_ONLY_BODY.replace("  amount,\n", "  amount\n")),
            ("bigquery", _LAYOUT_ONLY_BODY),
            (
                "databricks",
                " ".join(_LAYOUT_ONLY_BODY.replace("  amount,\n", "  amount\n").lower().split())
                + "\n",
            ),
            ("sqlserver", _LAYOUT_ONLY_BODY.replace("  amount,\n", "  amount\n")),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_select_all_and_implicit_aliases_when_formatting_then_only_layout_changes(
    test_case: LayoutOnlyFormatIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{test_case.adapter}"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    authored: str = " ".join(test_case.expected_body.lower().split())
    model.write_text(f'MODEL (description "Orders");\n\n{authored}\n', encoding="utf-8")

    format_exit: int = main(["--project-dir", str(tmp_path), "--no-color", "format"])
    check_exit: int = main(["--project-dir", str(tmp_path), "--no-color", "format", "--check"])
    output: CaptureResult[str] = capsys.readouterr()

    assert (format_exit, check_exit) == (0, 0), output.out + output.err
    assert model.read_text(encoding="utf-8").endswith(test_case.expected_body)


@pytest.mark.parametrize(
    "test_case",
    [
        LineWidthWrapIntegrationTestCase(
            description="long audit list and long reference arguments wrap at the project width",
            line_width=60,
            authored_sql=(
                "MODEL (\n"
                '  description "Order totals",\n'
                "  materialized table,\n"
                "  columns (\n"
                "    order_id (audits [not_null, unique, accepted_values (values [1, 2, 3])]),\n"
                "  ),\n"
                ");\n\n"
                "select o.order_id, coalesce(o.amount, o.fallback_amount, 0) + o.tax as total "
                'from __ref("orders_with_a_long_name") o where o.amount > 0 and o.order_id > 0 '
                "and o.tax >= 0 and o.fallback_amount is null\n"
            ),
            expected_sql=(
                "MODEL (\n"
                '  description "Order totals",\n'
                "  materialized table,\n"
                "  columns (\n"
                "    order_id (audits [\n"
                "      not_null,\n"
                "      unique,\n"
                "      accepted_values (values [1, 2, 3]),\n"
                "    ]),\n"
                "  ),\n"
                ");\n\n"
                "SELECT\n"
                "  o.order_id,\n"
                "  COALESCE(o.amount, o.fallback_amount, 0) + o.tax AS total\n"
                'FROM __ref("orders_with_a_long_name") o\n'
                "WHERE\n"
                "  o.amount > 0\n"
                "  AND o.order_id > 0\n"
                "  AND o.tax >= 0\n"
                "  AND o.fallback_amount IS NULL\n"
            ),
        ),
        LineWidthWrapIntegrationTestCase(
            description="CTE, VALUES, single-item and type-parameter layout rules build",
            line_width=60,
            authored_sql=(
                'MODEL (description "Order totals", materialized table);\n\n'
                'with orders as (select * from __ref("orders_with_a_long_name")),\n'
                "-- paid orders only\n"
                "paid as (select o.order_id, cast(coalesce(o.amount, o.fallback_amount, 0) "
                "+ o.tax as decimal(18, 2)) as total from orders o where o.amount > 0),\n"
                "labels(order_id, label) as (values (1, 'paid and shipped to the customer'), "
                "(2, 'cancelled by the customer'))\n"
                "select p.order_id, p.total from paid p join labels l on p.order_id = l.order_id\n"
            ),
            expected_sql=(
                'MODEL (description "Order totals", materialized table);\n'
                "\n"
                "WITH orders AS (\n"
                "  SELECT *\n"
                '  FROM __ref("orders_with_a_long_name")\n'
                "),\n"
                "\n"
                "-- paid orders only\n"
                "paid AS (\n"
                "  SELECT\n"
                "    o.order_id,\n"
                "    CAST(\n"
                "      COALESCE(o.amount, o.fallback_amount, 0)\n"
                "        + o.tax AS decimal(18, 2)\n"
                "    ) AS total\n"
                "  FROM orders o\n"
                "  WHERE o.amount > 0\n"
                "),\n"
                "\n"
                "labels(order_id, label) AS (\n"
                "  VALUES\n"
                "    (1, 'paid and shipped to the customer'),\n"
                "    (2, 'cancelled by the customer')\n"
                ")\n"
                "\n"
                "SELECT\n"
                "  p.order_id,\n"
                "  p.total\n"
                "FROM paid p\n"
                "JOIN labels l\n"
                "  ON p.order_id = l.order_id\n"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_line_width_when_formatting_then_long_lines_wrap_and_project_builds(
    test_case: LineWidthWrapIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\ndefault_target = "prod"\n\n'
        f"[format]\nline_width = {test_case.line_width}\n\n"
        '[connections.local]\ndatabase = "warehouse.duckdb"\n\n'
        '[targets.prod]\nconnection = "local"\nschema = "prod"\n',
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "orders_with_a_long_name.sql").write_text(
        'MODEL (description "Orders", materialized table);\n\n'
        "SELECT\n  1 AS order_id,\n  5 AS amount,\n  NULL AS fallback_amount,\n  1 AS tax\n",
        encoding="utf-8",
    )
    model: Path = models / "order_totals.sql"
    model.write_text(test_case.authored_sql, encoding="utf-8")

    unformatted_check: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "format", "--check"]
    )
    format_exit: int = main(["--project-dir", str(tmp_path), "--no-color", "format"])
    formatted_check: int = main(["--project-dir", str(tmp_path), "--no-color", "format", "--check"])
    build_exit, build_output = run_build(project_dir=tmp_path, flags=(), capsys=capsys)

    assert (unformatted_check, format_exit, formatted_check, build_exit) == (1, 0, 0, 0), (
        build_output
    )
    assert model.read_text(encoding="utf-8") == test_case.expected_sql
    assert query_duckdb_rows(
        db_path=tmp_path / "warehouse.duckdb", sql="SELECT order_id, total FROM prod.order_totals"
    ) == ((1, 6),)
