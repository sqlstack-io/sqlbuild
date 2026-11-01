"""Real compiler and CLI coverage for join-key and final-CTE conventions."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import SqlReadabilityTestCase

PROJECT_CONFIG: str = 'name = "orders"\nadapter = "duckdb"\n\n[rules]\nignore = ["SQLCLEAN"]\n'


@pytest.mark.parametrize(
    "test_case",
    [
        SqlReadabilityTestCase("equality", "ON o.order_id = c.order_id", False),
        SqlReadabilityTestCase("parenthesized column", "ON (o.order_id) = c.order_id", False),
        SqlReadabilityTestCase("null safe", "ON o.order_id IS NOT DISTINCT FROM c.order_id", False),
        SqlReadabilityTestCase("signed literal", "ON o.order_id >= -1", False),
        SqlReadabilityTestCase(
            "range", "ON o.order_id >= c.order_id AND o.order_id <> c.order_id", False
        ),
        SqlReadabilityTestCase("between", "ON o.order_id BETWEEN c.order_id AND c.order_id", False),
        SqlReadabilityTestCase("literal", "ON o.status = 'success'", False),
        SqlReadabilityTestCase("literal list", "ON o.status IN ('new', 'success')", False),
        SqlReadabilityTestCase("null", "ON o.order_id IS NOT NULL", False),
        SqlReadabilityTestCase("using", "USING (order_id)", False),
        SqlReadabilityTestCase(
            "multiple joins",
            "ON o.order_id = c.order_id LEFT JOIN customers AS p ON p.order_id = c.order_id",
            False,
        ),
        SqlReadabilityTestCase(
            "second computed join",
            "ON o.order_id = c.order_id LEFT JOIN customers AS p ON p.order_id + 1 = c.order_id",
            True,
        ),
        SqlReadabilityTestCase(
            "plain disjunction", "ON (o.order_id = c.order_id OR o.status = 'new')", False
        ),
        SqlReadabilityTestCase("function", "ON LOWER(o.status) = c.status", True),
        SqlReadabilityTestCase("cast", "ON CAST(o.order_id AS BIGINT) = c.order_id", True),
        SqlReadabilityTestCase("colon cast", "ON o.order_id = c.order_id::BIGINT", True),
        SqlReadabilityTestCase("arithmetic", "ON o.order_id + 1 = c.order_id", True),
        SqlReadabilityTestCase("concatenation", "ON o.status || 'x' = c.status", True),
        SqlReadabilityTestCase(
            "case", "ON CASE WHEN o.order_id = 1 THEN 1 ELSE 2 END = c.order_id", True
        ),
        SqlReadabilityTestCase(
            "subquery", "ON o.order_id IN (SELECT order_id FROM customers)", True
        ),
        SqlReadabilityTestCase(
            "computed disjunction", "ON o.order_id = c.order_id OR LOWER(o.status) = c.status", True
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_join_predicate_when_compiling_and_running_rules_then_expression_policy_applies(
    test_case: SqlReadabilityTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(PROJECT_CONFIG)
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders");\n'
        "WITH customers AS (SELECT 1 AS order_id, 'success' AS status),\n"
        "final AS (SELECT o.order_id FROM customers AS o LEFT JOIN customers AS c "
        f"{test_case.sql}) SELECT * FROM final\n"
    )
    assert main(["--project-dir", str(tmp_path), "compile", "--no-cache"]) == 0
    capsys.readouterr()
    result: int = main(["--project-dir", str(tmp_path), "rules", "--json", "run", "SQLCLEAN040"])
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert result == int(test_case.expected_fault)
    assert bool(payload["findings"]) == test_case.expected_fault
    assert ("SQLCLEAN040" in json.dumps(payload["findings"])) == test_case.expected_fault


@pytest.mark.parametrize(
    "test_case",
    [
        SqlReadabilityTestCase("no CTE", "SELECT 1 AS order_id", False),
        SqlReadabilityTestCase(
            "wrapped root", "(WITH final AS (SELECT 1 AS order_id) SELECT * FROM final)", False
        ),
        SqlReadabilityTestCase(
            "double wrapped root",
            "((WITH final AS (SELECT 1 AS order_id) SELECT * FROM final))",
            False,
        ),
        SqlReadabilityTestCase(
            "wrapped wrong name",
            "((WITH orders AS (SELECT 1 AS order_id) SELECT * FROM orders))",
            True,
        ),
        SqlReadabilityTestCase(
            "wrapped CTE union",
            "((WITH final AS (SELECT 1 AS order_id UNION ALL SELECT 2 AS order_id) SELECT * FROM final))",
            False,
        ),
        SqlReadabilityTestCase(
            "wrapped terminal union naming",
            "(WITH final AS (SELECT 1 AS order_id) SELECT * FROM final UNION ALL SELECT * FROM final)",
            False,
        ),
        SqlReadabilityTestCase(
            "nested scalar final",
            "SELECT (WITH final AS (SELECT 1 AS order_id) SELECT order_id FROM final) AS order_id",
            True,
        ),
        SqlReadabilityTestCase(
            "wrapped plain terminal",
            "((WITH final AS (SELECT 1 AS order_id) SELECT * FROM final))",
            False,
            "SQLCLEAN035",
        ),
        SqlReadabilityTestCase(
            "wrapped computed terminal",
            "((WITH final AS (SELECT 1 AS order_id) SELECT order_id + 1 AS order_id FROM final))",
            True,
            "SQLCLEAN035",
        ),
        SqlReadabilityTestCase(
            "wrapped terminal union shape",
            "(WITH final AS (SELECT 1 AS order_id) SELECT * FROM final UNION ALL SELECT * FROM final)",
            True,
            "SQLCLEAN035",
        ),
        SqlReadabilityTestCase(
            "wrapped CTE union terminal shape",
            "((WITH final AS (SELECT 1 AS order_id UNION ALL SELECT 2 AS order_id) SELECT * FROM final))",
            False,
            "SQLCLEAN035",
        ),
        SqlReadabilityTestCase(
            "wrapped root is not nested",
            "((WITH final AS (SELECT 1 AS order_id) SELECT * FROM final))",
            False,
            "SQLCLEAN036",
        ),
        SqlReadabilityTestCase(
            "scalar CTE is nested",
            "SELECT (WITH orders AS (SELECT 1 AS order_id) SELECT order_id FROM orders) AS order_id",
            True,
            "SQLCLEAN036",
        ),
        SqlReadabilityTestCase(
            "final", "WITH final AS (SELECT 1 AS order_id) SELECT * FROM final", False
        ),
        SqlReadabilityTestCase(
            "quoted mixed case",
            'WITH "FiNaL" AS (SELECT 1 AS order_id) SELECT * FROM "FiNaL"',
            False,
        ),
        SqlReadabilityTestCase(
            "column list", "WITH final(order_id) AS (SELECT 1) SELECT * FROM final", False
        ),
        SqlReadabilityTestCase(
            "other name", "WITH orders AS (SELECT 1 AS order_id) SELECT * FROM orders", True
        ),
        SqlReadabilityTestCase(
            "early final",
            "WITH final AS (SELECT 1 AS order_id), orders AS (SELECT * FROM final) SELECT * FROM orders",
            True,
        ),
        SqlReadabilityTestCase(
            "nested final",
            "WITH orders AS (WITH final AS (SELECT 1 AS order_id) SELECT * FROM final), final AS (SELECT * FROM orders) SELECT * FROM final",
            True,
        ),
        SqlReadabilityTestCase(
            "terminal shape belongs to 035",
            "WITH final AS (SELECT 1 AS order_id) SELECT order_id + 1 AS order_id FROM final",
            False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cte_names_when_compiling_and_running_rules_then_final_name_policy_applies(
    test_case: SqlReadabilityTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(PROJECT_CONFIG)
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(f'MODEL (description "Orders");\n{test_case.sql}\n')
    assert main(["--project-dir", str(tmp_path), "compile", "--no-cache"]) == 0
    capsys.readouterr()
    result: int = main(
        ["--project-dir", str(tmp_path), "rules", "--json", "run", test_case.rule_code]
    )
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert result == int(test_case.expected_fault)
    assert bool(payload["findings"]) == test_case.expected_fault
    assert (test_case.rule_code in json.dumps(payload["findings"])) == test_case.expected_fault


@pytest.mark.parametrize(
    "test_case",
    [SqlReadabilityTestCase("family enables both rules", "SQLCLEAN", True)],
    ids=lambda case: case.description,
)
def test_given_family_selection_when_compiling_then_new_rules_are_enforced(
    test_case: SqlReadabilityTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = ["{test_case.sql}"]\n'
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders");\n'
        "WITH customers AS (SELECT 1 AS order_id), "
        "orders AS (SELECT o.order_id FROM customers AS o INNER JOIN customers AS c "
        "ON o.order_id + 1 = c.order_id) SELECT order_id FROM orders\n"
    )
    assert main(["--project-dir", str(tmp_path), "compile", "--no-cache"]) == int(
        test_case.expected_fault
    )
    output: CaptureResult[str] = capsys.readouterr()
    assert "SQLCLEAN040" in output.out + output.err
    assert "SQLCLEAN041" in output.out + output.err
    assert main(["--project-dir", str(tmp_path), "rules", "--json", "run", test_case.sql]) == int(
        test_case.expected_fault
    )
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert "SQLCLEAN040" in json.dumps(payload["findings"])
    assert "SQLCLEAN041" in json.dumps(payload["findings"])


@pytest.mark.parametrize(
    "test_case",
    [
        SqlReadabilityTestCase("lateral true", "TRUE", False),
        SqlReadabilityTestCase("lateral false", "FALSE", False),
        SqlReadabilityTestCase("literal equality", "1 = 1", False),
        SqlReadabilityTestCase("computed constant", "CAST(TRUE AS BOOLEAN)", True),
        SqlReadabilityTestCase("variant key", "o.data:key = f.value", True),
    ],
    ids=lambda case: case.description,
)
def test_given_lateral_join_when_running_expression_rule_then_literals_pass_and_variant_keys_fail(
    test_case: SqlReadabilityTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "snowflake"\n')
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders", database warehouse, schema analytics);\n'
        "SELECT o.order_id FROM raw_orders AS o LEFT JOIN LATERAL FLATTEN(input => o.items) AS f "
        f"ON {test_case.sql}\n"
    )
    result: int = main(["--project-dir", str(tmp_path), "rules", "--json", "run", "SQLCLEAN040"])
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert result == int(test_case.expected_fault)
    assert ("SQLCLEAN040" in json.dumps(payload["findings"])) == test_case.expected_fault


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
