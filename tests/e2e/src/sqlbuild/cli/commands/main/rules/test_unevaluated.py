"""Real CLI coverage of compiler-accepted SQL beyond the Rules parser budget."""

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import (
    EvaluatedModelRulesCase,
    EvaluatedPredicateCase,
    GroupedRulesCase,
    SnowflakeBodyShapeCase,
    UnevaluatedResourceCase,
    UnevaluatedRuleCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import (
    run_unevaluated_rules_cli,
    write_unevaluated_rules_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SNOWFLAKE_RULES_PROJECT: str = (
    'name = "shop"\nadapter = "snowflake"\n\n[rules]\nselect = ["SQLCLEAN001"]\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        UnevaluatedResourceCase(
            "model guard",
            "models/staging/orders.sql",
            "MODEL (description 'Test model orders.');\nSELECT {expression} AS order_id",
            0,
        ),
        UnevaluatedResourceCase(
            "audit guard",
            "models/staging/_sqlbuild/audits/singular/orders.sql",
            "AUDIT ();\nSELECT {expression} AS order_id "
            'FROM __ref("orders") JOIN __ref("customers") USING (order_id)',
            expected_evaluated_models=2,
            extra_files=(
                (
                    "models/staging/customers.sql",
                    "MODEL (description 'Test model customers.');\nSELECT 1 AS order_id\n",
                ),
            ),
        ),
        UnevaluatedResourceCase(
            "built-in model rule beyond the parser budget",
            "models/staging/orders.sql",
            "MODEL (description 'Test model orders.');\nSELECT {expression} AS order_id",
            expected_evaluated_models=0,
            expected_reason="could not parse models/staging/orders.sql for rules",
            rule="SQBRMODEL101",
        ),
        UnevaluatedResourceCase(
            "SQL-test guard",
            "tests/unit/test_orders.sql",
            "TEST ();\nWITH __ref__orders AS (SELECT {expression} AS order_id), __expected__orders AS (SELECT 1 AS order_id) SELECT 1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_guarded_resource_when_running_rules_then_reports_failure_and_coverage(
    test_case: UnevaluatedResourceCase,
    tmp_path: Path,
) -> None:
    write_unevaluated_rules_project(
        project_dir=tmp_path,
        resource_path=test_case.path,
        resource_template=test_case.template,
        adapter=test_case.adapter,
        extra_files=test_case.extra_files,
        selected_rules=(test_case.rule,),
    )
    for _ in range(2):
        result: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
            tmp_path, "rules", "--json", "run", test_case.rule
        )
        assert result.returncode == 1, result.stdout + result.stderr
        payload: dict[str, Any] = json.loads(result.stdout)
        assert payload["unevaluated_resources"] == 1
        assert payload["evaluated_models"] == test_case.expected_evaluated_models
        (finding,) = payload["findings"]
        assert finding["code"] == "rules-unevaluated"
        assert finding["path"] == test_case.path
        assert finding["affected_rules"] == [test_case.rule]
        assert test_case.expected_reason in finding["message"]
    human: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "rules", "run", test_case.rule
    )
    assert human.returncode == 1
    assert (
        f"{test_case.expected_evaluated_models} models evaluated, 1 resources could not be evaluated"
        in human.stdout
    )
    assert "passed" not in human.stdout
    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--no-cache", "--json"
    )
    assert compiled.returncode == 1, compiled.stdout + compiled.stderr
    diagnostics: list[dict[str, Any]] = json.loads(compiled.stdout)["diagnostics"]
    assert any(
        item["code"] == "rules-unevaluated" and item["path"] == test_case.path
        for item in diagnostics
    )


