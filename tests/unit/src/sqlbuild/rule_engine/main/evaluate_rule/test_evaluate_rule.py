"""Public custom-rule harness behavior tests."""

import inspect
from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import (
    MIN_CUSTOM_RULE_TEST_CASES,
    RulesCacheConfig,
    RulesConfig,
    RulesError,
    RulesResult,
)
from sqlbuild._native import evaluate_project_rules as evaluate
from sqlbuild.compiler.compile.types import CompiledProjectView
from sqlbuild.rule_engine.types import RuleOptionValue
from sqlbuild.rules.testing import RuleCase, RuleFile, RuleResult, evaluate_rule
from tests.unit.src.sqlbuild.rule_engine.main.evaluate.helpers import build_project
from tests.unit.src.sqlbuild.rule_engine.main.evaluate_rule._test_types import (
    EnumTypesRuleTestCase,
    EvaluateRuleDeterminismTestCase,
    EvaluateRuleParityTestCase,
    EvaluateRuleTestCase,
)
from tests.unit.src.sqlbuild.rule_engine.main.evaluate_rule.helpers import (
    first_evaluation_only,
    required_domain,
    status_enum,
)


@pytest.mark.parametrize(
    "test_case",
    [
        EvaluateRuleTestCase(
            description="wrong configured domain faults through real pipeline",
            rule_case=RuleCase(
                description="wrong configured domain faults",
                source=(
                    "MODEL (description 'Test model.', materialized table);\n\n"
                    "WITH final AS (SELECT 1 AS id)\n"
                    "SELECT id FROM final\n"
                ),
                path="sql/models/mart/commerce__mart__orders.sql",
                config={"required_domain": "support"},
                expected_finding_count=1,
            ),
            expected_code="XSQBRD001",
            expected_path="sql/models/mart/commerce__mart__orders.sql",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_rule_case_when_evaluating_then_runs_real_pipeline(
    test_case: EvaluateRuleTestCase,
) -> None:
    result: RuleResult = evaluate_rule(rule=required_domain, test_case=test_case.rule_case)

    assert result.finding_count == 1
    assert result.findings[0].code == test_case.expected_code
    assert result.findings[0].path.as_posix() == test_case.expected_path


@pytest.mark.parametrize(
    "test_case",
    (
        EvaluateRuleParityTestCase(
            description="public and native custom paths return deterministic full fault parity",
            rule_case=RuleCase(
                description="wrong configured domain faults",
                source=(
                    "MODEL (description 'Test model.', materialized table);\n\n"
                    "WITH final AS (SELECT 1 AS id)\n"
                    "SELECT id FROM final\n"
                ),
                path="sql/models/mart/commerce__mart__orders.sql",
                config={"required_domain": "support"},
                expected_finding_count=1,
            ),
            expected_finding_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_same_custom_rule_when_using_public_and_native_paths_then_faults_have_full_parity(
    tmp_path: Path,
    test_case: EvaluateRuleParityTestCase,
) -> None:
    rule_case: RuleCase = test_case.rule_case
    public_result: RuleResult = evaluate_rule(rule=required_domain, test_case=rule_case)
    project: CompiledProjectView = build_project(
        name="commerce__mart__orders",
        relative_path=rule_case.path,
        sql="WITH final AS (SELECT 1 AS id) SELECT id FROM final",
        authored_sql=rule_case.source,
        config_values={"materialized": "table"},
    )
    source_file: str | None = inspect.getsourcefile(required_domain)
    assert source_file is not None
    rule_path: Path = tmp_path / "rules" / "custom.py"
    rule_path.parent.mkdir(parents=True)
    rule_path.write_text(Path(source_file).read_text(encoding="utf-8"), encoding="utf-8")
    config: RulesConfig = RulesConfig(
        select=("XSQBRD001",),
        thresholds={MIN_CUSTOM_RULE_TEST_CASES: 0},
        rule_options={
            "XSQBRD001": cast("dict[str, RuleOptionValue]", rule_case.config),
        },
        cache=RulesCacheConfig(enabled=False),
    )

    native_result: RulesResult = evaluate(
        project=project,
        config=config,
        project_dir=tmp_path,
    )
    repeated_native_result: RulesResult = evaluate(
        project=project,
        config=config,
        project_dir=tmp_path,
    )

    assert len(native_result.findings) == test_case.expected_finding_count
    assert native_result.findings == public_result.findings
    assert repeated_native_result.findings == native_result.findings
    assert native_result.findings[0] == public_result.findings[0]


@pytest.mark.parametrize(
    "test_case",
    [
        EvaluateRuleDeterminismTestCase(
            description="module-level state leaking between evaluations fails the harness",
            rule_case=RuleCase(
                description="first evaluation reports nothing",
                source="MODEL (description 'Test model.', materialized table);\n\nSELECT 1 AS id\n",
                path="sql/models/mart/commerce__mart__orders.sql",
                expected_finding_count=0,
            ),
            expected_error_pattern=(
                r"custom rule XSQBRS001 is not deterministic: evaluating every subject again "
                r"after the others changed its findings from 0 to 1"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_rule_with_module_state_when_evaluating_case_then_determinism_check_fails(
    test_case: EvaluateRuleDeterminismTestCase,
) -> None:
    with pytest.raises(RulesError, match=test_case.expected_error_pattern):
        evaluate_rule(rule=first_evaluation_only, test_case=test_case.rule_case)


@pytest.mark.parametrize(
    "test_case",
    (
        EnumTypesRuleTestCase(
            description="an enum-typed column reports its enum",
            status_type="order_status",
            expected_finding_count=0,
        ),
        EnumTypesRuleTestCase(
            description="a plain VARCHAR column reports no enum",
            status_type="VARCHAR",
            expected_finding_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_declared_status_column_when_rule_reads_enum_types_then_sees_declared_enum(
    test_case: EnumTypesRuleTestCase,
) -> None:
    rule_case: RuleCase = RuleCase(
        description=test_case.description,
        source=(
            "MODEL (\n  description 'Orders.',\n"
            f"  columns (\n    status (type {test_case.status_type}),\n  ),\n);\n\n"
            "SELECT 'open' AS status\n"
        ),
        path="sql/models/orders.sql",
        files=(
            RuleFile(
                path="sql/models/_enums/order_status.sql",
                source='ENUM (name order_status, members (OPEN "open", CLOSED "closed"));\n',
            ),
        ),
        expected_finding_count=test_case.expected_finding_count,
    )

    result: RuleResult = evaluate_rule(rule=status_enum, test_case=rule_case)

    assert result.finding_count == test_case.expected_finding_count
