"""Custom-rule loading and hermeticity behavior tests."""

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import (
    CompactLineageFacts,
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
from tests.unit.src.sqlbuild.rule_engine._helpers.engine._test_types import (
    CustomRuleCacheDependencyTestCase,
    CustomRuleSuppressionTestCase,
    CustomRuleTestCase,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.engine.helpers import custom_rule_inputs


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleTestCase(
            description="selected custom rule faults",
            body="return [ctx.finding(subject=model)]",
            expected_fault_codes=("XSQBRT101",),
            expected_fault_lines=(1,),
        ),
        CustomRuleTestCase(
            description="untested custom rule faults coverage",
            body="del model\n    return []",
            expected_fault_codes=("SQBRTEST301",),
            expected_fault_lines=(4,),
            minimum_custom_rule_cases=1,
        ),
        CustomRuleTestCase(
            description="explicit prefix activates default-off custom rule",
            body="return [ctx.finding(subject=model)]",
            expected_fault_codes=("XSQBRT101",),
            expected_fault_lines=(1,),
            select=("XSQBRT",),
        ),
        CustomRuleTestCase(
            description="default-enabled custom rule is activated by prefix",
            body="return [ctx.finding(subject=model)]",
            expected_fault_codes=("XSQBRT101",),
            expected_fault_lines=(1,),
            select=("XSQBRT",),
            enabled_by_default=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selected_custom_rule_when_evaluating_then_reports_fault(
    tmp_path: Path,
    test_case: CustomRuleTestCase,
) -> None:
    project: CompiledProjectView
    config: RulesConfig
    project, config = custom_rule_inputs(tmp_path=tmp_path, test_case=test_case)

    result: RulesResult = evaluate(project=project, config=config, project_dir=tmp_path)

    assert tuple(fault.code for fault in result.findings) == test_case.expected_fault_codes
    assert tuple(fault.line for fault in result.findings) == test_case.expected_fault_lines


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleTestCase(
            description="custom rule uses deterministic cache",
            body="del model\n    return []",
            expected_cache_hits=2,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_custom_rule_when_repeated_then_deterministic_cache_is_reused(
    tmp_path: Path,
    test_case: CustomRuleTestCase,
) -> None:
    project: CompiledProjectView
    config: RulesConfig
    project, config = custom_rule_inputs(tmp_path=tmp_path, test_case=test_case)
    _ = evaluate(project=project, config=config, project_dir=tmp_path)

    second: RulesResult = evaluate(project=project, config=config, project_dir=tmp_path)

    assert second.cache_hits == test_case.expected_cache_hits


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleTestCase(
            description="lazy lineage cache does not affect project fact identity",
            body="del project\n    return []",
            project_wide=True,
            expected_cache_hits=2,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_lazy_lineage_when_materialized_then_project_rule_cache_is_stable(
    tmp_path: Path,
    test_case: CustomRuleTestCase,
) -> None:
    project: CompiledProjectView
    config: RulesConfig
    project, config = custom_rule_inputs(tmp_path=tmp_path, test_case=test_case)
    lineage: CompactLineageFacts = CompactLineageFacts(
        string_pool=("order_id",),
        rows=((0, 0, 1, ()),),
        resource_name_indexes={0: 0},
    )
    project = project_with(
        project=project,
        models=(replace(compiled_model_record(project.models[0]), fast_lineage_columns=lineage),),
    )
    _ = evaluate(project=project, config=config, project_dir=tmp_path)
    materialized_project: CompiledProjectView = project_with(
        project=project,
        models=(
            replace(compiled_model_record(project.models[0]), fast_lineage_columns=tuple(lineage)),
        ),
    )

    second: RulesResult = evaluate(
        project=materialized_project,
        config=config,
        project_dir=tmp_path,
    )

    assert second.cache_hits == test_case.expected_cache_hits


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleTestCase(
            description="verified hermetic custom rule keeps cache",
            body="del model\n    return []",
            expected_cache_hits=2,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_hermetic_custom_rule_when_cache_required_then_second_run_hits_cache(
    tmp_path: Path,
    test_case: CustomRuleTestCase,
) -> None:
    project: CompiledProjectView
    config: RulesConfig
    project, config = custom_rule_inputs(tmp_path=tmp_path, test_case=test_case)
    _ = evaluate(project=project, config=config, project_dir=tmp_path)

    second: RulesResult = evaluate(project=project, config=config, project_dir=tmp_path)

    assert second.cache_hits == test_case.expected_cache_hits


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleCacheDependencyTestCase(
            description="model-only rule ignores unrelated project file changes",
            body="del model\n    return []",
            changed_path="config/new.yml",
            expected_cache_hits=2,
        ),
        CustomRuleCacheDependencyTestCase(
            description="graph-observing rule ignores unrelated project file changes",
            body="_ = ctx.graph.dependencies(model)\n    return []",
            changed_path="config/new.yml",
            expected_cache_hits=2,
        ),
        CustomRuleCacheDependencyTestCase(
            description="project glob observes a newly matching file",
            body=('del model\n    _ = ctx.project.tree.glob("config/*.yml")\n    return []'),
            changed_path="config/new.yml",
            expected_cache_hits=1,
        ),
        CustomRuleCacheDependencyTestCase(
            description="project glob ignores a file it does not match",
            body=('del model\n    _ = ctx.project.tree.glob("config/**/*.yml")\n    return []'),
            changed_path="config/new.yml",
            expected_cache_hits=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_file_change_when_repeating_custom_rule_then_dependencies_control_cache(
    tmp_path: Path,
    test_case: CustomRuleCacheDependencyTestCase,
) -> None:
    project: CompiledProjectView
    config: RulesConfig
    project, config = custom_rule_inputs(
        tmp_path=tmp_path,
        test_case=CustomRuleTestCase(description=test_case.description, body=test_case.body),
    )
    _ = evaluate(project=project, config=config, project_dir=tmp_path)
    changed_path: Path = tmp_path / test_case.changed_path
    changed_path.parent.mkdir(parents=True)
    changed_path.write_text("enabled: true\n", encoding="utf-8")

    second: RulesResult = evaluate(project=project, config=config, project_dir=tmp_path)

    assert second.cache_hits == test_case.expected_cache_hits


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleTestCase(
            description="nonhermetic cacheable rule fails",
            body='open("policy.txt")\n    return []',
            expected_error_pattern=(
                r"non-hermetic custom rule XSQBRT101 at rules/custom.py:\d+: "
                r"opening 'policy.txt' is not allowed"
            ),
        ),
        CustomRuleTestCase(
            description="project-wide cacheable rule rejects untracked file types",
            body=(
                'del model\n    _ = ctx.project.tree.read_text("rules/input.json")\n    return []'
            ),
            project_wide=True,
            expected_error_pattern=r"compiler rules input must be a tracked file type",
        ),
        CustomRuleTestCase(
            description="project-wide rule rejects untracked file types",
            body=(
                'del model\n    _ = ctx.project.tree.read_text("rules/input.json")\n    return []'
            ),
            project_wide=True,
            expected_error_pattern=r"compiler rules input must be a tracked file type",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_nonhermetic_custom_rule_when_cache_required_then_raises_file_line_error(
    tmp_path: Path,
    test_case: CustomRuleTestCase,
) -> None:
    project: CompiledProjectView
    config: RulesConfig
    project, config = custom_rule_inputs(tmp_path=tmp_path, test_case=test_case)
    with pytest.raises(RulesError, match=test_case.expected_error_pattern):
        evaluate(project=project, config=config, project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleTestCase(
            description="project-wide rule reads project facts",
            body="del model\n    _ = ctx.declarations.public_enums\n    return []",
            project_wide=True,
            expected_cache_hits=2,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_wide_rule_when_reading_project_context_then_cache_is_allowed(
    tmp_path: Path,
    test_case: CustomRuleTestCase,
) -> None:
    project: CompiledProjectView
    config: RulesConfig
    project, config = custom_rule_inputs(tmp_path=tmp_path, test_case=test_case)

    _ = evaluate(project=project, config=config, project_dir=tmp_path)
    result: RulesResult = evaluate(project=project, config=config, project_dir=tmp_path)

    assert result.cache_hits == test_case.expected_cache_hits


@pytest.mark.parametrize(
    "test_case",
    (
        CustomRuleSuppressionTestCase(
            description="exact exception suppresses custom finding",
            rule_exceptions=(
                RuleExemption(
                    rule="XSQBRT101",
                    path="sql/models/mart/commerce__mart__orders.sql",
                    reason="Tracked custom-rule migration",
                ),
            ),
            rule_ignores=(),
            expected_fault_codes=(),
        ),
        CustomRuleSuppressionTestCase(
            description="scoped ignore suppresses custom finding",
            rule_exceptions=(),
            rule_ignores=(
                RuleIgnore(
                    rules=("XSQBRT",),
                    paths=("sql/models/mart/**",),
                    reason="Tracked custom-rule migration",
                ),
            ),
            expected_fault_codes=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_common_suppression_when_custom_rule_faults_then_native_policy_suppresses_it(
    tmp_path: Path,
    test_case: CustomRuleSuppressionTestCase,
) -> None:
    custom_rule_case: CustomRuleTestCase = CustomRuleTestCase(
        description=test_case.description,
        body="return [ctx.finding(subject=model)]",
    )
    project: CompiledProjectView
    config: RulesConfig
    project, config = custom_rule_inputs(tmp_path=tmp_path, test_case=custom_rule_case)
    relative_path: str = "sql/models/mart/commerce__mart__orders.sql"
    target: Path = tmp_path / relative_path
    target.parent.mkdir(parents=True)
    target.write_text("WITH final AS (SELECT 1 AS id) SELECT id FROM final\n", encoding="utf-8")
    config = replace(
        config,
        rule_exceptions=test_case.rule_exceptions,
        rule_ignores=test_case.rule_ignores,
    )

    result: RulesResult = evaluate(project=project, config=config, project_dir=tmp_path)

    assert tuple(fault.code for fault in result.findings) == test_case.expected_fault_codes
