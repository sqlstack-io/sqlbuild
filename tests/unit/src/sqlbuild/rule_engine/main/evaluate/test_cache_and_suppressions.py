"""Policy cache and suppression behavior tests."""

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import (
    RuleExemption,
    RuleIgnore,
    RulesConfig,
    RulesError,
    RulesResult,
    compiled_model_record,
    project_with,
)
from sqlbuild._native import evaluate_project_rules as evaluate
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.rule_engine.main.evaluate._test_types import PolicyBehaviorTestCase
from tests.unit.src.sqlbuild.rule_engine.main.evaluate.helpers import build_project


@pytest.mark.parametrize(
    "test_case",
    [
        PolicyBehaviorTestCase(
            description="unchanged built-in inputs hit cache",
            expected_cache_hits=1,
            expected_cache_misses=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unchanged_builtin_inputs_when_evaluating_twice_then_second_run_hits_cache(
    tmp_path: Path,
    test_case: PolicyBehaviorTestCase,
) -> None:
    project: CompiledProjectView = build_project(
        name="commerce__mart__orders",
        relative_path="sql/models/mart/commerce__mart__orders.sql",
        sql="SELECT * FROM orders",
        config_values={},
    )
    config: RulesConfig = RulesConfig(select=("SQBRCONTRACT101",))

    first: RulesResult = evaluate(project=project, config=config, project_dir=tmp_path)
    second: RulesResult = evaluate(project=project, config=config, project_dir=tmp_path)

    assert first.cache_hits == 0
    assert first.cache_misses == 1
    assert second.cache_hits == test_case.expected_cache_hits
    assert second.cache_misses == test_case.expected_cache_misses
    assert second.findings == first.findings


@pytest.mark.parametrize(
    "test_case",
    [
        PolicyBehaviorTestCase(
            description="changed authored SQL invalidates equivalent compiled SQL",
            expected_finding_count=1,
            expected_cache_hits=0,
            expected_cache_misses=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_authored_sql_changes_when_compiled_sql_is_same_then_cache_is_invalidated(
    tmp_path: Path,
    test_case: PolicyBehaviorTestCase,
) -> None:
    compiled_sql: str = "WITH final AS (SELECT 1 AS id) SELECT id FROM final WHERE id > 7"
    project: CompiledProjectView = build_project(
        name="commerce__mart__orders",
        relative_path="sql/models/mart/commerce__mart__orders.sql",
        sql=compiled_sql,
        config_values={},
    )
    constant_source: str = compiled_sql.replace("7", '@const("minimum_id")')
    first_project: CompiledProjectView = project_with(
        project=project,
        models=(replace(compiled_model_record(project.models[0]), authored_sql=constant_source),),
    )
    bare_project: CompiledProjectView = project_with(
        project=project,
        models=(replace(compiled_model_record(project.models[0]), authored_sql=compiled_sql),),
    )
    config: RulesConfig = RulesConfig(select=("SQBRDECLARATION102",))
    _ = evaluate(project=first_project, config=config, project_dir=tmp_path)

    result: RulesResult = evaluate(project=bare_project, config=config, project_dir=tmp_path)

    assert len(result.findings) == test_case.expected_finding_count
    assert result.cache_hits == test_case.expected_cache_hits
    assert result.cache_misses == test_case.expected_cache_misses


@pytest.mark.parametrize(
    "test_case",
    [
        PolicyBehaviorTestCase(
            description="matching exact exception suppresses fault",
            expected_finding_count=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_exact_exception_when_fault_exists_then_suppresses_fault(
    tmp_path: Path,
    test_case: PolicyBehaviorTestCase,
) -> None:
    relative_path = "sql/models/mart/commerce__mart__orders.sql"
    target: Path = tmp_path / relative_path
    target.parent.mkdir(parents=True)
    target.write_text("SELECT * FROM orders\n", encoding="utf-8")
    project: CompiledProjectView = build_project(
        name="commerce__mart__orders",
        relative_path=relative_path,
        sql="SELECT * FROM orders",
        config_values={},
    )
    config: RulesConfig = RulesConfig(
        select=("SQBRMODEL103",),
        rule_exceptions=(
            RuleExemption(
                rule="SQBRCONTRACT101",
                path=relative_path,
                reason="Migration is tracked",
            ),
        ),
    )

    result: RulesResult = evaluate(project=project, config=config, project_dir=tmp_path)

    assert len(result.findings) == test_case.expected_finding_count


@pytest.mark.parametrize(
    "test_case",
    [
        PolicyBehaviorTestCase(
            description="stale exact exception fails",
            expected_error_pattern="stale rule exception",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_stale_exact_exception_when_evaluating_then_raises_error(
    tmp_path: Path,
    test_case: PolicyBehaviorTestCase,
) -> None:
    relative_path = "sql/models/mart/commerce__mart__orders.sql"
    target: Path = tmp_path / relative_path
    target.parent.mkdir(parents=True)
    target.write_text("WITH final AS (SELECT 1 AS id) SELECT id FROM final\n", encoding="utf-8")
    project: CompiledProjectView = build_project(
        name="commerce__mart__orders",
        relative_path=relative_path,
        sql="WITH final AS (SELECT 1 AS id) SELECT id FROM final",
        config_values={},
    )
    config: RulesConfig = RulesConfig(
        select=("SQBRMODEL103",),
        rule_exceptions=(
            RuleExemption(rule="SQBRMODEL103", path=relative_path, reason="Migration is tracked"),
        ),
    )

    with pytest.raises(RulesError, match=test_case.expected_error_pattern):
        evaluate(project=project, config=config, project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        PolicyBehaviorTestCase(
            description="scoped ignore suppresses matching path",
            expected_finding_count=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scoped_ignore_when_fault_exists_then_suppresses_fault(
    tmp_path: Path,
    test_case: PolicyBehaviorTestCase,
) -> None:
    relative_path = "sql/models/legacy/commerce__mart__orders.sql"
    project: CompiledProjectView = build_project(
        name="commerce__mart__orders",
        relative_path=relative_path,
        sql="SELECT * FROM orders",
        config_values={},
    )
    config: RulesConfig = RulesConfig(
        select=("SQBRCONTRACT101",),
        rule_ignores=(
            RuleIgnore(
                rules=("SQBRCONTRACT",),
                paths=("sql/models/legacy/**",),
                reason="Legacy migration boundary",
            ),
        ),
    )

    result: RulesResult = evaluate(project=project, config=config, project_dir=tmp_path)

    assert len(result.findings) == test_case.expected_finding_count
