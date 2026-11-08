"""Integration coverage for compiler-integrated built-in and custom rules."""

from __future__ import annotations

import json
import sys
import time
from collections.abc import Callable
from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast

import pytest
from _pytest.capture import CaptureResult

import sqlbuild._native as native_module
import sqlbuild.compiler.compile._helpers.render.user_macros as macros_module
from sqlbuild._native import (
    CustomHostObserver,
    DiscoveredMacroFile,
    LoadedMacro,
    rules_bulk_cache_path,
)
from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    CustomHostFailureIntegrationTestCase,
    CustomHostInterruptIntegrationTestCase,
    CustomHostOutcomeTestCase,
    CustomHostSplitIntegrationTestCase,
    CustomRuleCacheSequenceTestCase,
    CustomRuleSurfaceTestCase,
    DynamicPivotRulesIntegrationTestCase,
    EarlyLintInputsTestCase,
    EmptyRuleSelectionTestCase,
    ExplicitContractOutputRuleIntegrationTestCase,
    ImplicitAliasRuleIntegrationTestCase,
    NumericRangeDecisionIntegrationTestCase,
    ProjectRuleScratchStateTestCase,
    RulePassIntegrationTestCase,
    RulesIntegrationTestCase,
    SetOperationStarRuleTestCase,
    SqlRuleCacheSequenceTestCase,
    TypedContractRuleIntegrationTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    EARLY_LINT_PROJECT_FILES,
    child_processes,
    compile_finding_keys,
    compile_findings,
    interrupt_main_after,
    observe_hosts,
    write_custom_host_project,
    write_project_files,
    write_project_scratch_rules,
)

_CACHE_RULES_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN004"]\n'
)
_CACHE_THRESHOLD_TOML: str = _CACHE_RULES_TOML + "thresholds = { max_literal_length = 7 }\n"
_CACHE_SELECTION_TOML: str = _CACHE_THRESHOLD_TOML.replace(
    '"SQLCLEAN004"', '"SQLCLEAN004", "SQLCLEAN001"'
)
_CACHE_MODEL_SQL: str = 'MODEL (description "Orders");\nSELECT order_id FROM orders LIMIT 1\n'
_CACHE_EDITED_SQL: str = _CACHE_MODEL_SQL.replace("LIMIT 1", "LIMIT 2")
_CUSTOM_CACHE_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["XSQBRCACHE001"]\n'
)
_CUSTOM_CACHE_RULE: str = """from sqlbuild.rules import Finding, Model, RuleContext, rule

@rule(
    code="XSQBRCACHE001",
    message="Models must live in the final directory",
    remediation="Move this model beneath models/final/.",
)
def final_directory(*, model: Model, ctx: RuleContext) -> list[Finding]:
    return [] if "final" in model.path.parts else [ctx.finding(subject=model)]
"""
_CUSTOM_CACHE_EDITED_RULE: str = _CUSTOM_CACHE_RULE.replace('"final" in', '"staging" in')
_CUSTOM_CACHE_MODEL_SQL: str = 'MODEL (description "Orders");\nSELECT 1 AS order_id\n'
_CUSTOM_CACHE_EDITED_SQL: str = _CUSTOM_CACHE_MODEL_SQL.replace("SELECT 1", "SELECT 2")

_SURFACE_RULES: str = """from sqlbuild.rules import Finding, Model, Project, RuleContext, rule


def _names(models: tuple[Model, ...]) -> str:
    return ",".join(model.name for model in models)


@rule(code="XSQBRSURF001", message="model facts", remediation="Read every model fact.")
def model_facts(*, model: Model, ctx: RuleContext) -> list[Finding]:
    sql = ctx.sql.for_model(model)
    declared = ";".join(
        f"{column.name}:{column.type}:{column.nullable}:{column.description}:"
        + "/".join(
            f"{audit.definition_name}:{sorted(audit.arguments.items())}:{audit.severity}"
            for audit in column.audits
        )
        + f":{column.location.path.as_posix()}:{column.location.line}"
        for column in ctx.columns.declared(model)
    )
    inferred = ctx.columns.inferred(model) or ()
    parts = (
        model.name,
        model.path.as_posix(),
        str(model.materialization),
        str(len(sql.authored.source)),
        str(len(sql.expanded.source)),
        _names(ctx.graph.dependencies(model)),
        _names(ctx.graph.dependents(model)),
        declared,
        ";".join(f"{c.name}:{c.type}:{c.nullability.value}" for c in inferred),
        ",".join(ctx.columns.names(model) or ()),
        str(ctx.contracts.enforced(model)),
        ",".join(ctx.contracts.grain(model)),
        ",".join(test.name for test in ctx.tests.for_model(model)),
        ",".join(audit.name for audit in ctx.audits.for_model(model)),
    )
    return [ctx.finding(subject=model, message="|".join(parts))]


@rule(code="XSQBRSURF002", message="project facts", remediation="Read every project fact.")
def project_facts(*, project: Project, ctx: RuleContext) -> list[Finding]:
    facts = ctx.project
    parts = (
        str(project.model_count),
        str(project.target_name),
        _names(facts.models),
        ",".join(source.name for source in facts.sources),
        ",".join(seed.name for seed in facts.seeds),
        ",".join(function.name for function in facts.functions),
        ",".join(test.name for test in facts.tests),
        ",".join(audit.name for audit in facts.audits),
        str(len(ctx.tests.all())),
        str(len(ctx.audits.all())),
        ",".join(item.name for item in ctx.declarations.public_enums),
        ",".join(item.name for item in ctx.declarations.public_constants),
        str(len(ctx.declarations.enums)),
        str(len(ctx.declarations.constants)),
        _names(facts.tree.resources_under("models/marts")),
    )
    return [ctx.finding(subject=facts.models[0], message="|".join(parts))]
"""
_SURFACE_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["XSQBRSURF"]\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n"
    ),
    "rules/surface.py": _SURFACE_RULES,
    "sources/raw.yml": (
        "sources:\n  - name: raw_orders\n    description: Raw orders.\n"
        '    expression: "(SELECT 1 AS id, CAST(10.5 AS DOUBLE) AS amount)"\n'
    ),
    "seeds/regions.csv": "region_id,name\n1,north\n",
    "seeds/regions.yml": (
        "seeds:\n  - name: regions\n    description: Regions.\n    columns:\n"
        "      - name: region_id\n        type: INTEGER\n      - name: name\n        type: VARCHAR\n"
    ),
    "models/staging/_sqlbuild/_constants/policy.sql": (
        "CONSTANT (name minimum_amount, value 7);\n"
    ),
    "models/staging/_sqlbuild/_enums/order_status.sql": (
        "ENUM (name order_status, members [PAID, OPEN]);\n"
    ),
    "models/staging/stg_orders.sql": (
        "MODEL (description 'Staged orders.', contract enforced, columns (\n"
        "  order_id (type INTEGER, nullable false, description 'Order key',\n"
        "    audits [not_null, accepted_values (values [1, 2], severity warn)]),\n"
        "  amount (type DOUBLE),\n  status (type order_status),\n));\n\n"
        'SELECT CAST(id AS INTEGER) AS order_id, amount, @enum("order_status").PAID AS status\n'
        'FROM __source("raw_orders") WHERE amount > @const("minimum_amount")\n'
    ),
    "models/marts/order_totals.sql": (
        'MODEL (description "Order totals", materialized table);\n\n'
        'SELECT o.order_id, o.amount * 2 AS doubled_amount FROM __ref("stg_orders") AS o\n'
    ),
    "tests/unit/test_order_totals.sql": (
        "TEST ();\n\nWITH\n__ref__stg_orders AS (SELECT 1 AS order_id, 10.0 AS amount),\n"
        "__expected__order_totals AS (SELECT 1 AS order_id, 20.0 AS doubled_amount)\n"
        "SELECT 1\n"
    ),
}
_SURFACE_STAGING_EDIT: dict[str, str] = {
    "models/staging/stg_orders.sql": _SURFACE_FILES["models/staging/stg_orders.sql"].replace(
        "AS order_id, amount,", "AS order_id,\n  amount,"
    )
}

_EARLY_LINT_SLOW_HOOKS: int = 8
_EARLY_LINT_SLOW_EXPANSION_S: float = 0.5
_EARLY_LINT_EXPANSION_DELAYS_S: dict[str, float] = {"orders": 0.0}
_EARLY_LINT_FINDINGS: tuple[tuple[str, str, int | None, int | None], ...] = (
    ("SQLCLEAN005", "models/order_cents.sql", 2, 119),
)


