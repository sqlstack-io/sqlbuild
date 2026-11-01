"""Canonical formatting retains authored syntax and compiler-scale query budgets."""

import json
from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import FormatterSyntaxTestCase

# The window syntax cases order by a non-unique column, which SQLCLEAN018 reports.
_WINDOW_SYNTAX_PROJECT_TOML: str = (
    'name = "orders"\nadapter = "snowflake"\n\n[rules]\nignore = ["SQLCLEAN018"]\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        FormatterSyntaxTestCase(
            "casts and qualified aliases",
            "SELECT CAST(o.amount AS NUMERIC(10, 2)) AS amount, o.amount::DOUBLE AS total FROM orders o",
            ("CAST(o.amount AS NUMERIC(10, 2))", "o.amount::DOUBLE"),
        ),
        FormatterSyntaxTestCase(
            "variant paths and keyword keys",
            "SELECT o.payload:customer.name::VARCHAR AS customer_name, o.payload:index::INTEGER AS item_index FROM orders o",
            ("o.payload:customer.name::VARCHAR", "o.payload:index::INTEGER"),
        ),
        FormatterSyntaxTestCase(
            "literal spelling",
            r"SELECT $$Order summary$$ AS label, 'Customer''s order' AS other_label, '\\s+' AS pattern, 1.25e-5 AS ratio, TIMESTAMP '2025-01-01 00:00:00' AS created_at",
            (
                "$$Order summary$$",
                "'Customer''s order'",
                r"'\\s+'",
                "1.25e-5",
                "TIMESTAMP '2025-01-01 00:00:00'",
            ),
        ),
        FormatterSyntaxTestCase(
            "null treatment and function syntax",
            "SELECT FIRST_VALUE(o.id IGNORE NULLS) OVER (ORDER BY o.id) AS first_id, POSITION('new' IN o.status) AS status_position, MOD(o.id, 2) AS remainder FROM orders o",
            ("FIRST_VALUE(o.id IGNORE NULLS)", "POSITION('new' IN o.status)", "MOD(o.id, 2)"),
        ),
        FormatterSyntaxTestCase(
            "typed lambda",
            "SELECT TRANSFORM(o.items, item VARIANT -> item:order_id::INTEGER) AS order_ids FROM orders o",
            ("item VARIANT ->", "item:order_id::INTEGER"),
        ),
        FormatterSyntaxTestCase(
            "commented cast parameters",
            "SELECT CAST(amount AS NUMERIC(/* precision */ 10, 2)) AS amount FROM orders",
            ("/* precision */", "NUMERIC("),
        ),
        FormatterSyntaxTestCase(
            "trailing projection comma",
            "SELECT o.order_id, -- Keep the order identifier.\nFROM orders o",
            ("-- Keep the order identifier.", "o.order_id"),
        ),
        FormatterSyntaxTestCase(
            "compiler function depth",
            "SELECT " + "LOWER(" * 80 + "'NEW'" + ")" * 80 + " AS status",
            ("'NEW'", "AS status"),
        ),
        FormatterSyntaxTestCase(
            "compiler set operation budget",
            " UNION ALL ".join("SELECT 1 AS order_id" for _ in range(300)),
            ("UNION ALL", "AS order_id"),
        ),
        FormatterSyntaxTestCase(
            "CTE-producing macro",
            "WITH @py.order_input(), final AS (SELECT order_id FROM orders) SELECT * FROM final",
            ("@py.order_input()", "final AS ("),
            'def order_input() -> str:\n    """Return an orders input CTE."""\n'
            '    return "orders AS (SELECT 1 AS order_id)"\n',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_authored_syntax_when_formatting_then_spelling_and_idempotence_are_preserved(
    test_case: FormatterSyntaxTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_WINDOW_SYNTAX_PROJECT_TOML)
    macros: Path = tmp_path / "models" / "_macros" / "python"
    macros.mkdir(parents=True)
    (macros / "orders.py").write_text(test_case.macro_source)
    model: Path = tmp_path / "models" / "order_summary.sql"
    model.write_text(
        'MODEL (description "Order summary", database warehouse, schema analytics);\n-- Order query.\n'
        + test_case.sql
        + "\n"
    )
    assert main(["--project-dir", str(tmp_path), "format", "--json"]) == 0
    capsys.readouterr()
    formatted: str = model.read_text()
    assert all(fragment in formatted for fragment in test_case.expected_fragments)
    assert main(["--project-dir", str(tmp_path), "format", "--check", "--json"]) == 0
    assert model.read_text() == formatted
    assert main(["--project-dir", str(tmp_path), "compile", "--no-cache"]) == 0


@pytest.mark.parametrize(
    "test_case",
    [
        FormatterSyntaxTestCase(
            "computed variant key retains source-preserving parser support",
            "SELECT payload:customers[TO_VARCHAR(order_id)] AS customer FROM orders",
            ("payload:customers[TO_VARCHAR(order_id)]",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_computed_variant_key_when_formatting_then_preserves_syntax_and_compiles(
    test_case: FormatterSyntaxTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "snowflake"\n')
    model: Path = tmp_path / "models" / "order_summary.sql"
    model.parent.mkdir()
    original: str = (
        'MODEL (description "Order summary", database warehouse, schema analytics);   \n'
        + test_case.sql
        + "\n"
    )
    model.write_text(original)
    assert main(["--project-dir", str(tmp_path), "compile", "--no-cache"]) == 0
    capsys.readouterr()
    assert main(["--project-dir", str(tmp_path), "format", "--json"]) == 0
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["formatted_files"]
    formatted: str = model.read_text()
    assert all(fragment in formatted for fragment in test_case.expected_fragments)
    assert main(["--project-dir", str(tmp_path), "format", "--check", "--json"]) == 0
    assert model.read_text() == formatted
    assert main(["--project-dir", str(tmp_path), "compile", "--no-cache"]) == 0


@pytest.mark.parametrize(
    "test_case",
    [
        FormatterSyntaxTestCase(
            "model reference", 'select order_id from __ref("orders")', ('__ref("orders")',)
        ),
        FormatterSyntaxTestCase(
            "source reference",
            'select order_id from __source("raw.orders")',
            ('__source("raw.orders")',),
        ),
        FormatterSyntaxTestCase(
            "seed reference", 'select order_id from __seed("orders")', ('__seed("orders")',)
        ),
        FormatterSyntaxTestCase(
            "dbt reference", 'select order_id from __dbt_ref("orders")', ('__dbt_ref("orders")',)
        ),
        FormatterSyntaxTestCase(
            "table function",
            'select * from __table_fn("order_items")(1)',
            ('__table_fn("order_items")(1)',),
        ),
        FormatterSyntaxTestCase(
            "scalar function",
            'select __udf("order_label")(1) as label',
            ('__udf("order_label")(1)',),
        ),
        FormatterSyntaxTestCase(
            "cursor start", "select __cursor_start() as start_at", ("__cursor_start()",)
        ),
        FormatterSyntaxTestCase(
            "cursor end", "select __cursor_end() as end_at", ("__cursor_end()",)
        ),
        FormatterSyntaxTestCase(
            "empty fixture", "select * from __empty_fixture()", ("__empty_fixture()",)
        ),
        FormatterSyntaxTestCase(
            "authored case and spacing",
            "select __CuRsOr_StArT () as start_at",
            ("__CuRsOr_StArT ()",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sqlbuild_call_when_formatting_then_authored_spelling_is_preserved(
    test_case: FormatterSyntaxTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    model: Path = tmp_path / "models" / "order_summary.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Order summary");\n' + test_case.sql + "\n")
    assert main(["--project-dir", str(tmp_path), "format", "--json"]) == 0
    capsys.readouterr()
    formatted: str = model.read_text()
    assert all(fragment in formatted for fragment in test_case.expected_fragments)
    assert main(["--project-dir", str(tmp_path), "format", "--check", "--json"]) == 0
    assert model.read_text() == formatted


@pytest.mark.parametrize(
    "test_case",
    [
        FormatterSyntaxTestCase(
            "several CTE macros with attached comments",
            "WITH base AS (SELECT 1 AS order_id),\n"
            '-- First order stage.\n@py.make_ctes("first_orders", "base"),\n'
            '-- Second order stage.\n@py.make_ctes("second_orders", "first_orders"),\n'
            '@py.other_ctes("third_orders", "second_orders"),\n'
            "-- Publish order identifiers.\nfinal AS (SELECT order_id FROM third_orders)\nSELECT * FROM final",
            (
                '\n@py.make_ctes("first_orders", "base"),\n',
                '\n@py.make_ctes("second_orders", "first_orders"),\n',
                '\n@py.other_ctes("third_orders", "second_orders"),\n',
                "\n-- Publish order identifiers.\nfinal AS (",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cte_macros_and_comments_when_formatting_then_layout_and_rules_stay_valid(
    test_case: FormatterSyntaxTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = ["SQLCLEAN033"]\n'
    )
    macros: Path = tmp_path / "models" / "_macros" / "python"
    macros.mkdir(parents=True)
    (macros / "ctes.py").write_text(
        "def make_ctes(name: str, source: str) -> str:\n"
        '    return f"{name} AS (SELECT order_id FROM {source})"\n\n'
        "def other_ctes(name: str, source: str) -> str:\n"
        "    return make_ctes(name, source)\n"
    )
    model: Path = tmp_path / "models" / "order_summary.sql"
    model.write_text('MODEL (description "Order summary");\n' + test_case.sql + "\n")
    assert main(["--project-dir", str(tmp_path), "compile", "--no-cache"]) == 0
    capsys.readouterr()
    assert main(["--project-dir", str(tmp_path), "format", "--json"]) == 0
    capsys.readouterr()
    formatted: str = model.read_text()
    assert all(fragment in formatted for fragment in test_case.expected_fragments)
    assert main(["--project-dir", str(tmp_path), "format", "--check", "--json"]) == 0
    assert model.read_text() == formatted
    assert main(["--project-dir", str(tmp_path), "compile", "--no-cache"]) == 0
    capsys.readouterr()
    assert main(["--project-dir", str(tmp_path), "rules", "--json", "run", "SQLCLEAN033"]) == 0
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [
        FormatterSyntaxTestCase(
            f"Unicode restoration after {prefix}",
            f"WITH orders AS (SELECT '{prefix}' AS label, order_id, payload FROM __ref(\"raw_orders\")),\n"
            "values_input AS (SELECT column1 AS order_flag FROM VALUES (1)),\n"
            f'-- Stage {prefix} orders.\n@py.copy_orders("staged_orders", "orders"),\n'
            "final AS (SELECT label, "
            f"CAST(order_id AS NUMERIC(/* type {prefix} */ 10, 2)) AS order_id, "
            "payload:customer.name::VARCHAR AS customer_name, __cursor_start() AS start_at, "
            "__cursor_end() AS end_at, FIRST_VALUE(order_id IGNORE NULLS) OVER (ORDER BY order_id) AS first_id, "
            f"POSITION('{prefix}' IN label) AS label_position FROM staged_orders s CROSS JOIN values_input) SELECT * FROM final",
            (
                f"'{prefix}'",
                f"-- Stage {prefix} orders.",
                f"/* type {prefix} */",
                "NUMERIC(",
                "payload:customer.name::VARCHAR",
                '__ref("raw_orders")',
                "__cursor_start()",
                "__cursor_end()",
                "FROM VALUES (1)",
                "FIRST_VALUE(order_id IGNORE NULLS)",
                f"POSITION('{prefix}' IN label)",
                '\n@py.copy_orders("staged_orders", "orders"),\n',
                "\nfinal AS (",
            ),
        )
        for prefix in ("é", "€", "😀", "é € 😀")
    ],
    ids=lambda case: case.description,
)
def test_given_multibyte_text_before_restorations_when_formatting_then_output_is_preserved(
    test_case: FormatterSyntaxTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "snowflake"\n')
    macros: Path = tmp_path / "models" / "_macros" / "python"
    macros.mkdir(parents=True)
    (macros / "orders.py").write_text(
        "def copy_orders(name: str, source: str) -> str:\n"
        '    return f"{name} AS (SELECT * FROM {source})"\n'
    )
    model: Path = tmp_path / "models" / "order_summary.sql"
    model.write_text(
        'MODEL (description "Orders é € 😀");\n' + test_case.sql + "\n", encoding="utf-8"
    )
    assert main(["--project-dir", str(tmp_path), "format", "--json"]) == 0
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["faults"] == 0
    formatted: str = model.read_text(encoding="utf-8")
    assert all(fragment in formatted for fragment in test_case.expected_fragments)
    assert 'description "Orders é € 😀"' in formatted
    assert main(["--project-dir", str(tmp_path), "format", "--check", "--json"]) == 0
    assert model.read_text(encoding="utf-8") == formatted


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
