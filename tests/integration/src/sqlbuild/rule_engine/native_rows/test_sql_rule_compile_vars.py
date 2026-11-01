"""SQL Rules expand with the compile's effective target and vars when no early lint is reused."""

from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast

import pytest

import sqlbuild._native as native
from sqlbuild._native import SqlRulesEvaluation
from tests.integration.src.sqlbuild.rule_engine.native_rows._test_types import (
    SqlRuleCompileVarsTestCase,
)

_PROJECT_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
    '[vars]\nprobe = "1"\n\n[targets.dev]\n\n[targets.prod.vars]\nprobe = "NULL"\n'
)
_MODEL_PATH: Path = Path("models/orders.sql")
_MODEL_SQL: str = (
    'MODEL (description "Orders");\nSELECT o.order_id FROM orders AS o WHERE o.order_id = @@probe\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlRuleCompileVarsTestCase(
            "compiled prod vars, not the default", {"probe": "NULL"}, ("SQLCLEAN001",)
        ),
        SqlRuleCompileVarsTestCase("compiled vars that are clean", {"probe": "2"}, ()),
    ],
    ids=lambda case: case.description,
)
def test_given_no_early_lint_when_running_sql_rules_then_compile_vars_are_expanded(
    test_case: SqlRuleCompileVarsTestCase, tmp_path: Path
) -> None:
    (tmp_path / "models").mkdir()
    (tmp_path / "sqlbuild_project.toml").write_text(_PROJECT_TOML, encoding="utf-8")
    (tmp_path / _MODEL_PATH).write_text(_MODEL_SQL, encoding="utf-8")
    model: SimpleNamespace = SimpleNamespace(
        relative_path=_MODEL_PATH,
        authored_sql="SELECT o.order_id FROM orders AS o",
        query_sql="SELECT o.order_id FROM orders AS o",
        config=SimpleNamespace(values={}),
        dynamic_column_contract=None,
        relation_key_row=("model", "orders", "", "", "", False, None, [], []),
    )
    project: SimpleNamespace = SimpleNamespace(
        settings=SimpleNamespace(sql_analysis=True),
        effective_target_name="prod",
        effective_vars=test_case.effective_vars,
        models=(model,),
        seeds=(),
        sources=(),
        sql_expansions={},
    )
    plan: SimpleNamespace = SimpleNamespace(
        project_dir=tmp_path,
        project=project,
        model_paths=frozenset((_MODEL_PATH,)),
        selected=(SimpleNamespace(code="SQLCLEAN001", custom=False),),
        effective_config=SimpleNamespace(thresholds={}, cache=SimpleNamespace(enabled=False)),
    )

    result: SqlRulesEvaluation = native.run_planned_sql_rules(
        plan=cast(Any, plan),
        graph=cast(Any, SimpleNamespace(project=project)),
        dialect="duckdb",
        discovered_inputs=cast(Any, None),
        early_lint=None,
    )

    assert tuple(finding.code for finding in result.findings) == test_case.expected_codes


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
