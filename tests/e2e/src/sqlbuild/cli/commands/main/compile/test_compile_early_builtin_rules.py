"""E2E tests for built-in rules evaluated speculatively during compilation."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    EarlyBuiltinRulesEditTestCase,
    EarlyBuiltinRulesTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    compile_json_with,
    diagnostic_location_keys,
    early_rules_outcome,
    rule_cache_outcome,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project

_BUILTIN_PROJECT_TOML: str = (
    'name = "commerce"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRMODEL102"]\n'
)
_CUSTOM_PROJECT_TOML: str = (
    'name = "commerce"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRMODEL102", "XSQBR"]\n'
    "\n[rules.thresholds]\nmin_custom_rule_test_cases = 0\n"
)
_ORDERS_SQL: str = 'MODEL (description "Orders");\nSELECT 1 AS order_id, 2 AS amount\n'
_STAR_TOTALS_SQL: str = 'MODEL (description "Order totals");\nSELECT * FROM __ref("orders")\n'
_BROKEN_TOTALS_SQL: str = (
    'MODEL (description "Order totals");\nSELECT order_id, missing_column FROM __ref("orders")\n'
)
_LONG_NAME_RULE: str = (
    "from sqlbuild.rules import Finding, Model, RuleContext, rule\n\n\n"
    '@rule(code="XSQBRNAME001", message="Model names stay short", '
    'remediation="Rename the model.")\n'
    "def short_names(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
    "    if len(model.name) <= 8:\n"
    "        return []\n"
    "    return [ctx.finding(subject=model)]\n"
)
_STAR_FINDING: tuple[str, str, int, int] = ("SQBRMODEL102", "sql/models/order_totals.sql", 2, 8)


@pytest.mark.parametrize(
    "test_case",
    (
        EarlyBuiltinRulesTestCase(
            description="clean graph answers built-in rules from the early evaluation",
            repo_files={
                "sqlbuild_project.toml": _BUILTIN_PROJECT_TOML,
                "sql/models/orders.sql": _ORDERS_SQL,
                "sql/models/order_totals.sql": _STAR_TOTALS_SQL,
            },
            compile_arguments=(),
            expected_exit_code=1,
            expected_diagnostics=(_STAR_FINDING,),
            expected_early_hits=1,
            expected_early_misses=0,
        ),
        EarlyBuiltinRulesTestCase(
            description="graph error skips rules and discards the early evaluation",
            repo_files={
                "sqlbuild_project.toml": _BUILTIN_PROJECT_TOML,
                "sql/models/orders.sql": _ORDERS_SQL,
                "sql/models/order_totals.sql": _BROKEN_TOTALS_SQL,
            },
            compile_arguments=(),
            expected_exit_code=1,
            expected_diagnostics=(("B002", "sql/models/order_totals.sql", 2, 18),),
            expected_early_hits=0,
            expected_early_misses=0,
        ),
        EarlyBuiltinRulesTestCase(
            description="model selection evaluates rules without an early evaluation",
            repo_files={
                "sqlbuild_project.toml": _BUILTIN_PROJECT_TOML,
                "sql/models/orders.sql": _ORDERS_SQL,
                "sql/models/order_totals.sql": _STAR_TOTALS_SQL,
            },
            compile_arguments=("--select", "order_totals"),
            expected_exit_code=1,
            expected_diagnostics=(_STAR_FINDING,),
            expected_early_hits=0,
            expected_early_misses=0,
        ),
        EarlyBuiltinRulesTestCase(
            description="custom rules keep built-in rules in the rules phase",
            repo_files={
                "sqlbuild_project.toml": _CUSTOM_PROJECT_TOML,
                "sql/models/orders.sql": _ORDERS_SQL,
                "sql/models/order_totals.sql": _STAR_TOTALS_SQL,
                "rules/naming.py": _LONG_NAME_RULE,
            },
            compile_arguments=(),
            expected_exit_code=1,
            expected_diagnostics=(
                _STAR_FINDING,
                ("XSQBRNAME001", "sql/models/order_totals.sql", 1, 1),
            ),
            expected_early_hits=0,
            expected_early_misses=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_rules_project_when_compiling_cold_and_warm_then_early_rules_keep_findings(
    test_case: EarlyBuiltinRulesTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="commerce", repo_files=test_case.repo_files
    )

    runs: list[tuple[int, dict[str, Any]]] = [
        compile_json_with(project_dir=project_dir, arguments=test_case.compile_arguments)
        for _ in range(2)
    ]

    for exit_code, payload in runs:
        assert exit_code == test_case.expected_exit_code
        assert diagnostic_location_keys(payload) == test_case.expected_diagnostics
        assert early_rules_outcome(payload) == (
            test_case.expected_early_hits,
            test_case.expected_early_misses,
        )


@pytest.mark.parametrize(
    "test_case",
    (
        EarlyBuiltinRulesEditTestCase(
            description="editing one model reuses the other model's cached findings",
            edited_path="sql/models/orders.sql",
            edited_sql='MODEL (description "Orders");\nSELECT 1 AS order_id, 3 AS amount\n',
            expected_diagnostics=(_STAR_FINDING,),
            expected_early_outcomes=((1, 0), (1, 0), (1, 0)),
            expected_rule_cache_outcomes=((0, 2), (2, 0), (1, 1)),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_early_rules_when_compiling_cold_warm_and_edited_then_rule_cache_still_hits(
    test_case: EarlyBuiltinRulesEditTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="commerce",
        repo_files={
            "sqlbuild_project.toml": _BUILTIN_PROJECT_TOML,
            "sql/models/orders.sql": _ORDERS_SQL,
            "sql/models/order_totals.sql": _STAR_TOTALS_SQL,
        },
    )

    payloads: list[dict[str, Any]] = [compile_json_with(project_dir=project_dir, arguments=())[1]]
    payloads.append(compile_json_with(project_dir=project_dir, arguments=())[1])
    (project_dir / test_case.edited_path).write_text(test_case.edited_sql, encoding="utf-8")
    payloads.append(compile_json_with(project_dir=project_dir, arguments=())[1])

    assert diagnostic_location_keys(payloads[-1]) == test_case.expected_diagnostics
    assert tuple(early_rules_outcome(payload) for payload in payloads) == (
        test_case.expected_early_outcomes
    )
    assert tuple(rule_cache_outcome(payload) for payload in payloads) == (
        test_case.expected_rule_cache_outcomes
    )
