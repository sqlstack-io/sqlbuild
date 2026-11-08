"""E2E coverage that model rule findings point at authored file positions, not query ones."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import (
    RulePositionE2ETestCase,
    SyntaxTreeRuleE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import (
    custom_rule_diagnostics,
    custom_rule_source,
    run_compile_cli,
    write_custom_rule_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_HEADER: str = 'MODEL (description "Test model orders.", materialized table);'
_CONTRACT_HEADER: str = (
    'MODEL (\n  description "Test model orders.",\n  contract enforced,\n'
    "  columns (a (type VARCHAR), b (type VARCHAR), c (type VARCHAR)),\n);\n"
)
_BLANK_TO_NULL: dict[str, str] = {
    "macros/sql/blank_to_null.sql": (
        'MACRO (description "Blank to null.", arguments (x VARCHAR), returns VARCHAR);\n\n'
        "CASE\n  WHEN TRIM(@x) = ''\n    THEN NULL\n  ELSE\n    TRIM(@x)\nEND\n"
    )
}
_TREE_RULE: str = custom_rule_source(
    code="XSQBRTREE001",
    body=(
        "    sql = ctx.sql.for_model(model).expanded\n"
        "    lines = sql.source.splitlines()\n"
        '    selects = sum(node.kind == "select" for node in sql.ast().walk())\n'
        "    nodes = (*sql.ctes(), *sql.joins(), *sql.subqueries(), *sql.star_projections())\n"
        "    return [\n"
        "        ctx.finding(\n"
        "            subject=model,\n"
        '            message=f"{node.kind}/{selects}:"\n'
        "            + lines[node.line - 1][node.column - 1 : node.column + 3],\n"
        "        )\n"
        "        for node in nodes\n"
        "    ]\n"
    ),
)


@pytest.mark.parametrize(
    "test_case",
    [
        RulePositionE2ETestCase(
            description="a query on the header's line is offset by the header",
            model_sql=f"{_HEADER} WITH o AS (SELECT 1 AS id) SELECT * FROM o\n",
            expected_location="models/orders.sql:1:97 [SQBRMODEL102]",
        ),
        RulePositionE2ETestCase(
            description="a query starting on the header's line keeps later lines unshifted",
            model_sql=f"{_HEADER}   WITH o AS (SELECT 1 AS id)\nSELECT * FROM o\n",
            expected_location="models/orders.sql:2:8 [SQBRMODEL102]",
        ),
        RulePositionE2ETestCase(
            description="a query below the header counts file lines",
            model_sql=f"{_HEADER}\n-- orders\nWITH o AS (SELECT 1 AS id) SELECT * FROM o\n",
            expected_location="models/orders.sql:3:35 [SQBRMODEL102]",
        ),
        RulePositionE2ETestCase(
            description="a star below multi-line macro expansions points at its authored line",
            model_sql=(
                f"{_HEADER}\nWITH o AS (\n  SELECT\n    @sql.blank_to_null('a') AS a,\n"
                "    @sql.blank_to_null('b') AS b\n)\nSELECT * FROM o\n"
            ),
            macro_files=_BLANK_TO_NULL,
            expected_location="models/orders.sql:7:8 [SQBRMODEL102]",
        ),
        RulePositionE2ETestCase(
            description="a star after a macro call on its line points at its authored column",
            model_sql=(
                f"{_HEADER}\nWITH o AS (SELECT 'a' AS a)\n"
                "SELECT @sql.blank_to_null('a') AS x, * FROM o\n"
            ),
            macro_files=_BLANK_TO_NULL,
            expected_location="models/orders.sql:3:38 [SQBRMODEL102]",
        ),
        RulePositionE2ETestCase(
            description="the third of three macro call outputs points at its authored line",
            model_sql=(
                f"{_CONTRACT_HEADER}\nSELECT\n  @sql.blank_to_null('a') AS a,\n"
                "  @sql.blank_to_null('b') AS b,\n  @sql.blank_to_null('c') AS c\n"
            ),
            rule="SQBRCONTRACT105",
            macro_files=_BLANK_TO_NULL,
            expected_location="models/orders.sql:10:3 [SQBRCONTRACT105]",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_query_layout_when_running_rules_then_findings_use_file_positions(
    test_case: RulePositionE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
                "[scopes]\nenforce_placement = false\n"
            ),
            "models/orders.sql": test_case.model_sql,
            **test_case.macro_files,
        },
    )

    ran: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "rules", "run", test_case.rule), project_dir=project_dir
    )

    assert test_case.expected_location in ran.stdout + ran.stderr, ran.stdout + ran.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        SyntaxTreeRuleE2ETestCase(
            description="tree nodes point at their own text in the expanded query",
            model_sql=(
                'MODEL (description "Orders");\n'
                "WITH o AS (SELECT 1 AS id)\n"
                "SELECT o.*, (SELECT 2) AS two, COUNT(*) OVER () AS n, HASH(*) AS h\n"
                "FROM o\n"
                "  JOIN o AS p ON o.id = p.id, o AS q\n"
            ),
            expected_messages=(
                "CTE/3:o AS",
                "Join/3:JOIN",
                "Star/3:HASH",
                "Star/3:o.*,",
                "Subquery/3:(SEL",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_custom_rule_over_the_syntax_tree_when_compiling_then_nodes_point_at_their_source(
    test_case: SyntaxTreeRuleE2ETestCase, tmp_path: Path
) -> None:
    write_custom_rule_project(
        project_dir=tmp_path,
        selected_rules=("XSQBRTREE001",),
        files=(("rules/tree.py", _TREE_RULE),),
    )
    (tmp_path / "models" / "orders.sql").write_text(test_case.model_sql, encoding="utf-8")

    result: subprocess.CompletedProcess[str] = run_compile_cli(tmp_path)

    assert tuple(sorted(item["message"] for item in custom_rule_diagnostics(result))) == (
        test_case.expected_messages
    ), result.stdout + result.stderr
