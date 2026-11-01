"""Real compiler and CLI coverage for transactional Rule fixes."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import duckdb
import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    FormatCompileIntegrationTestCase,
    SemanticFixTestCase,
    UnusedOutputDifferentialCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    built_model_rows,
    rule_fix_statuses,
)

_DIFFERENTIAL_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
_DIFFERENTIAL_RAW_SQL: str = (
    'MODEL (description "Raw orders");\n'
    "WITH v (order_id, customer_id, status, amount, tags) AS (VALUES (1, 7, 'open', 3, [1, 2]),"
    " (2, 7, 'done', 5, [3]), (3, 8, 'done', 3, [4, 5, 6]), (4, 9, 'done', 2, [7]))\n"
    "SELECT v.order_id, v.customer_id, v.status, v.amount, v.tags FROM v\n"
)


@pytest.mark.parametrize(
    "test_case",
    [FormatCompileIntegrationTestCase("bare union", "UNION DISTINCT")],
    ids=lambda case: case.description,
)
def test_given_bare_union_when_fixing_then_preserves_output_and_is_idempotent(
    test_case: FormatCompileIntegrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    original = 'MODEL (description "Orders");\nSELECT 1 AS order_id UNION SELECT 2 AS order_id\n'
    model.write_text(original)
    assert main(["--project-dir", str(tmp_path), "format", "--fix", "--check", "--json"]) == 1
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert model.read_text() == original
    assert any(
        fix["code"] == "SQLCLEAN008" and fix["status"] == "applied" for fix in payload["rule_fixes"]
    )
    assert main(["--project-dir", str(tmp_path), "format", "--fix", "--json"]) == 0
    capsys.readouterr()
    assert test_case.expected_literal in model.read_text()
    assert main(["--project-dir", str(tmp_path), "format", "--fix", "--check", "--json"]) == 0


@pytest.mark.parametrize(
    "test_case",
    [FormatCompileIntegrationTestCase("null comparison", "= NULL")],
    ids=lambda case: case.description,
)
def test_given_null_comparison_when_fixing_then_refuses_semantics_change(
    test_case: FormatCompileIntegrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id WHERE 1 = NULL\n')
    main(["--project-dir", str(tmp_path), "format", "--fix", "--json"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert any(
        fix["code"] == "SQLCLEAN001" and fix["status"] == "refused" for fix in payload["rule_fixes"]
    )
    assert test_case.expected_literal in model.read_text()


@pytest.mark.parametrize(
    "test_case",
    [FormatCompileIntegrationTestCase("unanalysed model", "format-fix-verification-failed")],
    ids=lambda case: case.description,
)
def test_given_unanalysed_model_when_fixing_then_verification_preserves_original_file(
    test_case: FormatCompileIntegrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "customers.sql").write_text(
        'MODEL (description "Customers");\nSELECT 1 AS customer_id\n'
    )
    model: Path = models / "orders.sql"
    original = (
        'MODEL (description "Orders", sql_analysis false);\n'
        "WITH unused AS (SELECT 2 AS customer_id) SELECT 1 AS order_id\n"
    )
    model.write_text(original)
    assert main(["--project-dir", str(tmp_path), "format", "--fix", "--json"]) == 1
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert model.read_text() == original
    assert any(v["code"] == test_case.expected_literal for v in payload["violations"])


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticFixTestCase(
            "comma product",
            "SELECT a.order_id, b.customer_id FROM "
            "(SELECT 1 AS order_id) AS a, (SELECT 2 AS customer_id) AS b",
            "SQLCLEAN002",
        ),
        SemanticFixTestCase(
            "unused CTE",
            "WITH unused AS (SELECT 2 AS customer_id) SELECT 1 AS order_id",
            "SQLCLEAN005",
        ),
        SemanticFixTestCase(
            "constant row count",
            "SELECT COUNT(1) AS order_count FROM (SELECT 1 AS order_id UNION ALL SELECT 2) AS o",
            "SQLCLEAN017",
        ),
        SemanticFixTestCase(
            "implicit inner join",
            "SELECT a.order_id, b.customer_id FROM (SELECT 1 AS order_id) AS a "
            "JOIN (SELECT 1 AS order_id, 2 AS customer_id) AS b ON a.order_id = b.order_id",
            "SQLCLEAN025",
        ),
        SemanticFixTestCase(
            "redundant distinct",
            "SELECT DISTINCT order_id FROM (SELECT 1 AS order_id) AS orders GROUP BY order_id",
            "SQLCLEAN006",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_safe_native_fix_when_formatting_then_compiler_and_query_results_are_preserved(
    test_case: SemanticFixTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders", schema analytics);\n' + test_case.sql)
    assert main(["--project-dir", str(tmp_path), "format", "--fix", "--json"]) == 0
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert any(
        fix["code"] == test_case.expected_code and fix["status"] == "applied"
        for fix in payload["rule_fixes"]
    )
    with duckdb.connect() as connection:
        expected: list[tuple[Any, ...]] = connection.execute(test_case.sql).fetchall()
        actual: list[tuple[Any, ...]] = connection.execute(
            model.read_text().split(";", 1)[1]
        ).fetchall()
    assert actual == expected


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticFixTestCase(
            "Snowflake cartesian JOIN",
            "SELECT a.order_id, b.customer_id FROM "
            "(SELECT 1 AS order_id) AS a JOIN (SELECT 2 AS customer_id) AS b",
            "SQLCLEAN003",
            dialect="snowflake",
        ),
        SemanticFixTestCase(
            "DuckDB invalid JOIN",
            "SELECT a.order_id, b.customer_id FROM "
            "(SELECT 1 AS order_id) AS a JOIN (SELECT 2 AS customer_id) AS b",
            "SQLCLEAN003",
            expected_status="refused",
        ),
        SemanticFixTestCase(
            "parentheses precedence",
            "SELECT DISTINCT (1 + 2) * 3 AS order_id",
            "SQLCLEAN013",
            expected_status="refused",
        ),
        SemanticFixTestCase(
            "unused table alias proven by recompiling",
            "SELECT order_id FROM (SELECT 1 AS order_id) AS o",
            "SQLCLEAN023",
        ),
        SemanticFixTestCase(
            "boolean CASE whose rewrite changes inferred nullability",
            "SELECT CASE WHEN 1 = 2 THEN TRUE ELSE FALSE END AS active",
            "SQLCLEAN030",
            expected_status="refused",
            expected_exit_code=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dialect_sensitive_rewrite_when_fixing_then_requires_semantic_proof(
    test_case: SemanticFixTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{test_case.dialect}"\n'
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders", database example, schema analytics);\n' + test_case.sql
    )
    exit_code: int = main(["--project-dir", str(tmp_path), "format", "--fix", "--json"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert exit_code == test_case.expected_exit_code
    assert any(
        fix["code"] == test_case.expected_code and fix["status"] == test_case.expected_status
        for fix in payload["rule_fixes"]
    )


@pytest.mark.parametrize(
    "test_case",
    [FormatCompileIntegrationTestCase("one unverifiable file", "format-fix-verification-failed")],
    ids=lambda case: case.description,
)
def test_given_one_unverifiable_file_when_fixing_then_only_that_file_keeps_its_original(
    test_case: FormatCompileIntegrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    models: Path = tmp_path / "models"
    models.mkdir()
    verified: Path = models / "orders.sql"
    verified.write_text(
        'MODEL (description "Orders");\nSELECT 1 AS order_id UNION SELECT 2 AS order_id\n'
    )
    unverifiable: Path = models / "customers.sql"
    unverifiable_sql: str = (
        'MODEL (description "Customers", sql_analysis false);\n'
        "WITH unused AS (SELECT 2 AS customer_id) SELECT 1 AS customer_id\n"
    )
    unverifiable.write_text(unverifiable_sql)

    exit_code: int = main(["--project-dir", str(tmp_path), "format", "--fix", "--json"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)

    assert exit_code == 1
    assert "UNION DISTINCT" in verified.read_text()
    assert unverifiable.read_text() == unverifiable_sql
    assert {
        (Path(fix["file"]).name, fix["code"], fix["status"]) for fix in payload["rule_fixes"]
    } >= {
        ("orders.sql", "SQLCLEAN008", "applied"),
        ("customers.sql", "SQLCLEAN005", "refused"),
    }
    faults: set[tuple[str, str]] = {
        (Path(violation["file"]).name, violation["code"]) for violation in payload["violations"]
    }
    assert ("customers.sql", test_case.expected_literal) in faults
    assert ("orders.sql", test_case.expected_literal) not in faults
    listed: list[tuple[str, str, int, str]] = [
        (fix["file"], fix["code"], fix["line"], fix["reason"]) for fix in payload["rule_fixes"]
    ]
    assert len(listed) == len(set(listed))


@pytest.mark.parametrize(
    "test_case",
    [FormatCompileIntegrationTestCase("existing errors elsewhere", "UNION DISTINCT")],
    ids=lambda case: case.description,
)
def test_given_existing_errors_and_unexpandable_file_when_fixing_then_other_files_are_fixed(
    test_case: FormatCompileIntegrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    models: Path = tmp_path / "models"
    models.mkdir()
    (tmp_path / "macros" / "python").mkdir(parents=True)
    (tmp_path / "macros" / "python" / "relations.py").write_text(
        "def orders_relation() -> str:\n    return '__ref(\"orders\")'\n"
    )
    fixable: Path = models / "orders.sql"
    fixable.write_text(
        'MODEL (description "Orders");\nSELECT 1 AS order_id UNION SELECT 2 AS order_id\n'
    )
    unexpandable: Path = models / "customers.sql"
    unexpandable_sql: str = (
        'MODEL (description "Customers");\n'
        "SELECT o.order_id AS customer_id FROM @py.orders_relation() AS o\n"
    )
    unexpandable.write_text(unexpandable_sql)
    (models / "payments.sql").write_text(
        "MODEL (description \"Payments\");\nSELECT 1 AS payment_id WHERE 'open' > 1\n"
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "format", "--fix", "--json"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)

    assert exit_code == 0
    assert test_case.expected_literal in fixable.read_text()
    assert unexpandable.read_text() == unexpandable_sql
    assert {
        (Path(fix["file"]).name, fix["code"], fix["status"]) for fix in payload["rule_fixes"]
    } >= {("orders.sql", "SQLCLEAN008", "applied")}
    assert {
        (
            Path(violation["file"]).name,
            violation["code"],
            violation["severity"],
            "could not be expanded" in violation["message"],
        )
        for violation in payload["violations"]
    } >= {("customers.sql", "format-fix-skipped", "warning", True)}


@pytest.mark.parametrize(
    "test_case",
    [
        FormatCompileIntegrationTestCase(
            "only an unverifiable fix", "Output columns could not be inferred"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_only_unverifiable_fixes_when_fixing_then_every_refusal_is_listed_with_its_reason(
    test_case: FormatCompileIntegrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "orders.sql").write_text(
        'MODEL (description "Orders");\nSELECT 1 AS order_id WHERE 1 = NULL\n'
    )
    (models / "customers.sql").write_text(
        'MODEL (description "Customers", sql_analysis false);\n'
        "WITH unused AS (SELECT 2 AS customer_id) SELECT 1 AS customer_id\n"
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "format", "--fix", "--json"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)

    assert exit_code == 1
    assert {
        (Path(fix["file"]).name, fix["code"], fix["status"]) for fix in payload["rule_fixes"]
    } >= {("orders.sql", "SQLCLEAN001", "refused"), ("customers.sql", "SQLCLEAN005", "refused")}
    assert {
        (Path(violation["file"]).name, violation["message"]) for violation in payload["violations"]
    } >= {("customers.sql", test_case.expected_literal + ", so the fix cannot be verified")}


@pytest.mark.parametrize(
    "test_case",
    [
        UnusedOutputDifferentialCase(
            description="a plain column is removed",
            cte_sql='SELECT r.order_id, r.status FROM __ref("raw_orders") AS r',
            reader_sql="SELECT g.order_id FROM g",
            expected_statuses=("applied",),
        ),
        UnusedOutputDifferentialCase(
            description="GROUP BY ALL keeps its grouping column",
            cte_sql=(
                "SELECT r.customer_id, r.status, SUM(r.amount) AS total"
                ' FROM __ref("raw_orders") AS r GROUP BY ALL'
            ),
            reader_sql="SELECT g.customer_id, g.total FROM g",
            expected_statuses=("refused",),
        ),
        UnusedOutputDifferentialCase(
            description="an ungrouped aggregate keeps the select at one row",
            cte_sql="SELECT 'all' AS label, COUNT(*) AS n FROM __ref(\"raw_orders\") AS r",
            reader_sql="SELECT g.label FROM g",
            expected_statuses=("refused",),
        ),
        UnusedOutputDifferentialCase(
            description="a set-returning projection keeps its row expansion",
            cte_sql='SELECT r.order_id, UNNEST(r.tags) AS tag FROM __ref("raw_orders") AS r',
            reader_sql="SELECT g.order_id FROM g",
            expected_statuses=("refused",),
        ),
        UnusedOutputDifferentialCase(
            description="ORDER BY ALL with LIMIT keeps its sort columns",
            cte_sql=(
                'SELECT r.order_id, r.amount FROM __ref("raw_orders") AS r ORDER BY ALL LIMIT 2'
            ),
            reader_sql="SELECT g.amount FROM g",
            expected_statuses=("refused",),
        ),
        UnusedOutputDifferentialCase(
            description="a COLUMNS pattern reads every column",
            cte_sql='SELECT r.order_id, r.status FROM __ref("raw_orders") AS r',
            reader_sql="SELECT MAX(COLUMNS('^s')) FROM g",
            expected_statuses=(),
        ),
        UnusedOutputDifferentialCase(
            description="COLUMNS(*) reads every column",
            cte_sql='SELECT r.order_id, r.amount FROM __ref("raw_orders") AS r',
            reader_sql="SELECT MAX(COLUMNS(*)) FROM g",
            expected_statuses=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unused_cte_output_when_fixing_then_model_rows_are_unchanged(
    test_case: UnusedOutputDifferentialCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_DIFFERENTIAL_TOML)
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "raw_orders.sql").write_text(_DIFFERENTIAL_RAW_SQL)
    (models / "summary.sql").write_text(
        'MODEL (description "Summary");\n'
        f"WITH g AS ({test_case.cte_sql}),\nfinal AS ({test_case.reader_sql})\n"
        "SELECT * FROM final\n"
    )

    before: list[tuple[Any, ...]] = built_model_rows(
        project_dir=tmp_path, model="summary", capsys=capsys
    )
    assert main(["--project-dir", str(tmp_path), "format", "--fix", "--json"]) in {0, 1}
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    after: list[tuple[Any, ...]] = built_model_rows(
        project_dir=tmp_path, model="summary", capsys=capsys
    )

    assert after == before
    assert (
        rule_fix_statuses(payload=payload, code="SQLCLEAN042", file_name="summary.sql")
        == test_case.expected_statuses
    )


@pytest.mark.parametrize(
    "test_case",
    [FormatCompileIntegrationTestCase("macro and CRLF source", "UNION DISTINCT")],
    ids=lambda case: case.description,
)
def test_given_macro_and_crlf_source_when_fixing_then_previews_preserve_bytes_and_write_keeps_mode(
    test_case: FormatCompileIntegrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    macros: Path = tmp_path / "macros" / "python"
    macros.mkdir(parents=True)
    (macros / "values.py").write_text('def order_value():\n    return "1"\n')
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    original: bytes = (
        b'MODEL (description "Orders");\r\n'
        b"SELECT @py.order_value() AS order_id UNION SELECT 2 AS order_id\r\n"
    )
    model.write_bytes(original)
    model.chmod(0o640)
    original_mode: int = model.stat().st_mode

    assert main(["--project-dir", str(tmp_path), "format", "--fix", "--check", "--json"]) == 1
    capsys.readouterr()
    assert model.read_bytes() == original
    assert main(["--project-dir", str(tmp_path), "format", "--fix", "--diff"]) == 0
    capsys.readouterr()
    assert model.read_bytes() == original

    assert main(["--project-dir", str(tmp_path), "format", "--fix", "--json"]) == 0
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert any(
        fix["code"] == "SQLCLEAN008" and fix["status"] == "applied" for fix in payload["rule_fixes"]
    )
    written: bytes = model.read_bytes()
    assert b"@py.order_value()" in written
    assert test_case.expected_literal.encode() in written
    assert written.count(b"\n") == written.count(b"\r\n")
    assert model.stat().st_mode == original_mode


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-vv"]))