@pytest.mark.parametrize(
    "test_case",
    [
        EvaluatedModelRulesCase(
            "lambda model the compiler accepts gets built-in model findings",
            "WITH items AS (SELECT [1, 2] AS quantities)\n"
            "SELECT *, list_transform(i.quantities, lambda q: q + 1) AS shipped\n"
            "FROM items AS i WHERE i.quantities[1] > 7",
            ("SQBRMODEL101", "SQBRMODEL102", "SQBRDECLARATION102"),
            "SQBRMODEL102",
            (("SQBRMODEL102", 3, 8),),
            (("SQBRMODEL102", 3, 8), ("SQBRDECLARATION102", 4, 23)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_only_polyglot_parses_when_running_rules_then_reports_model_findings(
    test_case: EvaluatedModelRulesCase,
    tmp_path: Path,
) -> None:
    write_unevaluated_rules_project(
        project_dir=tmp_path,
        resource_path="models/staging/orders.sql",
        resource_template="MODEL (description 'Test model orders.');\n" + test_case.sql,
        selected_rules=test_case.rules,
    )
    for _ in range(2):
        result: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
            tmp_path, "rules", "--json", "run", test_case.selector
        )
        assert result.returncode == 1, result.stdout + result.stderr
        payload: dict[str, Any] = json.loads(result.stdout)
        assert payload["unevaluated_resources"] == 0
        assert payload["evaluated_models"] == 1
        findings: list[tuple[str, int, int]] = [
            (finding["code"], finding["line"], finding["column"]) for finding in payload["findings"]
        ]
        assert findings == list(test_case.expected_rule_findings)
    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--no-cache", "--json"
    )
    assert compiled.returncode == 1, compiled.stdout + compiled.stderr
    diagnostics: list[tuple[str, int, int]] = [
        (item["code"], item["line"], item["column"])
        for item in json.loads(compiled.stdout)["diagnostics"]
    ]
    assert diagnostics == list(test_case.expected_compile_findings)


@pytest.mark.parametrize(
    "test_case",
    [
        EvaluatedPredicateCase(
            "prefix NOT LIKE",
            "SELECT 1 AS order_id WHERE NOT SPLIT_PART('orders:pending', ':', 2) LIKE 'pending%'",
        ),
        EvaluatedPredicateCase(
            "prefix NOT LIKE inside CASE",
            "SELECT CASE WHEN NOT SPLIT_PART('orders:pending', ':', 2) LIKE 'pending%' THEN 1 ELSE 0 END AS order_id",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_prefix_not_like_when_running_rules_then_evaluates_the_complete_model(
    test_case: EvaluatedPredicateCase,
    tmp_path: Path,
) -> None:
    write_unevaluated_rules_project(
        project_dir=tmp_path,
        resource_path="models/staging/orders.sql",
        resource_template="MODEL (description 'Test model orders.', database warehouse, schema analytics);\n"
        + test_case.sql,
        adapter="snowflake",
    )
    for _ in range(2):
        result: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
            tmp_path,
            "rules",
            "--json",
            "run",
            "SQLCLEAN035",
        )
        assert result.returncode == 0, result.stdout + result.stderr
        payload: dict[str, Any] = json.loads(result.stdout)
        assert payload["unevaluated_resources"] == 0
        assert payload["evaluated_models"] == test_case.expected_evaluated_models
        assert payload["findings"] == []
    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path,
        "compile",
        "--no-cache",
        "--json",
    )
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert json.loads(compiled.stdout)["diagnostics"] == []


@pytest.mark.parametrize(
    "test_case",
    [
        UnevaluatedRuleCase(
            "model analysis opt out", model_options="sql_analysis false", expected_exit=0
        ),
        UnevaluatedRuleCase(
            "path analysis opt out",
            configuration="\n[path_defaults.staging]\nsql_analysis = false\n",
            expected_exit=0,
        ),
        UnevaluatedRuleCase(
            "explicit scoped rule ignore",
            configuration='\n[[rules.rule_ignores]]\nrules = ["SQLCLEAN035"]\npaths = ["models/staging/orders.sql"]\nreason = "External SQL syntax"\n',
            expected_exit=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_explicit_escape_hatch_when_running_rules_then_honors_policy(
    test_case: UnevaluatedRuleCase,
    tmp_path: Path,
) -> None:
    write_unevaluated_rules_project(
        project_dir=tmp_path,
        model_options=test_case.model_options,
        configuration=test_case.configuration,
    )
    result: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "rules", "--json", "run", "SQLCLEAN035"
    )
    assert result.returncode == test_case.expected_exit, result.stdout + result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    assert payload["unevaluated_resources"] == 0
    assert payload["findings"] == []
    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--no-cache", "--json"
    )
    assert compiled.returncode == test_case.expected_exit, compiled.stdout + compiled.stderr
    assert json.loads(compiled.stdout)["diagnostics"] == []


@pytest.mark.parametrize(
    "test_case",
    [
        GroupedRulesCase(
            "compile groups two Rules",
            ("compile", "--json", "--no-cache"),
            "diagnostics",
            ("SQLCLEAN034", "SQLCLEAN035"),
        ),
        GroupedRulesCase(
            "Rules CLI groups selected family",
            ("rules", "--json", "run", "SQLCLEAN"),
            "findings",
            tuple(f"SQLCLEAN{number:03}" for number in range(1, 54)),
        ),
        GroupedRulesCase(
            "suppression precedes grouping",
            ("compile", "--json", "--no-cache"),
            "diagnostics",
            ("SQLCLEAN034",),
            '\n[[rules.rule_ignores]]\nrules = ["SQLCLEAN035"]\npaths = ["models/staging/orders.sql"]\nreason = "External SQL syntax"\n',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_one_failed_resource_and_multiple_rules_when_running_cli_then_groups_cause(
    test_case: GroupedRulesCase,
    tmp_path: Path,
) -> None:
    write_unevaluated_rules_project(
        project_dir=tmp_path,
        selected_rules=("SQLCLEAN034", "SQLCLEAN035"),
        configuration=test_case.configuration,
    )
    for _ in range(2):
        result: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
            tmp_path, *test_case.arguments
        )
        assert result.returncode == 1, result.stdout + result.stderr
        payload: dict[str, Any] = json.loads(result.stdout)
        (finding,) = payload[test_case.result_key]
        assert finding["code"] == "rules-unevaluated"
        assert finding["affected_rules"] == list(test_case.expected_rules)
        assert finding["message"].startswith(
            f"{len(test_case.expected_rules)} selected Rules could not evaluate this resource:"
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeBodyShapeCase(
            "scalar function expression body",
            (
                (
                    "functions/sql/next_quantity.sql",
                    "FUNCTION (\n  description 'Next order quantity.',\n"
                    "  arguments (quantity INTEGER),\n  returns INTEGER,\n"
                    "  database warehouse,\n  schema analytics,\n);\n\nquantity + 1\n",
                ),
                (
                    "models/orders.sql",
                    "MODEL (description 'Orders.', database warehouse, schema analytics);\n\n"
                    'SELECT __udf("next_quantity")(2) AS quantity\n',
                ),
            ),
        ),
        SnowflakeBodyShapeCase(
            "table function call outside TABLE( )",
            (
                (
                    "functions/sql/order_lines.sql",
                    "FUNCTION (\n  description 'Lines of one order.',\n"
                    "  arguments (p_order_id VARCHAR),\n  returns table (line_id VARCHAR),\n"
                    "  database warehouse,\n  schema analytics,\n);\n\n"
                    "SELECT p_order_id AS line_id\n",
                ),
                (
                    "models/orders.sql",
                    "MODEL (description 'Orders.', database warehouse, schema analytics);\n\n"
                    "WITH lines AS (\n  SELECT l.line_id\n"
                    "  FROM __table_fn(\"order_lines\")('B-1') AS l\n)\n\n"
                    "SELECT line_id FROM lines\n",
                ),
            ),
        ),
        SnowflakeBodyShapeCase(
            "quoted hook argument where a string is required",
            (
                (
                    "hooks/sql/comment_on_relation.sql",
                    "HOOK (description 'Comment on a relation.');\n\n"
                    "COMMENT ON TABLE @relation IS @'comment'\n",
                ),
                (
                    "models/orders.sql",
                    "MODEL (\n  description 'Orders.',\n  database warehouse,\n  schema analytics,\n"
                    "  post_hooks [\n"
                    '    sql("comment_on_relation", relation: "@@CTX:destination.qualified", '
                    'comment: "Order facts"),\n  ],\n);\n\nSELECT 1 AS order_id\n',
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_body_shape_when_compiling_and_formatting_then_rules_and_format_read_it(
    test_case: SnowflakeBodyShapeCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={"sqlbuild_project.toml": _SNOWFLAKE_RULES_PROJECT, **dict(test_case.files)},
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile", "--no-cache")
    )
    formatted: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "format", "--check")
    )

    assert compiled.returncode == test_case.expected_compile_exit, compiled.stdout + compiled.stderr
    assert "rules-unevaluated" not in compiled.stdout + compiled.stderr
    assert formatted.returncode == test_case.expected_format_exit, (
        formatted.stdout + formatted.stderr
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
