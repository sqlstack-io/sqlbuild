"""Unicode code-point locations through the real compiler/Rules CLI boundary."""

import json
from itertools import product
from pathlib import Path
from typing import Any

import pytest

import sqlbuild._native as _native
from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import UnicodeRuleLocationTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        UnicodeRuleLocationTestCase(
            f"{case.description} after {prefix}",
            case.rule_code,
            case.sql.replace("LABEL", prefix),
            case.expected_anchors,
        )
        for prefix, case in product(
            ("é", "€", "😀", "é € 😀"),
            (
                UnicodeRuleLocationTestCase(
                    "uncontrolled star",
                    "SQLCLEAN021",
                    "SELECT 'LABEL' AS label, * FROM orders",
                    ("* FROM",),
                ),
                UnicodeRuleLocationTestCase(
                    "controlled projection", "SQLCLEAN021", "SELECT 'LABEL' AS label", ()
                ),
                UnicodeRuleLocationTestCase(
                    "computed terminal",
                    "SQLCLEAN035",
                    "WITH final AS (SELECT 'LABEL' AS label) SELECT label || 'x' AS label FROM final",
                    ("SELECT label ||",),
                ),
                UnicodeRuleLocationTestCase(
                    "plain terminal",
                    "SQLCLEAN035",
                    "((WITH final AS (SELECT 'LABEL' AS label) SELECT label FROM final))",
                    (),
                ),
                UnicodeRuleLocationTestCase(
                    "nested CTE",
                    "SQLCLEAN036",
                    "SELECT 'LABEL' AS label, (WITH nested_orders AS (SELECT 1 AS order_id) SELECT order_id FROM nested_orders) AS order_id",
                    ("WITH nested_orders",),
                ),
                UnicodeRuleLocationTestCase(
                    "root wrapper",
                    "SQLCLEAN036",
                    "((WITH final AS (SELECT 'LABEL' AS label) SELECT label FROM final))",
                    (),
                ),
                UnicodeRuleLocationTestCase(
                    "computed join",
                    "SQLCLEAN040",
                    "WITH orders AS (SELECT 'LABEL' AS label) SELECT o.label FROM orders AS o JOIN orders AS c ON LOWER(o.label) = c.label",
                    ("ON LOWER",),
                ),
                UnicodeRuleLocationTestCase(
                    "plain join",
                    "SQLCLEAN040",
                    "WITH orders AS (SELECT 'LABEL' AS label) SELECT o.label FROM orders AS o JOIN orders AS c ON o.label = c.label",
                    (),
                ),
                UnicodeRuleLocationTestCase(
                    "wrong final name",
                    "SQLCLEAN041",
                    "WITH marker AS (SELECT 'LABEL' AS label), orders AS (SELECT label FROM marker) SELECT label FROM orders",
                    ("orders AS",),
                ),
                UnicodeRuleLocationTestCase(
                    "valid final name",
                    "SQLCLEAN041",
                    "((WITH marker AS (SELECT 'LABEL' AS label), final AS (SELECT label FROM marker) SELECT label FROM final))",
                    (),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unicode_before_sql_when_running_rules_then_findings_have_character_locations(
    test_case: UnicodeRuleLocationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    contents: str = 'MODEL (description "Orders");\n' + test_case.sql + "\n"
    model.write_text(contents, encoding="utf-8")
    native_payload: dict[str, Any] = json.loads(
        _native.lint_sql_json(
            json.dumps(
                {
                    "version": 1,
                    "sql": test_case.sql,
                    "dialect": "duckdb",
                    "enabled_rules": [test_case.rule_code],
                }
            )
        )
    )
    assert [(finding["start"], finding["end"]) for finding in native_payload["diagnostics"]] == [
        (test_case.sql.index(anchor), test_case.sql.index(anchor) + len(anchor.split()[0]))
        for anchor in test_case.expected_anchors
    ]
    result: int = main(
        ["--project-dir", str(tmp_path), "rules", "--json", "run", test_case.rule_code]
    )
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    expected_locations: list[tuple[int, int]] = [
        (
            contents.count("\n", 0, contents.index(anchor)) + 1,
            contents.index(anchor) - contents.rfind("\n", 0, contents.index(anchor)),
        )
        for anchor in test_case.expected_anchors
    ]
    assert result == int(bool(test_case.expected_anchors))
    assert [
        (finding["line"], finding["column"]) for finding in payload["findings"]
    ] == expected_locations
    assert all(finding["code"] == test_case.rule_code for finding in payload["findings"])
    assert all(finding["path"] == "models/orders.sql" for finding in payload["findings"])


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