@pytest.mark.parametrize(
    "test_case",
    [
        RulePassIntegrationTestCase(
            description="conflicting model metadata reports independent boundaries",
            expected_exit_code=1,
            expected_diagnostics=(
                "SQBRMODEL103",
                "SQBRMODEL104",
                "SQBRPROJECT101",
                "SQBRPROJECT104",
                "SQBRPROJECT105",
                "SQBRPROJECT106",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_conflicting_model_layer_metadata_when_compiling_then_rules_report_each_boundary(
    test_case: RulePassIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Exercise model-header provenance and layer alignment through the real compiler."""
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "commerce"\nadapter = "duckdb"\n\n'
        "[rules]\n"
        'select = ["SQBRMODEL103", "SQBRMODEL104", "SQBRPROJECT101", '
        '"SQBRPROJECT104", "SQBRPROJECT105", "SQBRPROJECT106"]\n',
        encoding="utf-8",
    )
    model_dir: Path = tmp_path / "models" / "commerce" / "intermediate" / "enriched"
    model_dir.mkdir(parents=True)
    (model_dir / "commerce__mart_v__int_v_orders.sql").write_text(
        "MODEL (\n"
        '  description "Published orders",\n'
        "  materialized table,\n"
        "  schema staging,\n"
        ");\n\n"
        'SELECT order_id FROM __ref("commerce__mart__stg_orders")\n',
        encoding="utf-8",
    )
    published_dir: Path = tmp_path / "models" / "commerce" / "mart"
    published_dir.mkdir()
    (published_dir / "commerce__mart__stg_orders.sql").write_text(
        'MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8"
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], result["diagnostics"])
    codes: set[object] = {diagnostic["code"] for diagnostic in diagnostics}

    assert exit_code == test_case.expected_exit_code
    assert set(test_case.expected_diagnostics) <= codes


@pytest.mark.parametrize(
    "test_case",
    [
        RulesIntegrationTestCase(
            description="live exact edge passes and stale edge fails",
            expected_exit_code=0,
            expected_code="SQBRGRAPH101",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_exact_internal_edge_exception_when_compiling_then_live_edge_passes_and_stale_fails(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Exercise exact graph exceptions and stale detection through TOML configuration."""
    project_file: Path = tmp_path / "sqlbuild_project.toml"
    base_config = 'name = "commerce"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRGRAPH101"]\n\n'
    live_exception = (
        "[[rules.graph_edge_exceptions]]\n"
        'consumer = "commerce__stg__orders"\n'
        'dependency = "commerce__int_enriched__orders"\n'
        'reason = "Tracked cleanup"\n'
    )
    project_file.write_text(base_config + live_exception, encoding="utf-8")
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "commerce__stg__orders.sql").write_text(
        'MODEL (description "Test model commerce__stg__orders.");\nSELECT order_id FROM __ref("commerce__int_enriched__orders")\n',
        encoding="utf-8",
    )
    (models / "commerce__int_enriched__orders.sql").write_text(
        "MODEL (description 'Test model commerce__int_enriched__orders.');\nSELECT 1 AS order_id\n",
        encoding="utf-8",
    )
    (models / "commerce__mart__summary.sql").write_text(
        "MODEL (description 'Test model commerce__mart__summary.');\nSELECT 1 AS order_count\n",
        encoding="utf-8",
    )

    live_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    live_result: dict[str, object] = json.loads(capsys.readouterr().out)
    focused_exit: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "compile",
            "--json",
            "--no-cache",
            "--select",
            "commerce__mart__summary",
        ]
    )
    focused_result: dict[str, object] = json.loads(capsys.readouterr().out)
    project_file.write_text(
        base_config
        + live_exception.replace(
            'dependency = "commerce__int_enriched__orders"',
            'dependency = "commerce__int_clean__missing"',
        ),
        encoding="utf-8",
    )
    stale_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    stale_result: dict[str, object] = json.loads(capsys.readouterr().out)

    assert live_exit == test_case.expected_exit_code
    assert not cast(list[object], live_result["diagnostics"])
    assert focused_exit == 0
    assert not cast(list[object], focused_result["diagnostics"])
    assert stale_exit == 1
    assert any(
        diagnostic["code"] == test_case.expected_code and "stale" in str(diagnostic["message"])
        for diagnostic in cast(list[dict[str, object]], stale_result["diagnostics"])
    )


@pytest.mark.parametrize(
    "test_case",
    [
        NumericRangeDecisionIntegrationTestCase(
            description="literal between bounds and value list are reported",
            predicate=(
                "TRY_CAST(code AS INTEGER) BETWEEN 30 AND 61\n"
                "  OR TRY_CAST(code AS INTEGER) NOT IN (30, 31, 45)"
            ),
            expected_findings=(
                (
                    "non-canonical numeric comparison: TRY_CAST(code AS INTEGER) BETWEEN 30 AND 61",
                    "Declare the threshold as a CONSTANT and compare through "
                    '@const("<name>"); only -1, 0, and 1 are self-explanatory.',
                ),
                (
                    "non-canonical numeric comparison: "
                    "TRY_CAST(code AS INTEGER) NOT IN (30, 31, 45)",
                    "Declare the values as a list or set CONSTANT and compare through "
                    '[NOT] IN @const("<name>"); only -1, 0, and 1 are self-explanatory.',
                ),
            ),
            expected_exit_code=1,
        ),
        NumericRangeDecisionIntegrationTestCase(
            description="constant-backed between bounds and value list pass",
            predicate=(
                'TRY_CAST(code AS INTEGER) BETWEEN @const("_low_code") AND @const("_high_code")\n'
                '  OR TRY_CAST(code AS INTEGER) NOT IN @const("_held_codes")'
            ),
            expected_findings=(),
            expected_exit_code=0,
            constants="  constants (_low_code 30, _high_code 61, _held_codes [30, 31, 45]),\n",
        ),
        NumericRangeDecisionIntegrationTestCase(
            description="canonical between bounds and value list pass",
            predicate=(
                "TRY_CAST(code AS INTEGER) BETWEEN 0 AND 1\n"
                "  OR TRY_CAST(code AS INTEGER) IN (-1, 0, 1)"
            ),
            expected_findings=(),
            expected_exit_code=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_numeric_range_or_value_list_when_compiling_then_literal_decisions_are_reported(
    test_case: NumericRangeDecisionIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """BETWEEN bounds and IN members follow the same named-decision rule as comparisons."""
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRDECLARATION102"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (\n"
        '  description "Orders",\n' + test_case.constants + ");\n\n"
        "SELECT code\n"
        "FROM (SELECT '31' AS code) AS items\n"
        f"WHERE {test_case.predicate}\n",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], result["diagnostics"])

    assert exit_code == test_case.expected_exit_code
    assert tuple((diagnostic["message"], diagnostic["help"]) for diagnostic in diagnostics) == (
        test_case.expected_findings
    )


@pytest.mark.parametrize(
    "test_case",
    [RulePassIntegrationTestCase("constant-backed numeric decision", 0, ())],
    ids=lambda case: case.description,
)
def test_given_constant_backed_decision_and_numeric_alias_when_compiling_then_rule_passes(
    test_case: RulePassIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRDECLARATION102"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        """MODEL (description "Test model orders.",
  constants (_large_batch 7),
);

SELECT
  CASE
    WHEN item_count > @const("_large_batch") THEN 'tier 7'
    ELSE 'small'
  END AS batch_size_7
FROM (SELECT 8 AS item_count) AS items
""",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)

    assert exit_code == test_case.expected_exit_code
    assert tuple(diagnostic["code"] for diagnostic in result["diagnostics"]) == (
        test_case.expected_diagnostics
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ExplicitContractOutputRuleIntegrationTestCase(
            description="proven direct output passes",
            query_sql=(
                "WITH typed AS (SELECT CAST(1 AS INTEGER) AS order_id),\n"
                "final AS (SELECT order_id FROM typed)\n"
                "SELECT order_id FROM final\n"
            ),
            expected_exit_code=0,
            expected_rule_findings=0,
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="calculated output without outer cast fails",
            query_sql=(
                "WITH typed AS (SELECT CAST(1 AS INTEGER) AS order_id),\n"
                "final AS (SELECT order_id + 1 AS order_id FROM typed)\n"
                "SELECT order_id FROM final\n"
            ),
            expected_exit_code=1,
            expected_rule_findings=1,
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="calculated output with outer cast passes",
            query_sql=(
                "WITH typed AS (SELECT CAST(1 AS INTEGER) AS order_id),\n"
                "final AS (SELECT CAST(order_id + 1 AS INTEGER) AS order_id FROM typed)\n"
                "SELECT order_id FROM final\n"
            ),
            expected_exit_code=0,
            expected_rule_findings=0,
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="unproven direct output fails",
            query_sql="SELECT order_id FROM orders\n",
            expected_exit_code=1,
            expected_rule_findings=1,
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="set branch cast to a different contract type fails",
            query_sql=(
                "SELECT CAST(1 AS INTEGER) AS order_id\n"
                "UNION ALL\n"
                "SELECT CAST(2 AS BIGINT) AS order_id\n"
            ),
            expected_exit_code=1,
            expected_rule_findings=1,
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="contract declarations map by output name",
            query_sql="SELECT 'ready' AS label, CAST(1 AS INTEGER) AS order_id\n",
            expected_exit_code=0,
            expected_rule_findings=0,
            columns_sql=(
                'order_id (type INTEGER),\n    label (description "Current processing state")'
            ),
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="final CTE maps contract declarations by output name",
            query_sql=(
                "WITH final AS (\n"
                "  SELECT 'ready' AS label, CAST(1 AS INTEGER) AS order_id\n"
                ")\n"
                "SELECT label, order_id FROM final\n"
            ),
            expected_exit_code=0,
            expected_rule_findings=0,
            columns_sql=(
                'order_id (type INTEGER),\n    label (description "Current processing state")'
            ),
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="terminal subset ignores extra final CTE outputs",
            query_sql=(
                "WITH final AS (\n"
                "  SELECT CAST(1 AS INTEGER) AS order_id, "
                "ignored_count + 1, 'unused' AS ignored_label, "
                "CAST(12.50 AS DECIMAL(18, 2)) AS amount\n"
                ")\n"
                "SELECT order_id AS id, amount AS total FROM final\n"
            ),
            expected_exit_code=0,
            expected_rule_findings=0,
            columns_sql="id (type INTEGER),\n    total (type DECIMAL(18, 2))",
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="terminal subset checks selected final CTE outputs",
            query_sql=(
                "WITH final AS (\n"
                "  SELECT CAST(1 AS INTEGER) AS order_id, "
                "1 AS ignored_count, 'unused' AS ignored_label, 12.50 + 1 AS amount\n"
                ")\n"
                "SELECT order_id AS id, amount AS total FROM final\n"
            ),
            expected_exit_code=1,
            expected_rule_findings=1,
            columns_sql="id (type INTEGER),\n    total (type DECIMAL(18, 2))",
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="terminal subset checks right by-name branch outputs",
            query_sql=(
                "WITH final AS (\n"
                "  SELECT CAST(1 AS INTEGER) AS ignored\n"
                "  UNION ALL BY NAME\n"
                "  SELECT 2 + 1 AS amount\n"
                ")\n"
                "SELECT amount AS total FROM final\n"
            ),
            expected_exit_code=1,
            expected_rule_findings=1,
            columns_sql="total (type INTEGER)",
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="terminal subset checks explicit final CTE column aliases",
            query_sql=(
                "WITH final(order_id, ignored) AS (\n"
                "  SELECT 1 + 1 AS raw_id, 3 AS extra\n"
                ")\n"
                "SELECT order_id AS id FROM final\n"
            ),
            expected_exit_code=1,
            expected_rule_findings=1,
            columns_sql="id (type INTEGER)",
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="uncast set branches fail",
            query_sql=(
                "WITH final AS (\n"
                "  SELECT 1 AS order_id\n"
                "  UNION ALL\n"
                "  SELECT 2 AS order_id\n"
                ")\n"
                "SELECT order_id FROM final\n"
            ),
            expected_exit_code=1,
            expected_rule_findings=2,
        ),
        ExplicitContractOutputRuleIntegrationTestCase(
            description="set branches require explicit casts",
            query_sql=(
                "WITH final AS (\n"
                "  SELECT CAST(1 AS INTEGER) AS order_id\n"
                "  UNION ALL\n"
                "  SELECT CAST(2 AS INTEGER) AS order_id\n"
                ")\n"
                "SELECT order_id FROM final\n"
            ),
            expected_exit_code=0,
            expected_rule_findings=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_enforced_contract_when_compiling_explicit_output_rule_then_enforces_boundary(
    test_case: ExplicitContractOutputRuleIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRCONTRACT105"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (description 'Test model orders.',\n"
        "  contract enforced,\n"
        f"  columns ({test_case.columns_sql}),\n"
        ");\n\n"
        f"{test_case.query_sql}",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], result["diagnostics"])
    diagnostic_codes: tuple[object, ...] = tuple(item["code"] for item in diagnostics)

    assert exit_code == test_case.expected_exit_code
    assert diagnostic_codes.count("SQBRCONTRACT105") == test_case.expected_rule_findings


@pytest.mark.parametrize(
    "test_case",
    (
        DynamicPivotRulesIntegrationTestCase(
            description="proven family governs only output wildcards",
            expected_valid_exit_code=0,
            expected_invalid_exit_code=1,
            expected_invalid_model_findings=1,
            expected_invalid_sql_findings=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_proven_dynamic_pivot_when_compiling_contract_rules_then_wildcard_is_governed(
    test_case: DynamicPivotRulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n\n[rules]\n'
        'select = ["SQBRMODEL102", "SQLCLEAN021", "SQBRCONTRACT101", '
        '"SQBRCONTRACT105", "SQBRCONTRACT106"]\n\n'
        "[defaults]\n"
        'contract = "enforced"\n',
        encoding="utf-8",
    )
    models_dir: Path = tmp_path / "models"
    models_dir.mkdir()
    (models_dir / "stg_order_amounts.sql").write_text(
        "MODEL (description 'Test model stg_order_amounts.', database analytics, schema analytics, columns (customer_id (type INTEGER), "
        "category (type VARCHAR), "
        'amount (type "DECIMAL(12,2)")));\n'
        "SELECT CAST(1 AS INTEGER) AS customer_id, "
        "CAST('books' AS VARCHAR) AS category, "
        "CAST(10.25 AS DECIMAL(12,2)) AS amount\n",
        encoding="utf-8",
    )
    dynamic_model: Path = models_dir / "customer_category_amounts.sql"
    dynamic_model.write_text(
        "MODEL (description 'Test model customer_category_amounts.',\n"
        "  database analytics,\n"
        "  schema analytics,\n"
        "  columns (customer_id (type INTEGER)),\n"
        "  dynamic_columns (category_amounts (pivot_column category, value_column amount, "
        'aggregate MAX, type "DECIMAL(12,2)")),\n'
        ");\n"
        "WITH pivot_input AS (\n"
        "  SELECT customer_id, category, amount\n"
        '  FROM __ref("stg_order_amounts")\n'
        ")\n"
        "SELECT *\n"
        "FROM pivot_input PIVOT(MAX(amount) FOR category IN (ANY ORDER BY category))\n",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], result["diagnostics"])

    assert exit_code == test_case.expected_valid_exit_code
    assert diagnostics == []

    dynamic_model.write_text(
        dynamic_model.read_text(encoding="utf-8").replace(
            ")\nSELECT *\nFROM pivot_input PIVOT",
            "),\nunrelated AS (SELECT * FROM pivot_input)\nSELECT *\nFROM pivot_input PIVOT",
        ),
        encoding="utf-8",
    )
    invalid_exit_code: int = main(
        ["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"]
    )
    invalid_result: dict[str, object] = json.loads(capsys.readouterr().out)
    invalid_diagnostics: list[dict[str, object]] = cast(
        list[dict[str, object]], invalid_result["diagnostics"]
    )
    invalid_codes: tuple[object, ...] = tuple(item["code"] for item in invalid_diagnostics)

    assert invalid_exit_code == test_case.expected_invalid_exit_code
    assert invalid_codes.count("SQBRMODEL102") == test_case.expected_invalid_model_findings
    assert invalid_codes.count("SQLCLEAN021") == test_case.expected_invalid_sql_findings


@pytest.mark.parametrize(
    "test_case",
    (
        TypedContractRuleIntegrationTestCase(
            description="enforced contract with an untyped column fails only the typed-column rule",
            columns_sql='order_id (type INTEGER), status (description "Current status")',
            expected_exit_code=1,
            expected_contract_101_findings=0,
            expected_contract_105_findings=0,
            expected_contract_106_findings=1,
        ),
        TypedContractRuleIntegrationTestCase(
            description="enforced contract with fully typed explicit outputs passes",
            columns_sql="order_id (type INTEGER), status (type VARCHAR)",
            expected_exit_code=0,
            expected_contract_101_findings=0,
            expected_contract_105_findings=0,
            expected_contract_106_findings=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_contract_rules_when_compiling_then_contract_is_enforced_typed_and_explicit(
    test_case: TypedContractRuleIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\n'
        'select = ["SQBRCONTRACT101", "SQBRCONTRACT105", "SQBRCONTRACT106"]\n\n'
        '[defaults]\ncontract = "enforced"\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (description 'Test model orders.',\n"
        f"  columns ({test_case.columns_sql}),\n"
        ");\n\n"
        "SELECT CAST(1 AS INTEGER) AS order_id, CAST('ready' AS VARCHAR) AS status\n",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], result["diagnostics"])
    diagnostic_codes: tuple[object, ...] = tuple(item["code"] for item in diagnostics)

    assert exit_code == test_case.expected_exit_code
    assert diagnostic_codes.count("SQBRCONTRACT101") == (test_case.expected_contract_101_findings)
    assert diagnostic_codes.count("SQBRCONTRACT105") == (test_case.expected_contract_105_findings)
    assert diagnostic_codes.count("SQBRCONTRACT106") == (test_case.expected_contract_106_findings)


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("selected SQL rule blocks artifacts", 1, "SQLCLEAN004")],
    ids=lambda case: case.description,
)
def test_given_selected_sql_rule_when_compiling_then_authored_diagnostic_blocks_artifacts(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN004"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders");\nSELECT order_id FROM orders LIMIT 1\n',
        encoding="utf-8",
    )

    first_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--manifest"])
    first: dict[str, object] = json.loads(capsys.readouterr().out)
    second_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--manifest"])
    second: dict[str, object] = json.loads(capsys.readouterr().out)
    model.write_text(
        'MODEL (description "Orders");\nSELECT order_id FROM orders ORDER BY order_id LIMIT 1\n',
        encoding="utf-8",
    )
    third_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--manifest"])
    third: dict[str, object] = json.loads(capsys.readouterr().out)

    assert first_exit == second_exit == test_case.expected_exit_code
    diagnostics: object = first["diagnostics"]
    assert isinstance(diagnostics, list)
    assert diagnostics[0]["code"] == test_case.expected_code
    assert diagnostics[0]["path"] == "models/orders.sql"
    assert second["compile_timings"]["rule_cache_hits"] == 1
    assert third_exit == 0
    assert third["compile_timings"]["rule_cache_misses"] == 1
    assert third["diagnostics"] == []
    assert (tmp_path / "target" / "manifest.json").is_file()


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("SQL rule cache follows the native build", 1, "SQLCLEAN004")],
    ids=lambda case: case.description,
)
def test_given_sql_rule_cache_from_another_native_build_when_compiling_then_rules_rerun(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["{test_case.expected_code}"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders");\nSELECT order_id FROM orders LIMIT 1\n',
        encoding="utf-8",
    )
    command: list[str] = ["--project-dir", str(tmp_path), "compile", "--json"]

    cold_exit: int = main(command)
    cold: dict[str, object] = json.loads(capsys.readouterr().out)
    cache_path: Path = rules_bulk_cache_path(project_dir=tmp_path, file_name="sql.json")
    stored: dict[str, object] = json.loads(cache_path.read_text(encoding="utf-8"))
    stored["build_identity"] = f"{stored['build_identity']}-rebuilt"
    cache_path.write_text(json.dumps(stored), encoding="utf-8")
    rebuilt_exit: int = main(command)
    rebuilt: dict[str, object] = json.loads(capsys.readouterr().out)
    warm_exit: int = main(command)
    warm: dict[str, object] = json.loads(capsys.readouterr().out)

    assert cold_exit == rebuilt_exit == warm_exit == test_case.expected_exit_code
    rebuilt_timings: dict[str, int] = cast(dict[str, int], rebuilt["compile_timings"])
    warm_timings: dict[str, int] = cast(dict[str, int], warm["compile_timings"])
    assert (rebuilt_timings["rule_cache_hits"], rebuilt_timings["rule_cache_misses"]) == (0, 1)
    assert (warm_timings["rule_cache_hits"], warm_timings["rule_cache_misses"]) == (1, 0)
    assert [item["code"] for item in cast(list[dict[str, object]], rebuilt["diagnostics"])] == [
        test_case.expected_code
    ]
    assert rebuilt["diagnostics"] == cold["diagnostics"]


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("warm SQL rules track non-model file edits", 1, "SQLCLEAN004")],
    ids=lambda case: case.description,
)
def test_given_cached_sql_test_rules_when_test_or_macro_changes_then_findings_are_current(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["{test_case.expected_code}"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    tests_dir: Path = tmp_path / "tests" / "unit"
    macro: Path = tests_dir / "_macros" / "python" / "row_limit.py"
    macro.parent.mkdir(parents=True)
    macro.write_text('def row_limit() -> str:\n    return "LIMIT 1"\n', encoding="utf-8")
    plain: Path = tests_dir / "test_plain_orders.sql"
    plain_body: str = (
        "TEST ();\nWITH\n__ref__orders AS (SELECT 1 AS order_id),\n"
        "__expected__orders AS (SELECT order_id FROM __ref__orders {limit})\n"
        "SELECT 1\n"
    )
    plain.write_text(plain_body.format(limit="LIMIT 1"), encoding="utf-8")
    (tests_dir / "test_macro_orders.sql").write_text(
        plain_body.format(limit="@py.row_limit()"), encoding="utf-8"
    )
    plain_finding: str = f"tests/unit/test_plain_orders.sql:{test_case.expected_code}"
    macro_finding: str = f"tests/unit/test_macro_orders.sql:{test_case.expected_code}"

    cold: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)
    warm: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)
    plain.write_text(plain_body.format(limit="ORDER BY order_id LIMIT 1"), encoding="utf-8")
    after_test_edit: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)
    macro.write_text(
        'def row_limit() -> str:\n    return "ORDER BY order_id LIMIT 1"\n', encoding="utf-8"
    )
    after_macro_edit: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)

    assert {plain_finding, macro_finding} <= cold
    assert warm == cold
    assert plain_finding not in after_test_edit
    assert macro_finding in after_test_edit
    assert macro_finding not in after_macro_edit
    assert rules_bulk_cache_path(project_dir=tmp_path, file_name="sql.json").is_file()


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("warm SQL rules reuse compiler declaration scope", 1, "SQLCLEAN004")],
    ids=lambda case: case.description,
)
def test_given_warm_scoped_macro_sql_test_when_compiling_then_rules_reuse_compiler_scope(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["{test_case.expected_code}"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    tests_dir: Path = tmp_path / "tests" / "unit"
    macro: Path = tests_dir / "_macros" / "python" / "row_limit.py"
    macro.parent.mkdir(parents=True)
    macro.write_text('def row_limit() -> str:\n    return "LIMIT 1"\n', encoding="utf-8")
    (tests_dir / "test_macro_orders.sql").write_text(
        "TEST ();\nWITH\n__ref__orders AS (SELECT 1 AS order_id),\n"
        "__expected__orders AS (SELECT order_id FROM __ref__orders @py.row_limit())\n"
        "SELECT 1\n",
        encoding="utf-8",
    )
    macro_finding: str = f"tests/unit/test_macro_orders.sql:{test_case.expected_code}"
    cold: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)
    _ = native_module.compile_stage_work_info(reset=True)

    warm: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)

    assert macro_finding in cold
    assert warm == cold
    assert native_module.compile_stage_work_info()["build_declaration_scope"] == 1


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("cold SQL rules rebind the compiler scope index", 1, "SQLCLEAN004")],
    ids=lambda case: case.description,
)
def test_given_cold_scoped_macro_sql_test_when_compiling_then_early_lint_uses_private_macros(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["{test_case.expected_code}"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    tests_dir: Path = tmp_path / "tests" / "unit"
    macro: Path = tests_dir / "_macros" / "python" / "row_limit.py"
    macro.parent.mkdir(parents=True)
    macro.write_text('def row_limit() -> str:\n    return "LIMIT 1"\n', encoding="utf-8")
    (tests_dir / "test_macro_orders.sql").write_text(
        "TEST ();\nWITH\n__ref__orders AS (SELECT 1 AS order_id),\n"
        "__expected__orders AS (SELECT order_id FROM __ref__orders @py.row_limit())\n"
        "SELECT 1\n",
        encoding="utf-8",
    )
    macro_loads: list[dict[str, LoadedMacro]] = []
    original_load: Callable[..., dict[str, LoadedMacro]] = macros_module.load_project_macros

    def recording_load(macro_files: tuple[DiscoveredMacroFile, ...]) -> dict[str, LoadedMacro]:
        loaded: dict[str, LoadedMacro] = original_load(macro_files)
        macro_loads.append(loaded)
        return loaded

    monkeypatch.setattr(macros_module, "load_project_macros", recording_load)
    _ = native_module.compile_stage_work_info(reset=True)

    cold: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)

    assert f"tests/unit/test_macro_orders.sql:{test_case.expected_code}" in cold
    assert native_module.compile_stage_work_info()["build_declaration_scope"] == 1
    assert len(macro_loads) == 2
    assert macro_loads[0]["row_limit"].function is not macro_loads[1]["row_limit"].function


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("early file read gives the cached findings", 1, "SQLCLEAN004")],
    ids=lambda case: case.description,
)
def test_given_cold_non_model_sql_finding_when_compiling_twice_then_warm_findings_match(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["{test_case.expected_code}"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    test_file: Path = tmp_path / "tests" / "unit" / "test_orders.sql"
    test_file.parent.mkdir(parents=True)
    test_file.write_bytes(
        b"TEST ();\r\nWITH\r\n__ref__orders AS (SELECT 1 AS order_id),\r\n"
        b"__expected__orders AS (SELECT order_id FROM __ref__orders LIMIT 1)\r\n"
        b"SELECT 1\r\n"
    )
    cold: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)
    warm: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)

    assert f"tests/unit/test_orders.sql:{test_case.expected_code}" in cold
    assert warm == cold


@pytest.mark.parametrize(
    "test_case",
    [
        EarlyLintInputsTestCase(
            description="compile lints every model from the compile's expansion",
            expected_findings=_EARLY_LINT_FINDINGS,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_variable_and_macro_models_when_compiling_then_early_lint_findings_match(
    test_case: EarlyLintInputsTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_project_files(project_dir=tmp_path, files=EARLY_LINT_PROJECT_FILES)
    findings: tuple[tuple[str, str, int | None, int | None], ...] = compile_findings(
        project_dir=tmp_path, capsys=capsys
    )

    assert findings == test_case.expected_findings


@pytest.mark.parametrize(
    "test_case",
    [
        EarlyLintInputsTestCase(
            description="old build entries leave early lint enabled",
            expected_findings=_EARLY_LINT_FINDINGS,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_previous_build_rule_cache_when_compiling_then_lint_overlaps_with_same_findings(
    test_case: EarlyLintInputsTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("SQLBUILD_DISABLE_COMPILE_CACHE", "1")
    write_project_files(project_dir=tmp_path, files=EARLY_LINT_PROJECT_FILES)
    previous_findings: tuple[tuple[str, str, int | None, int | None], ...] = compile_findings(
        project_dir=tmp_path, capsys=capsys, flags=()
    )
    cache_path: Path = rules_bulk_cache_path(project_dir=tmp_path, file_name="sql.json")
    stored: dict[str, object] = json.loads(cache_path.read_text(encoding="utf-8"))
    stored["build_identity"] = "previous-build"
    cache_path.write_text(json.dumps(stored), encoding="utf-8")
    upgraded_findings: tuple[tuple[str, str, int | None, int | None], ...] = compile_findings(
        project_dir=tmp_path, capsys=capsys, flags=()
    )

    assert previous_findings == upgraded_findings == test_case.expected_findings
    assert compile_findings(project_dir=tmp_path, capsys=capsys, flags=()) == (upgraded_findings)


@pytest.mark.parametrize(
    "test_case",
    [RulePassIntegrationTestCase("graph error stops the unused early lint", 1, ("B002", "S024"))],
    ids=lambda case: case.description,
)
def test_given_graph_error_when_compiling_then_early_lint_is_stopped_instead_of_awaited(
    test_case: RulePassIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN005"]\n',
        encoding="utf-8",
    )
    models_dir: Path = tmp_path / "models"
    models_dir.mkdir()
    (models_dir / "orders.sql").write_text(
        'MODEL (description "Orders");\nSELECT @py.slow_total("orders") AS order_id\n',
        encoding="utf-8",
    )
    (models_dir / "order_totals.sql").write_text(
        'MODEL (description "Order totals");\n'
        "WITH unused_orders AS (SELECT 1 AS order_id)\n"
        'SELECT order_id, missing_column FROM __ref("orders")\n',
        encoding="utf-8",
    )
    macros_dir: Path = tmp_path / "macros" / "python"
    macros_dir.mkdir(parents=True)
    (tmp_path / "hooks").mkdir()
    (macros_dir / "slow_total.py").write_text(
        "import orders_lint_probe\n\n\n"
        "def slow_total(name: str) -> str:\n"
        '    """Record the expansion and take a while."""\n'
        "    return orders_lint_probe.expanded(name)\n",
        encoding="utf-8",
    )
    for index in range(_EARLY_LINT_SLOW_HOOKS):
        (tmp_path / "hooks" / f"total_{index:02d}.sql").write_text(
            f'SELECT @py.slow_total("total_{index:02d}") AS order_total\n', encoding="utf-8"
        )
    expanded: list[str] = []

    def expand_slowly(name: str) -> str:
        expanded.append(name)
        time.sleep(_EARLY_LINT_EXPANSION_DELAYS_S.get(name, _EARLY_LINT_SLOW_EXPANSION_S))
        return "1"

    monkeypatch.setitem(sys.modules, "orders_lint_probe", SimpleNamespace(expanded=expand_slowly))

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])

    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], payload["diagnostics"])
    assert exit_code == test_case.expected_exit_code
    assert tuple(diagnostic["code"] for diagnostic in diagnostics) == (
        test_case.expected_diagnostics
    )
    assert len(set(expanded) - set(_EARLY_LINT_EXPANSION_DELAYS_S)) < _EARLY_LINT_SLOW_HOOKS


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("custom host runs once and is skipped when warm", 1, "XSQBRARCH001")],
    ids=lambda case: case.description,
)
def test_given_sql_and_custom_rules_when_compiling_twice_then_custom_host_runs_only_cold(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n'
        f'[rules]\nselect = ["SQLCLEAN004", "{test_case.expected_code}"]\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n",
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    test_file: Path = tmp_path / "tests" / "unit" / "test_orders.sql"
    test_file.parent.mkdir(parents=True)
    test_file.write_text(
        "TEST ();\nWITH\n__ref__orders AS (SELECT 1 AS order_id),\n"
        "__expected__orders AS (SELECT order_id FROM __ref__orders LIMIT 1)\n"
        "SELECT 1\n",
        encoding="utf-8",
    )
    rule_file: Path = tmp_path / "rules" / "architecture.py"
    rule_file.parent.mkdir()
    rule_file.write_text(
        """from sqlbuild.rules import Finding, Model, RuleContext, rule

@rule(
    code="XSQBRARCH001",
    message="Final models must use the final directory",
    remediation="Move this model beneath models/final/.",
)
def final_directory(*, model: Model, ctx: RuleContext) -> list[Finding]:
    return [] if "final" in model.path.parts else [ctx.finding(subject=model)]
""",
        encoding="utf-8",
    )
    hosts: CustomHostObserver = CustomHostObserver()
    monkeypatch.setattr(native_module, "custom_host_observer", hosts)
    cold: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)

    warm: set[str] = compile_finding_keys(project_dir=tmp_path, capsys=capsys)

    assert len(hosts.launches()) == 1
    assert warm == cold
    assert {
        f"models/orders.sql:{test_case.expected_code}",
        "tests/unit/test_orders.sql:SQLCLEAN004",
    } <= cold


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectRuleScratchStateTestCase(
            description="one host runs the project rules in order",
            hosts=1,
            expected_findings=(("XSQBRSCR002", "models/customers.sql"),),
            expected_launches=1,
        ),
        ProjectRuleScratchStateTestCase(
            description="split project rules rerun in one host after the scratch list fills",
            hosts=2,
            expected_findings=(("XSQBRSCR002", "models/customers.sql"),),
            expected_launches=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_rules_sharing_scratch_state_when_splitting_hosts_then_findings_match_one_host(
    test_case: ProjectRuleScratchStateTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_project_scratch_rules(tmp_path)
    hosts: CustomHostObserver = observe_hosts(monkeypatch=monkeypatch, cores=test_case.hosts)

    findings: tuple[tuple[str, str, int | None, int | None], ...] = compile_findings(
        project_dir=tmp_path, capsys=capsys
    )

    assert tuple((code, path) for code, path, _, _ in findings) == test_case.expected_findings
    assert len(hosts.launches()) == test_case.expected_launches


@pytest.mark.parametrize(
    "test_case",
    [CustomHostSplitIntegrationTestCase("custom rules split across hosts", 9, 3, "XSQBRARCH001")],
    ids=lambda case: case.description,
)
def test_given_custom_rules_split_across_hosts_when_compiling_then_output_matches_one_host(
    test_case: CustomHostSplitIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n'
        f'[rules]\nselect = ["{test_case.expected_code}"]\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n",
        encoding="utf-8",
    )
    for index in range(test_case.model_count):
        folder: str = ("final", "staging", "staging")[index % 3]
        model: Path = tmp_path / "models" / folder / f"orders_{index:02d}.sql"
        model.parent.mkdir(parents=True, exist_ok=True)
        model.write_text(
            f'MODEL (description "Orders {index}");\nSELECT {index} AS order_id\n',
            encoding="utf-8",
        )
    rule_file: Path = tmp_path / "rules" / "architecture.py"
    rule_file.parent.mkdir()
    rule_file.write_text(
        """from sqlbuild.rules import Finding, Model, RuleContext, rule

@rule(
    code="XSQBRARCH001",
    message="Final models must use the final directory",
    remediation="Move this model beneath models/final/.",
)
def final_directory(*, model: Model, ctx: RuleContext) -> list[Finding]:
    return [] if "final" in model.path.parts else [ctx.finding(subject=model)]
""",
        encoding="utf-8",
    )

    def compile_payload() -> dict[str, Any]:
        exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
        assert exit_code == 1
        return json.loads(capsys.readouterr().out)

    monkeypatch.setenv("SQLBUILD_DISABLE_COMPILE_CACHE", "1")
    single_hosts: CustomHostObserver = observe_hosts(monkeypatch=monkeypatch, cores=1)
    single: dict[str, Any] = compile_payload()
    rules_bulk_cache_path(project_dir=tmp_path, file_name="custom-rules.json").parent.parent.rename(
        tmp_path / "single-rules-cache"
    )
    split_hosts: CustomHostObserver = observe_hosts(monkeypatch=monkeypatch, cores=test_case.hosts)
    split: dict[str, Any] = compile_payload()
    split_runs: int = len(split_hosts.launches())
    warm: dict[str, Any] = compile_payload()

    assert (len(single_hosts.launches()), split_runs, len(split_hosts.launches())) == (
        1,
        test_case.hosts,
        test_case.hosts,
    )
    assert split["diagnostics"] == single["diagnostics"]
    assert len(split["diagnostics"]) == test_case.model_count - test_case.model_count // 3
    assert warm["diagnostics"] == single["diagnostics"]
    assert warm["compile_timings"]["rule_cache_hits"] >= test_case.model_count
    assert (tmp_path / "single-rules-cache" / "bulk" / "custom-rules.json").read_bytes() == (
        rules_bulk_cache_path(project_dir=tmp_path, file_name="custom-rules.json").read_bytes()
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CustomHostFailureIntegrationTestCase(
            description="earliest failure lives on a later host",
            model_count=9,
            hosts=3,
            failing_models=("orders_004", "orders_007"),
            expected_failing_model="orders_004",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_custom_rule_on_split_hosts_when_compiling_then_one_host_failure_is_reported(
    test_case: CustomHostFailureIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_custom_host_project(
        project_dir=tmp_path,
        model_count=test_case.model_count,
        rule_body=(
            f"if model.name in {test_case.failing_models!r}:\n"
            '    raise ValueError(f"cannot inspect {model.name}")\n'
            "return []"
        ),
    )

    def failed_compile() -> tuple[int, dict[str, Any]]:
        exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
        return exit_code, json.loads(capsys.readouterr().out)

    single_hosts: CustomHostObserver = observe_hosts(monkeypatch=monkeypatch, cores=1)
    single_exit, single = failed_compile()
    split_hosts: CustomHostObserver = observe_hosts(monkeypatch=monkeypatch, cores=test_case.hosts)
    split_exit, split = failed_compile()

    assert split_exit == single_exit == 1
    assert split["diagnostics"] == single["diagnostics"]
    assert f"cannot inspect {test_case.expected_failing_model}" in json.dumps(split["diagnostics"])
    assert (len(single_hosts.launches()), len(split_hosts.launches())) == (1, test_case.hosts + 1)
    assert not tuple((tmp_path / "target" / "rules-cache" / "host-inputs").iterdir())


@pytest.mark.parametrize(
    "test_case",
    [
        CustomHostInterruptIntegrationTestCase(
            description="busy split hosts stop after an interrupt",
            model_count=45,
            hosts=3,
            interrupt_after_seconds=1.5,
            expected_max_seconds=12.0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_busy_split_hosts_when_compile_is_interrupted_then_hosts_stop_and_none_remain(
    test_case: CustomHostInterruptIntegrationTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_custom_host_project(
        project_dir=tmp_path,
        model_count=test_case.model_count,
        rule_body="total = 0\nfor index in range(10_000_000):\n    total += index\nreturn []",
    )
    children_before: frozenset[int] = child_processes()
    hosts: CustomHostObserver = observe_hosts(
        monkeypatch=monkeypatch,
        cores=test_case.hosts,
        publish_inputs=interrupt_main_after(test_case.interrupt_after_seconds),
    )
    started: float = time.monotonic()

    with pytest.raises(KeyboardInterrupt):
        _ = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])

    assert time.monotonic() - started < test_case.expected_max_seconds
    assert len(hosts.launches()) == test_case.hosts
    assert child_processes() <= children_before
    assert not tuple((tmp_path / "target" / "rules-cache" / "host-inputs").iterdir())


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("custom rule is enforced and cached", 1, "XSQBRARCH001")],
    ids=lambda case: case.description,
)
def test_given_typed_custom_rule_when_compiling_then_same_rule_is_enforced_and_cached(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n'
        '[rules]\nselect = ["XSQBRARCH001"]\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n",
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    rule_file: Path = tmp_path / "rules" / "architecture.py"
    rule_file.parent.mkdir()
    rule_file.write_text(
        """from sqlbuild.rules import Finding, Model, RuleContext, rule

@rule(
    code="XSQBRARCH001",
    message="Final models must use the final directory",
    remediation="Move this model beneath models/final/.",
)
def final_directory(*, model: Model, ctx: RuleContext) -> list[Finding]:
    return [] if "final" in model.path.parts else [ctx.finding(subject=model)]
""",
        encoding="utf-8",
    )

    first_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    first: dict[str, object] = json.loads(capsys.readouterr().out)
    second_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    second: dict[str, object] = json.loads(capsys.readouterr().out)

    assert first_exit == second_exit == test_case.expected_exit_code
    assert first["diagnostics"][0]["code"] == test_case.expected_code
    assert second["compile_timings"]["rule_cache_hits"] >= 1
    config_path: Path = tmp_path / "sqlbuild_project.toml"
    original_config: str = config_path.read_text(encoding="utf-8")
    config_path.write_text(original_config + "\n[rules.cache]\nenabled = false\n", encoding="utf-8")
    cold_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    cold: dict[str, object] = json.loads(capsys.readouterr().out)
    config_path.write_text(original_config, encoding="utf-8")
    warm_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    warm: dict[str, object] = json.loads(capsys.readouterr().out)

    assert cold_exit == warm_exit == test_case.expected_exit_code
    assert cold["diagnostics"] == warm["diagnostics"] == first["diagnostics"]
    assert cold["compile_timings"]["rule_cache_hits"] == 0
    assert warm["compile_timings"]["rule_cache_hits"] >= 1
    assert (
        (tmp_path / "target" / "compiled-failed" / "models" / "orders.sql")
        .read_text(encoding="utf-8")
        .startswith("-- sqb: compile failed for this model; not buildable\n-- XSQBRARCH001 at ")
    )
    host_inputs: Path = tmp_path / "target" / "rules-cache" / "host-inputs"
    assert not tuple(host_inputs.glob("*.payload"))


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("focused SQL rule reports requested code", 1, "SQLCLEAN004")],
    ids=lambda case: case.description,
)
def test_given_project_when_running_focused_rule_family_then_only_that_family_runs(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders");\nSELECT order_id FROM orders LIMIT 1\n',
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            "SQLCLEAN004",
        ]
    )

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert test_case.expected_code in json.dumps(payload["findings"])
    assert "Rule selection SQLCLEAN004 failed with 1 finding(s)." in captured.err


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("inline join query is rejected", 1, "SQLCLEAN039")],
    ids=lambda case: case.description,
)
def test_given_inline_query_relation_when_running_readability_rule_then_finding_is_reported(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        """MODEL (description "Orders");
WITH actual_orders AS (
  SELECT 1 AS order_id
)
SELECT COALESCE(actual.order_id, expected.order_id) AS order_id
FROM actual_orders AS actual
FULL OUTER JOIN (
  SELECT column0 AS order_id
  FROM VALUES (1), (2)
) AS expected
  ON actual.order_id = expected.order_id
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert test_case.expected_code in json.dumps(payload["findings"])
    assert "Inline query relation obscures data flow" in json.dumps(payload["findings"])


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("scalar query internals are excluded", 0, "SQLCLEAN039")],
    ids=lambda case: case.description,
)
def test_given_scalar_query_with_derived_relation_when_running_readability_rule_then_no_finding_is_reported(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "order_summary.sql"
    model.parent.mkdir()
    model.write_text(
        """MODEL (description "Order summary");
SELECT (
  SELECT derived.order_id
  FROM (
    SELECT nested.order_id
    FROM (SELECT 1 AS order_id) AS nested
  ) AS derived
) AS order_id
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert test_case.expected_code not in json.dumps(payload["findings"])


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("SQL function arguments are not columns", 0, "SQLCLEAN027")],
    ids=lambda case: case.description,
)
def test_given_sql_function_argument_when_running_qualification_rule_then_argument_is_not_reported(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    function: Path = tmp_path / "functions" / "sql" / "orders_by_date.sql"
    function.parent.mkdir(parents=True)
    function.write_text(
        """FUNCTION (description "Test function orders_by_date.",
  arguments (start_date DATE),
  returns table (order_id INTEGER)
);

SELECT current_orders.order_id
FROM current_orders
INNER JOIN order_statuses ON current_orders.order_id = order_statuses.order_id
WHERE current_orders.created_at >= start_date
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("table function argument preserves left alias", 0, "SQLCLEAN023")],
    ids=lambda case: case.description,
)
def test_given_table_function_argument_when_running_alias_rule_then_left_alias_is_preserved(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    function: Path = tmp_path / "functions" / "sql" / "expand_order.sql"
    function.parent.mkdir(parents=True)
    function.write_text(
        """FUNCTION (description "Test function expand_order.",
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
        """MODEL (description "Expanded orders");
SELECT expanded.order_id
FROM orders AS source_orders
CROSS JOIN __table_fn("expand_order")(source_orders.order_id) AS expanded
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [
        ImplicitAliasRuleIntegrationTestCase(
            description="unused implicit join alias is reported",
            query_sql=(
                "SELECT o.order_id\n"
                'FROM __ref("orders") o\n'
                'INNER JOIN __ref("customers") c ON o.customer_id = o.order_id\n'
            ),
            expected_locations=((4, 31),),
        ),
        ImplicitAliasRuleIntegrationTestCase(
            description="unused implicit subquery alias is reported",
            query_sql='SELECT order_id\nFROM (SELECT order_id FROM __ref("orders")) recent\n',
            expected_locations=((3, 45),),
        ),
        ImplicitAliasRuleIntegrationTestCase(
            description="implicit and explicit unused aliases are reported alike",
            query_sql='SELECT order_id\nFROM __ref("orders") AS o, __ref("customers") c\n',
            expected_locations=((3, 22), (3, 47)),
        ),
        ImplicitAliasRuleIntegrationTestCase(
            description="whole-row alias uses are clean",
            query_sql=(
                "SELECT to_json(o) AS payload, c\n"
                'FROM __ref("orders") o\n'
                'INNER JOIN __ref("customers") AS c ON o.customer_id = c.customer_id\n'
            ),
            expected_locations=(),
        ),
        ImplicitAliasRuleIntegrationTestCase(
            description="join modifiers after unaliased relations are clean",
            query_sql='SELECT *\nFROM __ref("orders") POSITIONAL JOIN __ref("customers")\n',
            expected_locations=(),
        ),
        ImplicitAliasRuleIntegrationTestCase(
            description="used implicit aliases are clean",
            query_sql=(
                "SELECT o.order_id, c.customer_id\n"
                'FROM __ref("orders") o\n'
                'INNER JOIN __ref("customers") c ON o.customer_id = c.customer_id\n'
            ),
            expected_locations=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_implicit_table_alias_when_running_alias_rule_then_usage_decides_finding(
    test_case: ImplicitAliasRuleIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Implicit aliases are held to the same unused-alias contract as AS aliases."""
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "orders.sql").write_text(
        'MODEL (description "Orders");\nSELECT 1 AS order_id, 2 AS customer_id\n',
        encoding="utf-8",
    )
    (models / "customers.sql").write_text(
        'MODEL (description "Customers");\nSELECT 2 AS customer_id\n', encoding="utf-8"
    )
    (models / "order_customers.sql").write_text(
        'MODEL (description "Order customers");\n' + test_case.query_sql, encoding="utf-8"
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "rules", "--json", "run", "SQLCLEAN023"])

    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    findings: list[dict[str, object]] = cast(list[dict[str, object]], payload["findings"])
    assert exit_code == int(bool(test_case.expected_locations))
    assert tuple((finding["line"], finding["column"]) for finding in findings) == (
        test_case.expected_locations
    )
    assert {finding["path"] for finding in findings} <= {"models/order_customers.sql"}


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("ceremonial SQL test passes terminal rule", 0, "SQLCLEAN035")],
    ids=lambda case: case.description,
)
def test_given_ceremonial_sql_test_when_compiling_with_rule_035_then_project_is_valid(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN035"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        """MODEL (description "Orders");
WITH final AS (SELECT 1 AS order_id)
SELECT order_id FROM final
""",
        encoding="utf-8",
    )
    test: Path = tmp_path / "tests" / "unit" / "test_orders.sql"
    test.parent.mkdir(parents=True)
    test.write_text(
        """TEST ();
WITH
__ref__orders AS (SELECT 1 AS order_id),
__expected__orders AS (SELECT 1 AS order_id)
SELECT 1
""",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["diagnostics"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("contextual keywords are plain terminal columns", 0, "SQLCLEAN035")],
    ids=lambda case: case.description,
)
def test_given_keyword_named_columns_when_running_terminal_rule_then_projection_is_accepted(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        """MODEL (description "Orders with contextual column names");
WITH final AS (
  SELECT
    CAST('2026-01-01' AS DATE) AS date,
    'ready' AS comment,
    'order-1' AS key,
    50 AS percent,
    1 AS sequence,
    CAST('12:00:00' AS TIME) AS time
)
SELECT
  date,
  comment,
  key,
  percent,
  sequence,
  time
FROM final
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [
        RulesIntegrationTestCase(
            "assertion and expected result must be independent",
            1,
            "expected results and assertions must be independent",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_assertion_expected_dependency_when_compiling_then_compiler_rejects_test(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    test: Path = tmp_path / "tests" / "unit" / "test_orders.sql"
    test.parent.mkdir(parents=True)
    test.write_text(
        """TEST ();
WITH
__ref__orders AS (SELECT 1 AS order_id),
__expected__orders AS (SELECT 1 AS order_id),
__assert__expected_is_nonempty AS (SELECT * FROM __expected__orders)
SELECT 1
""",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], payload["diagnostics"])
    assert len(diagnostics) == 1
    assert test_case.expected_code in str(diagnostics[0]["message"])
    assert test_case.expected_code in captured.err


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("non-source references are not source tokens", 0, "SQBRPROJECT103")],
    ids=lambda case: case.description,
)
def test_given_seed_and_function_references_when_running_source_rule_then_they_are_not_tokens(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\napproved_source_tokens = ["partner"]\n',
        encoding="utf-8",
    )
    seed: Path = tmp_path / "seeds" / "order_statuses.csv"
    seed.parent.mkdir()
    seed.write_text("status\nopen\n", encoding="utf-8")
    (tmp_path / "seeds" / "schema.yml").write_text(
        """seeds:
  - name: order_statuses
    description: Supported order statuses.
    columns:
      - name: status
        type: VARCHAR
""",
        encoding="utf-8",
    )
    function: Path = tmp_path / "functions" / "sql" / "normalize_quantity.sql"
    function.parent.mkdir(parents=True)
    function.write_text(
        """FUNCTION (description "Test function normalize_quantity.",
  arguments (quantity INTEGER),
  returns INTEGER,
);

quantity
""",
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "sales__stg__orders__partner.sql"
    model.parent.mkdir()
    model.write_text(
        """MODEL (description "Partner orders");
SELECT __udf("normalize_quantity")(1) AS quantity
FROM __seed("order_statuses")
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [
        RulesIntegrationTestCase(
            "dependency import star is compatible with output-shape rule",
            0,
            "SQLCLEAN021",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dependency_import_star_when_running_output_shape_rule_then_import_is_accepted(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    staging: Path = tmp_path / "models" / "stg_orders.sql"
    staging.parent.mkdir()
    staging.write_text(
        'MODEL (description "Staged orders");\nSELECT 1 AS order_id\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.write_text(
        """MODEL (description "Orders");
WITH imported_orders AS (
  SELECT *
  FROM __ref("stg_orders")
),
final AS (
  SELECT order_id
  FROM imported_orders
)
SELECT order_id
FROM final
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("invocation-dependent stars are accepted", 0, "SQLCLEAN021")],
    ids=lambda case: case.description,
)
def test_given_invocation_dependent_shapes_when_running_output_shape_rule_then_stars_are_accepted(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "order_totals.sql"
    model.parent.mkdir()
    model.write_text(
        """MODEL (
  description "Dynamic order totals",
  database analytics,
  schema reporting,
  audits [recent_orders],
);
SELECT *
FROM (
  SELECT 1 AS customer_id, 'books' AS category, 25 AS amount
) PIVOT(MAX(amount) FOR category IN (ANY ORDER BY category))
""",
        encoding="utf-8",
    )
    audit: Path = tmp_path / "audits" / "generic" / "recent_orders.sql"
    audit.parent.mkdir(parents=True)
    audit.write_text(
        """AUDIT ();
WITH base AS (
  SELECT *
  FROM @relation
)
SELECT customer_id
FROM base
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("commented dependency calls are ignored", 0, "SQBRMODEL101")],
    ids=lambda case: case.description,
)
def test_given_commented_dependency_call_when_running_import_rule_then_comment_is_ignored(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    staging: Path = tmp_path / "models" / "stg_orders.sql"
    staging.parent.mkdir()
    staging.write_text(
        'MODEL (description "Staged orders");\nSELECT 1 AS order_id\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.write_text(
        """MODEL (description "Orders");
WITH imported_orders AS (
  SELECT *
  FROM __ref("stg_orders")
),
final AS (
  SELECT order_id
  FROM imported_orders
  -- FROM __ref("retired_orders")
  /* JOIN __source("archived_orders") */
)
SELECT order_id
FROM final
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("quoted comment marker preserves real call", 1, "SQBRMODEL101")],
    ids=lambda case: case.description,
)
def test_given_bigquery_quoted_comment_marker_when_running_import_rule_then_real_call_is_detected(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "bigquery"\n', encoding="utf-8"
    )
    staging: Path = tmp_path / "models" / "stg_orders.sql"
    staging.parent.mkdir()
    staging.write_text(
        'MODEL (description "Staged orders", database warehouse, schema analytics);\n'
        "SELECT 1 AS order_id\n",
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.write_text(
        """MODEL (description "Orders", database warehouse, schema analytics);
WITH imported_orders AS (
  SELECT
    order_id,
    1 AS `--note`
  FROM __ref("stg_orders")
),
final AS (
  SELECT order_id
  FROM imported_orders
)
SELECT order_id
FROM final
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    findings: list[dict[str, object]] = cast(list[dict[str, object]], payload["findings"])
    assert [finding["message"] for finding in findings] == [
        'import CTE "imported_orders" contains transformation logic'
    ]


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("nested comment dependency call is ignored", 0, "SQBRMODEL101")],
    ids=lambda case: case.description,
)
def test_given_postgres_nested_comment_when_running_import_rule_then_comment_is_ignored(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "postgres"\n', encoding="utf-8"
    )
    staging: Path = tmp_path / "models" / "stg_orders.sql"
    staging.parent.mkdir()
    staging.write_text(
        'MODEL (description "Staged orders", schema analytics);\nSELECT 1 AS order_id\n',
        encoding="utf-8",
    )
    retired: Path = tmp_path / "models" / "retired_orders.sql"
    retired.write_text(
        'MODEL (description "Retired orders", schema analytics);\nSELECT 1 AS order_id\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.write_text(
        """MODEL (description "Orders", schema analytics);
WITH imported_orders AS (
  SELECT *
  FROM __ref("stg_orders")
),
final AS (
  SELECT order_id
  FROM imported_orders
  /* outer comment /* inner comment */ __ref("retired_orders") */
)
SELECT order_id
FROM final
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("cross join used by filter is not unused", 0, "SQLCLEAN032")],
    ids=lambda case: case.description,
)
def test_given_cross_join_used_by_filter_when_running_unused_join_rule_then_no_finding_is_reported(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "recent_orders.sql"
    model.parent.mkdir()
    model.write_text(
        """MODEL (description "Recent orders");
SELECT orders.order_id
FROM orders
CROSS JOIN processing_cutoff AS cutoff
WHERE orders.created_at < cutoff.created_at
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("ceremonial SQL test select is ignored", 0, "SQLCLEAN022")],
    ids=lambda case: case.description,
)
def test_given_ceremonial_test_select_when_running_alias_rule_then_control_projection_is_not_reported(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    test: Path = tmp_path / "tests" / "unit" / "orders_test.sql"
    test.parent.mkdir(parents=True)
    test.write_text(
        "TEST();\n\nWITH __ref__orders AS (SELECT 1 AS order_id), "
        "__expected__orders AS (SELECT 1 AS order_id)\nSELECT 1\n",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [
        RulesIntegrationTestCase(
            "direct ceremonial select satisfies terminal shape", 0, "SQLCLEAN035"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_direct_ceremonial_select_when_running_terminal_rule_then_test_is_accepted(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    test: Path = tmp_path / "tests" / "unit" / "orders_test.sql"
    test.parent.mkdir(parents=True)
    test.write_text(
        "TEST();\n\nWITH __ref__orders AS (SELECT 1 AS order_id), "
        "__expected__orders AS (SELECT 1 AS order_id)\nSELECT 1\n",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("scalar subquery after prior CTE is stable", 0, "SQLCLEAN022")],
    ids=lambda case: case.description,
)
def test_given_prior_cte_when_scalar_subquery_is_aliased_then_inner_aggregate_is_not_reported(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "support"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "ticket_totals.sql"
    model.parent.mkdir()
    model.write_text(
        """MODEL (description "Support ticket totals");
WITH products_used AS (
    SELECT product_id FROM products
),
ticket_totals AS (
    SELECT
        ticket.ticket_id,
        (SELECT MAX(change_id) FROM support_changes) AS latest_change_id
    FROM support_tickets AS ticket
)
SELECT * FROM ticket_totals
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("cast type is not a self alias", 0, "SQLCLEAN016")],
    ids=lambda case: case.description,
)
def test_given_cast_and_relation_aliases_when_running_self_alias_rule_then_no_finding_is_reported(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        """MODEL (description "Orders");
SELECT CAST(orders.order_id::INTEGER AS INTEGER) AS order_id
FROM orders AS orders
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert payload["findings"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("upstream graph selector scopes an ignore", 1, "SQLCLEAN021")],
    ids=lambda case: case.description,
)
def test_given_graph_scoped_rule_ignore_when_running_rules_then_only_matched_resources_are_ignored(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n'
        "[[rules.rule_ignores]]\n"
        'rules = ["SQLCLEAN021"]\n'
        'selectors = ["+orders*"]\n'
        'reason = "Reviewed upstream interface boundary"\n',
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "base_orders.sql").write_text(
        'MODEL (description "Base orders");\nSELECT 1 AS order_id\n',
        encoding="utf-8",
    )
    (models / "orders.sql").write_text(
        'MODEL (description "Orders");\nSELECT * FROM __ref("base_orders")\n',
        encoding="utf-8",
    )
    (models / "customer_orders.sql").write_text(
        'MODEL (description "Customer orders");\nSELECT * FROM __ref("orders")\n',
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    findings: list[dict[str, object]] = cast(list[dict[str, object]], payload["findings"])
    finding_paths: list[object] = [finding.get("path") for finding in findings]
    assert finding_paths == ["models/customer_orders.sql"]


@pytest.mark.parametrize(
    "test_case",
    [
        RulesIntegrationTestCase(
            "default-off custom family is explicitly selected", 1, "XSQBRARCH001"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_default_off_custom_rule_when_running_exact_and_prefix_then_both_select_rule(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n",
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    rule_file: Path = tmp_path / "rules" / "architecture.py"
    rule_file.parent.mkdir()
    rule_file.write_text(
        """from sqlbuild.rules import Finding, Model, RuleContext, rule

@rule(code="XSQBRARCH001", message="Forbidden name", remediation="Rename the model.")
def forbidden_name(*, model: Model, ctx: RuleContext) -> list[Finding]:
    return [ctx.finding(subject=model)]
""",
        encoding="utf-8",
    )

    exact_exit: int = main(
        ["--project-dir", str(tmp_path), "rules", "--json", "run", "XSQBRARCH001"]
    )
    exact: dict[str, object] = json.loads(capsys.readouterr().out)
    prefix_exit: int = main(["--project-dir", str(tmp_path), "rules", "--json", "run", "XSQBRARCH"])
    prefix: dict[str, object] = json.loads(capsys.readouterr().out)

    assert exact_exit == prefix_exit == test_case.expected_exit_code
    assert exact["findings"][0]["code"] == test_case.expected_code
    assert prefix["findings"][0]["code"] == test_case.expected_code


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("SQL rule exception suppresses finding", 0, "SQLCLEAN004")],
    ids=lambda case: case.description,
)
def test_given_sql_rule_exception_when_compiling_then_unified_suppression_is_applied(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n'
        '[rules]\nselect = ["SQLCLEAN004"]\n\n'
        '[[rules.rule_exceptions]]\nrule = "SQLCLEAN004"\n'
        'path = "models/orders.sql"\nreason = "The fixture intentionally selects one row."\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders");\nSELECT order_id FROM orders LIMIT 1\n',
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert test_case.expected_code not in json.dumps(payload["diagnostics"])


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("module constant edit invalidates custom rule", 0, "")],
    ids=lambda case: case.description,
)
def test_given_custom_rule_module_constant_edit_when_compiling_then_cached_result_is_invalidated(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n'
        '[rules]\nselect = ["XSQBRARCH001"]\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n",
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n", encoding="utf-8"
    )
    rule_file: Path = tmp_path / "rules" / "architecture.py"
    rule_file.parent.mkdir()
    rule_source: str = """from sqlbuild.rules import Finding, Model, RuleContext, rule

FORBIDDEN, UNUSED = ("orders", "unused")

@rule(code="XSQBRARCH001", message="Forbidden name", remediation="Rename the model.")
def forbidden_name(*, model: Model, ctx: RuleContext) -> list[Finding]:
    return [ctx.finding(subject=model)] if model.name == FORBIDDEN else []
"""
    rule_file.write_text(rule_source, encoding="utf-8")

    first_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    _ = capsys.readouterr()
    rule_file.write_text(
        rule_source.replace(
            'FORBIDDEN, UNUSED = ("orders", "unused")',
            'FORBIDDEN, UNUSED = ("customers", "unused")',
        ),
        encoding="utf-8",
    )
    second_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    second: dict[str, object] = json.loads(capsys.readouterr().out)

    assert first_exit == 1
    assert second_exit == test_case.expected_exit_code
    assert second["diagnostics"] == []
    assert second["compile_timings"]["rule_cache_misses"] >= 1


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("focused project cache cannot poison full compile", 0, "")],
    ids=lambda case: case.description,
)
def test_given_focused_project_rule_result_when_compiling_full_project_then_subset_cache_is_not_used(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n'
        '[rules]\nselect = ["XSQBRARCH001"]\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n",
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "orders.sql").write_text(
        "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n", encoding="utf-8"
    )
    (models / "customers.sql").write_text(
        "MODEL (description 'Test model customers.');\nSELECT 1 AS customer_id\n", encoding="utf-8"
    )
    rule_file: Path = tmp_path / "rules" / "architecture.py"
    rule_file.parent.mkdir()
    rule_file.write_text(
        """from sqlbuild.rules import Finding, Project, RuleContext, rule

@rule(code="XSQBRARCH001", message="Two models required", remediation="Add the other model.")
def two_models(*, project: Project, ctx: RuleContext) -> list[Finding]:
    del project
    return [] if len(ctx.project.models) == 2 else [ctx.finding(subject="models")]
""",
        encoding="utf-8",
    )

    focused_exit: int = main(
        ["--project-dir", str(tmp_path), "compile", "--json", "--select", "orders"]
    )
    _ = capsys.readouterr()
    full_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    full: dict[str, object] = json.loads(capsys.readouterr().out)

    assert focused_exit == test_case.expected_exit_code
    assert full_exit == test_case.expected_exit_code
    assert full["diagnostics"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("default compile evaluates audit SQL", 1, "SQLCLEAN004")],
    ids=lambda case: case.description,
)
def test_given_non_model_sql_violation_when_compiling_then_compile_is_authoritative(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN004"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n", encoding="utf-8"
    )
    (tmp_path / "models" / "customers.sql").write_text(
        "MODEL (description 'Test model customers.');\nSELECT 1 AS order_id\n", encoding="utf-8"
    )
    audit: Path = tmp_path / "audits" / "singular" / "order_sample.sql"
    audit.parent.mkdir(parents=True)
    audit.write_text(
        'AUDIT ();\nSELECT * FROM __ref("orders") JOIN __ref("customers") USING (order_id) LIMIT 1\n',
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    payload: dict[str, object] = json.loads(capsys.readouterr().out)

    assert exit_code == test_case.expected_exit_code
    assert payload["diagnostics"][0]["code"] == test_case.expected_code
    assert payload["diagnostics"][0]["path"] == "audits/singular/order_sample.sql"


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("focused compile ignores unselected exact exception", 0, "")],
    ids=lambda case: case.description,
)
def test_given_exception_for_other_model_when_compiling_selection_then_exception_is_not_stale(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN004"]\n\n'
        '[[rules.rule_exceptions]]\nrule = "SQLCLEAN004"\npath = "models/customers.sql"\n'
        'reason = "The fixture intentionally selects one row."\n',
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "orders.sql").write_text(
        "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n", encoding="utf-8"
    )
    (models / "customers.sql").write_text(
        "MODEL (description 'Test model customers.');\nSELECT customer_id FROM customers LIMIT 1\n",
        encoding="utf-8",
    )

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "compile", "--json", "--select", "orders"]
    )
    payload: dict[str, object] = json.loads(capsys.readouterr().out)

    assert exit_code == test_case.expected_exit_code
    assert payload["diagnostics"] == []


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("typed unselected model stays outside rule scope", 0, "")],
    ids=lambda case: case.description,
)
def test_given_typed_unselected_model_when_planning_selection_then_rules_use_selected_scope(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n\n'
        '[rules]\nselect = ["SQBRCONTRACT105"]\n',
        encoding="utf-8",
    )
    sources: Path = tmp_path / "sources"
    sources.mkdir()
    (sources / "customers.yml").write_text(
        """sources:
  - name: raw_customers
    description: Test source raw_customers.
    schema: raw
    table: customers
    columns:
      - name: customer_id
        type: VARCHAR
""",
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', contract enforced, columns (order_id (type INTEGER)));\n"
        "SELECT CAST(1 AS INTEGER) AS order_id\n",
        encoding="utf-8",
    )
    (models / "customers.sql").write_text(
        """MODEL (description "Test model customers.",
  contract enforced,
  columns (customer_id (type VARCHAR)),
);

WITH source_rows AS (
  SELECT customer_id FROM __source("raw_customers")
), final AS (
  SELECT customer_id FROM source_rows
)
SELECT customer_id FROM final
""",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "plan", "--json", "--select", "orders"])
    payload: dict[str, object] = json.loads(capsys.readouterr().out)

    assert exit_code == test_case.expected_exit_code
    assert payload["selected_count"] == 1
    assert [model["name"] for model in cast(list[dict[str, object]], payload["models"])] == [
        "orders"
    ]


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("subject folder precedes model layer", 1, "SQBRPROJECT102")],
    ids=lambda case: case.description,
)
def test_given_subject_before_layer_when_running_folder_rule_then_canonical_move_is_reported(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = (
        tmp_path
        / "models"
        / "commerce"
        / "intermediate"
        / "payments"
        / "clean"
        / "commerce__int_clean__orders.sql"
    )
    model.parent.mkdir(parents=True)
    model.write_text(
        'MODEL (description "Clean orders");\nSELECT 1 AS order_id\n', encoding="utf-8"
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "rules",
            "--json",
            "run",
            test_case.expected_code,
        ]
    )

    assert exit_code == test_case.expected_exit_code
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    findings: list[dict[str, object]] = cast(list[dict[str, object]], payload["findings"])
    assert [finding["message"] for finding in findings] == [
        'int_clean model "commerce__int_clean__orders" must keep layer folder '
        '"intermediate/clean" contiguous'
    ]
    assert [finding["remediation"] for finding in findings] == [
        'Move the model to "models/commerce/intermediate/clean/payments/'
        'commerce__int_clean__orders.sql"; preserve subject folders outside '
        '"intermediate/clean". Use rules.layout with SQBRPROJECT201-204 to enforce '
        "domain and subdomain ordering."
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        RulesIntegrationTestCase(
            "relative project path retains cached SQL finding", 1, "SQLCLEAN004"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_relative_project_path_when_compiling_twice_then_sql_finding_remains_cached(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = tmp_path / "project"
    project_dir.mkdir()
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN004"]\n',
        encoding="utf-8",
    )
    model: Path = project_dir / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (description 'Test model orders.');\nSELECT order_id FROM orders LIMIT 1\n",
        encoding="utf-8",
    )
    monkeypatch.chdir(tmp_path)

    first_exit: int = main(["--project-dir", "project", "compile", "--json"])
    _ = capsys.readouterr()
    second_exit: int = main(["--project-dir", "project", "compile", "--json"])
    second: dict[str, object] = json.loads(capsys.readouterr().out)

    assert first_exit == second_exit == test_case.expected_exit_code
    assert second["diagnostics"][0]["code"] == test_case.expected_code
    assert second["compile_timings"]["rule_cache_hits"] == 1


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("inline directive without a finding fails", 1, "SQBRSUPPRESSION000")],
    ids=lambda case: case.description,
)
def test_given_inline_directive_without_finding_when_compiling_then_directive_is_reported(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN004"]\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (description 'Test model orders.');\n-- sqb: ignore SQLCLEAN004 because this fixture checks stale directives\n"
        "SELECT 1 AS order_id\n",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    payload: dict[str, object] = json.loads(capsys.readouterr().out)

    assert exit_code == test_case.expected_exit_code
    assert payload["diagnostics"][0]["code"] == test_case.expected_code


@pytest.mark.parametrize(
    "test_case",
    [RulesIntegrationTestCase("unselected custom options validate consistently", 0, "")],
    ids=lambda case: case.description,
)
def test_given_unselected_custom_rule_options_when_compiling_then_configuration_is_validated(
    test_case: RulesIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = []\n\n'
        '[rules.rule_options.XSQBRARCH001]\nrequired_prefix = "order"\n',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n", encoding="utf-8"
    )
    rule_file: Path = tmp_path / "rules" / "architecture.py"
    rule_file.parent.mkdir()
    rule_file.write_text(
        """from sqlbuild.rules import Finding, Model, RuleContext, RuleOption, rule

REQUIRED_PREFIX = RuleOption.string(
    name="required_prefix", default="model", description="Required model prefix."
)

@rule(
    code="XSQBRARCH001",
    message="Wrong prefix",
    remediation="Rename the model.",
    options=(REQUIRED_PREFIX,),
)
def required_prefix(*, model: Model, ctx: RuleContext) -> list[Finding]:
    return [] if model.name.startswith(ctx.option(REQUIRED_PREFIX)) else [ctx.finding(subject=model)]
""",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    payload: dict[str, object] = json.loads(capsys.readouterr().out)

    assert exit_code == test_case.expected_exit_code
    assert payload["diagnostics"] == []


@pytest.mark.parametrize(
    "test_case",
    [
        EmptyRuleSelectionTestCase(
            "valid exception for an unevaluated rule compiles",
            "[[rules.rule_exceptions]]\n"
            'rule = "SQBRCONTRACT101"\npath = "models/orders.sql"\nreason = "Legacy."\n',
            0,
            (),
        ),
        EmptyRuleSelectionTestCase(
            "unknown exception rule is rejected",
            '[[rules.rule_exceptions]]\nrule = "SQBRNOPE999"\npath = "models/orders.sql"\n'
            'reason = "Legacy."\n',
            1,
            ("rule exceptions target unknown rules: SQBRNOPE999",),
        ),
        EmptyRuleSelectionTestCase(
            "missing exception path is rejected",
            '[[rules.rule_exceptions]]\nrule = "SQBRCONTRACT101"\npath = "models/missing.sql"\n'
            'reason = "Legacy."\n',
            1,
            ("rule exception path does not exist: models/missing.sql",),
        ),
        EmptyRuleSelectionTestCase(
            "ignore selector matching no rule is rejected",
            '[[rules.rule_ignores]]\nrules = ["SQBRZZZ"]\npaths = ["models/orders.sql"]\n'
            'reason = "Legacy."\n',
            1,
            ("rule-ignore selector matches no rules: SQBRZZZ",),
        ),
        EmptyRuleSelectionTestCase(
            "missing literal ignore path is rejected",
            '[[rules.rule_ignores]]\nrules = ["SQLCLEAN"]\npaths = ["models/orders.sql", "models/gone.sql"]\n'
            'reason = "Legacy."\n',
            1,
            ("rule ignore path does not exist: models/gone.sql",),
        ),
        EmptyRuleSelectionTestCase(
            "ignore paths naming a folder or a glob that matches nothing compile",
            '[[rules.rule_ignores]]\nrules = ["SQLCLEAN"]\npaths = ["models", "models/legacy/**"]\n'
            'reason = "Legacy."\n',
            0,
            (),
        ),
        EmptyRuleSelectionTestCase(
            "unknown built-in rule option is rejected",
            "[rules.rule_options.SQLCLEAN004]\nnope = 3\n",
            1,
            ("rule SQLCLEAN004 has unknown options: nope",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_empty_rule_selection_when_compiling_then_only_configuration_is_validated(
    test_case: EmptyRuleSelectionTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = []\n\n{test_case.configuration}',
        encoding="utf-8",
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n", encoding="utf-8"
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    timings: dict[str, int] = payload.get("compile_timings", {})

    assert exit_code == test_case.expected_exit_code
    assert [diagnostic["message"] for diagnostic in payload["diagnostics"]] == list(
        test_case.expected_messages
    )
    assert not rules_bulk_cache_path(
        project_dir=tmp_path, file_name="native-response.json"
    ).exists()
    assert (timings.get("rule_cache_hits", 0), timings.get("rule_cache_misses", 0)) == (0, 0)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlRuleCacheSequenceTestCase(
            description="cold misses, warm hits and each SQL rule input edit misses",
            steps=(
                (_CACHE_RULES_TOML, _CACHE_MODEL_SQL),
                (_CACHE_RULES_TOML, _CACHE_MODEL_SQL),
                (_CACHE_RULES_TOML, _CACHE_EDITED_SQL),
                (_CACHE_RULES_TOML, _CACHE_EDITED_SQL),
                (_CACHE_THRESHOLD_TOML, _CACHE_EDITED_SQL),
                (_CACHE_SELECTION_TOML, _CACHE_EDITED_SQL),
                (_CACHE_SELECTION_TOML, _CACHE_EDITED_SQL),
            ),
            expected_hits_and_misses=(
                (0, 1),
                (1, 0),
                (0, 1),
                (1, 0),
                (0, 1),
                (0, 1),
                (1, 0),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_rule_cache_when_inputs_change_then_only_unchanged_runs_hit(
    test_case: SqlRuleCacheSequenceTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    observed: list[tuple[int, int]] = []
    for project_toml, model_sql in test_case.steps:
        (tmp_path / "sqlbuild_project.toml").write_text(project_toml, encoding="utf-8")
        model.write_text(model_sql, encoding="utf-8")
        _ = main(["--project-dir", str(tmp_path), "compile", "--json"])
        timings: dict[str, int] = cast(
            dict[str, int], json.loads(capsys.readouterr().out)["compile_timings"]
        )
        observed.append((timings["rule_cache_hits"], timings["rule_cache_misses"]))

    assert tuple(observed) == test_case.expected_hits_and_misses


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleCacheSequenceTestCase(
            description=(
                "the custom rule misses cold and after a source edit, and hits through a model "
                "SQL edit it never read; the built-in model rules add one count per run"
            ),
            steps=(
                (_CUSTOM_CACHE_RULE, _CUSTOM_CACHE_MODEL_SQL),
                (_CUSTOM_CACHE_RULE, _CUSTOM_CACHE_MODEL_SQL),
                (_CUSTOM_CACHE_RULE, _CUSTOM_CACHE_EDITED_SQL),
                (_CUSTOM_CACHE_EDITED_RULE, _CUSTOM_CACHE_EDITED_SQL),
                (_CUSTOM_CACHE_EDITED_RULE, _CUSTOM_CACHE_EDITED_SQL),
                (_CUSTOM_CACHE_RULE, _CUSTOM_CACHE_EDITED_SQL),
            ),
            expected_hits_and_misses=((0, 2), (2, 0), (1, 1), (0, 2), (2, 0), (0, 2)),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_custom_rule_cache_when_inputs_change_then_only_unchanged_runs_hit(
    test_case: CustomRuleCacheSequenceTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_CUSTOM_CACHE_TOML, encoding="utf-8")
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    rule_file: Path = tmp_path / "rules" / "layout.py"
    rule_file.parent.mkdir()
    observed: list[tuple[int, int]] = []
    for rule_source, model_sql in test_case.steps:
        rule_file.write_text(rule_source, encoding="utf-8")
        model.write_text(model_sql, encoding="utf-8")
        _ = main(["--project-dir", str(tmp_path), "compile", "--json"])
        timings: dict[str, int] = cast(
            dict[str, int], json.loads(capsys.readouterr().out)["compile_timings"]
        )
        observed.append((timings["rule_cache_hits"], timings["rule_cache_misses"]))

    assert tuple(observed) == test_case.expected_hits_and_misses


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleSurfaceTestCase(
            description=(
                "rules reading every public fact see the compiled values cold, reuse them warm, "
                "and rerun only the edited model's invocation"
            ),
            project_files=_SURFACE_FILES,
            edits=({}, {}, _SURFACE_STAGING_EDIT),
            expected_messages=(
                "2|None|order_totals,stg_orders|raw_orders|regions||test_order_totals|"
                "not_null,accepted_values,accepted_values|1|3|order_status|minimum_amount|1|1|"
                "order_totals",
                "order_totals|models/marts/order_totals.sql|table|137|79|stg_orders|||"
                "order_id:INTEGER:non_null;doubled_amount:DOUBLE:unknown|order_id,doubled_amount|"
                "False||test_order_totals|",
                "stg_orders|models/staging/stg_orders.sql|None|417|109||order_totals|"
                "order_id:INTEGER:False:Order key:not_null:[]:None/accepted_values:"
                "[('values', [1, 2])]:warn:models/staging/stg_orders.sql:2;"
                "amount:DOUBLE:None:None::models/staging/stg_orders.sql:4;"
                "status:VARCHAR:None:None:accepted_values:[('values', ('PAID', 'OPEN'))]:None:"
                "models/staging/stg_orders.sql:5|"
                "order_id:INTEGER:unknown;amount:DOUBLE:unknown;status:VARCHAR:non_null|"
                "order_id,amount,status|True|||not_null,accepted_values,accepted_values",
            ),
            expected_hits_and_misses=((0, 4), (4, 0), (3, 1)),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_rules_reading_every_fact_when_compiling_then_values_and_cache_reuse_hold(
    test_case: CustomRuleSurfaceTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_project_files(project_dir=tmp_path, files=test_case.project_files)
    observed: list[tuple[int, int]] = []
    messages: list[tuple[str, ...]] = []
    for edit in test_case.edits:
        write_project_files(project_dir=tmp_path, files=edit)
        _ = main(["--project-dir", str(tmp_path), "compile", "--json"])
        output: dict[str, Any] = json.loads(capsys.readouterr().out)
        timings: dict[str, int] = output["compile_timings"]
        observed.append((timings["rule_cache_hits"], timings["rule_cache_misses"]))
        messages.append(tuple(sorted(item["message"] for item in output["diagnostics"])))
    config_path: Path = tmp_path / "sqlbuild_project.toml"
    config_path.write_text(
        config_path.read_text(encoding="utf-8") + "\n[rules.cache]\nenabled = false\n",
        encoding="utf-8",
    )
    _ = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    uncached: dict[str, Any] = json.loads(capsys.readouterr().out)

    assert tuple(observed) == test_case.expected_hits_and_misses
    assert messages[0] == messages[1] == test_case.expected_messages
    assert messages[2] == tuple(sorted(item["message"] for item in uncached["diagnostics"]))


@pytest.mark.parametrize(
    "test_case",
    [
        CustomHostOutcomeTestCase(
            description="a rule failing after lazy fact reads reports its model",
            rule_body=(
                "columns = ctx.columns.inferred(model)\n"
                'if model.name == "orders_001":\n'
                '    raise ValueError(f"cannot inspect {columns[0].name}")\n'
                "return []"
            ),
            expected_exit_code=1,
            expected_messages=(
                "invalid rules lifecycle: custom host failed: rule XSQBRHOST001 failed for "
                "models/orders_001.sql: cannot inspect order_id",
            ),
        ),
        CustomHostOutcomeTestCase(
            description="a rule exiting its host reports the host failure",
            rule_body=(
                "sql = ctx.sql.for_model(model).expanded.source\n"
                'if model.name == "orders_002":\n'
                "    raise SystemExit(3)\n"
                "return []"
            ),
            expected_exit_code=1,
            expected_messages=(
                "invalid rules lifecycle: custom host failed: process exited with exit status: 3",
            ),
        ),
        CustomHostOutcomeTestCase(
            description="a slow rule reading lazy facts reports every finding",
            rule_body=(
                "_ = sum(range(2_000_000))\n"
                "return [ctx.finding(subject=model, message=ctx.sql.for_model(model).expanded.source)]"
            ),
            expected_exit_code=1,
            expected_messages=(
                "SELECT 0 AS order_id",
                "SELECT 1 AS order_id",
                "SELECT 2 AS order_id",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_rule_outcome_when_compiling_then_result_and_cleanup_are_reported(
    test_case: CustomHostOutcomeTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_custom_host_project(project_dir=tmp_path, model_count=3, rule_body=test_case.rule_body)

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    output: dict[str, Any] = json.loads(capsys.readouterr().out)

    assert exit_code == test_case.expected_exit_code
    assert tuple(item["message"] for item in output["diagnostics"]) == test_case.expected_messages
    assert not tuple((tmp_path / "target" / "rules-cache" / "host-inputs").iterdir())


@pytest.mark.parametrize(
    "test_case",
    [
        SetOperationStarRuleTestCase(
            description="unaliased and aliased derived-table branches",
            model_sql=(
                "MODEL (description 'Test model order_ids.', materialized view);\n"
                'SELECT * FROM (SELECT order_id FROM __ref("orders"))\n'
                "UNION ALL\n"
                'SELECT * FROM (SELECT order_id FROM __ref("orders")) AS s\n'
            ),
            expected_codes=("SQBRMODEL102", "SQBRMODEL102"),
            expected_distinct_lines=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_star_in_set_operation_branches_when_compiling_then_each_branch_is_reported(
    test_case: SetOperationStarRuleTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRMODEL102"]\n',
        encoding="utf-8",
    )
    (tmp_path / "models").mkdir()
    (tmp_path / "models" / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized view);\nSELECT 1 AS order_id\n",
        encoding="utf-8",
    )
    (tmp_path / "models" / "order_ids.sql").write_text(test_case.model_sql, encoding="utf-8")

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], result["diagnostics"])

    assert exit_code == 1
    assert tuple(item["code"] for item in diagnostics) == test_case.expected_codes
    assert len({item["line"] for item in diagnostics}) == test_case.expected_distinct_lines
