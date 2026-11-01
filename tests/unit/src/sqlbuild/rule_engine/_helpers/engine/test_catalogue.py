"""Rule catalogue selection behavior tests."""

from pathlib import Path

import pytest

from sqlbuild._native import (
    Rule,
    RulesCacheConfig,
    RulesConfig,
    RulesResult,
    SqlTestRulesConfig,
    ThresholdOverride,
    build_rule_catalogue,
    format_rule,
    install_rules_skills,
    select_catalogue_rules,
)
from sqlbuild._native import evaluate_project_rules as evaluate
from tests.unit.src.sqlbuild.rule_engine._helpers.engine._test_types import (
    PolicyGuidanceTestCase,
    PolicySelectionTestCase,
    SqlTestPolicyGuidanceTestCase,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.engine.helpers import installed_skill
from tests.unit.src.sqlbuild.rule_engine.main.evaluate.helpers import build_project

RULES_BUILT_IN_CODES: tuple[str, ...] = (
    "SQBRCONTRACT101",
    "SQBRCONTRACT102",
    "SQBRCONTRACT103",
    "SQBRCONTRACT104",
    "SQBRCONTRACT105",
    "SQBRCONTRACT106",
    "SQBRDECLARATION101",
    "SQBRDECLARATION102",
    "SQBRDECLARATION201",
    "SQBRDECLARATION301",
    "SQBRDECLARATION302",
    "SQBRDECLARATION303",
    "SQBRDECLARATION304",
    "SQBRDECLARATION305",
    "SQBRDECLARATION306",
    "SQBRGRAPH101",
    "SQBRGRAPH102",
    "SQBRGRAPH103",
    "SQBRPROJECT101",
    "SQBRPROJECT102",
    "SQBRPROJECT103",
    "SQBRPROJECT104",
    "SQBRPROJECT105",
    "SQBRPROJECT106",
    "SQBRPROJECT107",
    "SQBRPROJECT201",
    "SQBRPROJECT202",
    "SQBRPROJECT203",
    "SQBRPROJECT204",
    "SQBRMODEL101",
    "SQBRMODEL102",
    "SQBRMODEL103",
    "SQBRMODEL104",
    "SQBRTEST101",
    "SQBRTEST102",
    "SQBRTEST103",
    "SQBRTEST104",
    "SQBRTEST201",
    "SQBRTEST202",
    "SQBRTEST203",
    "SQBRTEST301",
)
SQL_CODES: tuple[str, ...] = tuple(f"SQLCLEAN{number:03d}" for number in range(1, 54))
SQL_DEFAULT_CODES: tuple[str, ...] = tuple(
    f"SQLCLEAN{number:03d}"
    for number in (1, 2, 3, 4, 5, 6, 7, 9, 10, 18, 19, 20, 39, 46, 47, 48, 51, 52, 53)
)
BUILT_IN_CODES: tuple[str, ...] = tuple(sorted(RULES_BUILT_IN_CODES))
STRUCTURE_CODES: tuple[str, ...] = (
    "SQBRMODEL101",
    "SQBRMODEL102",
    "SQBRMODEL103",
    "SQBRMODEL104",
)
SQL_TEST_CODES: tuple[str, ...] = (
    "SQBRTEST101",
    "SQBRTEST102",
    "SQBRTEST103",
    "SQBRTEST104",
    "SQBRTEST201",
    "SQBRTEST202",
    "SQBRTEST203",
    "SQBRTEST301",
)
NON_STRUCTURE_CODES: tuple[str, ...] = tuple(
    sorted(
        (
            "SQBRCONTRACT101",
            "SQBRCONTRACT102",
            "SQBRCONTRACT103",
            "SQBRCONTRACT104",
            "SQBRCONTRACT105",
            "SQBRCONTRACT106",
            "SQBRDECLARATION101",
            "SQBRDECLARATION102",
            "SQBRDECLARATION201",
            "SQBRDECLARATION301",
            "SQBRDECLARATION302",
            "SQBRDECLARATION303",
            "SQBRDECLARATION304",
            "SQBRDECLARATION305",
            "SQBRDECLARATION306",
            "SQBRGRAPH101",
            "SQBRGRAPH102",
            "SQBRGRAPH103",
            "SQBRPROJECT101",
            "SQBRPROJECT102",
            "SQBRPROJECT103",
            "SQBRPROJECT104",
            "SQBRPROJECT105",
            "SQBRPROJECT106",
            "SQBRPROJECT107",
            "SQBRPROJECT201",
            "SQBRPROJECT202",
            "SQBRPROJECT203",
            "SQBRPROJECT204",
            *SQL_TEST_CODES,
        )
    )
)


@pytest.mark.parametrize(
    "test_case",
    (
        PolicySelectionTestCase(
            description="SQBR namespace activates every SQLBuild-owned built-in",
            select=("SQBR",),
            ignore=(),
            expected_codes=BUILT_IN_CODES,
        ),
        PolicySelectionTestCase(
            description="built-in family activates every family rule",
            select=("SQBRMODEL",),
            ignore=(),
            expected_codes=STRUCTURE_CODES,
        ),
        PolicySelectionTestCase(
            description="exact built-in code activates one rule",
            select=("SQBRMODEL101",),
            ignore=(),
            expected_codes=("SQBRMODEL101",),
        ),
        PolicySelectionTestCase(
            description="SQL test family activates every SQL test rule",
            select=("SQBRTEST",),
            ignore=(),
            expected_codes=SQL_TEST_CODES,
        ),
        PolicySelectionTestCase(
            description="no selection activates SQLClean's default SQL Rules only",
            select=None,
            ignore=(),
            expected_codes=SQL_DEFAULT_CODES,
        ),
        PolicySelectionTestCase(
            description="empty selection activates no rules",
            select=(),
            ignore=(),
            expected_codes=(),
        ),
        PolicySelectionTestCase(
            description="SQLCLEAN family activates every SQL Rule, opt-in ones too",
            select=("SQLCLEAN",),
            ignore=(),
            expected_codes=SQL_CODES,
        ),
        PolicySelectionTestCase(
            description="exact opt-in SQL code activates only that Rule",
            select=("SQLCLEAN050",),
            ignore=(),
            expected_codes=("SQLCLEAN050",),
        ),
        PolicySelectionTestCase(
            description="ignoring the SQLCLEAN family without a selection activates nothing",
            select=None,
            ignore=("SQLCLEAN",),
            expected_codes=(),
        ),
        PolicySelectionTestCase(
            description="ignored family is removed from selected namespace",
            select=("SQBR",),
            ignore=("SQBRMODEL",),
            expected_codes=NON_STRUCTURE_CODES,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_rule_selectors_when_resolving_then_returns_expected_codes(
    tmp_path: Path,
    test_case: PolicySelectionTestCase,
) -> None:
    config: RulesConfig = RulesConfig(select=test_case.select, ignore=test_case.ignore)
    catalogue: tuple[Rule, ...] = build_rule_catalogue(
        config=config, project_dir=tmp_path, include_custom=True
    )

    selected: tuple[Rule, ...] = select_catalogue_rules(
        catalogue=catalogue, config=config, project_dir=tmp_path
    )

    assert tuple(rule.code for rule in selected) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case",
    (
        SqlTestPolicyGuidanceTestCase(
            description="nested pipeline directory",
            pipeline_directory="chains/commerce",
            expected_path="tests/unit/chains/commerce/",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sql_test_policy_when_inspecting_and_rendering_skills_then_effective_paths_match(
    tmp_path: Path,
    test_case: SqlTestPolicyGuidanceTestCase,
) -> None:
    config: RulesConfig = RulesConfig(
        select=("SQBRTEST",),
        sql_tests=SqlTestRulesConfig(pipeline_directory=test_case.pipeline_directory),
    )
    catalogue: tuple[Rule, ...] = build_rule_catalogue(
        config=config, project_dir=tmp_path, include_custom=True
    )
    rules_by_code: dict[str, Rule] = {item.code: item for item in catalogue}
    rule: Rule = rules_by_code["SQBRTEST103"]

    inspection: str = format_rule(rule=rule, config=config)
    skill: str = installed_skill(config=config, project_dir=tmp_path)

    assert test_case.expected_path in inspection
    assert test_case.expected_path in skill


@pytest.mark.parametrize(
    "test_case",
    (
        PolicyGuidanceTestCase(
            description="minimum test guidance is identical across remediation inspection and skills",
            expected_snippets=(
                "TEST();",
                "__ref__upstream_model AS",
                "__expected__example_model AS",
                "Do not merely assert that inputs survive unchanged",
                "temporarily perturb the model logic or expected value",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_minimum_test_rule_when_rendering_guidance_then_all_surfaces_have_exact_parity(
    tmp_path: Path,
    test_case: PolicyGuidanceTestCase,
) -> None:
    override: ThresholdOverride = ThresholdOverride(
        paths=("models/mart/**",),
        thresholds={"min_tests_per_model": 2},
        reason="marts require two focused tests",
    )
    config: RulesConfig = RulesConfig(
        select=("SQBRTEST202",),
        threshold_overrides=(override,),
        cache=RulesCacheConfig(enabled=False),
    )
    catalogue: tuple[Rule, ...] = build_rule_catalogue(
        config=config, project_dir=tmp_path, include_custom=True
    )
    rules_by_code: dict[str, Rule] = {item.code: item for item in catalogue}
    rule: Rule = rules_by_code["SQBRTEST202"]
    result: RulesResult = evaluate(
        project=build_project(
            name="commerce__mart__orders",
            relative_path="models/mart/commerce__mart__orders.sql",
            sql="SELECT id + 1 AS id FROM orders",
            config_values={"materialized": "table"},
        ),
        config=config,
        project_dir=tmp_path,
    )
    inspection: str = format_rule(rule=rule, config=config)
    skill: str = installed_skill(config=config, project_dir=tmp_path)
    fresh_without_override: bool = install_rules_skills(
        config=RulesConfig(select=("SQBRTEST202",)), project_dir=tmp_path, check=True
    )

    assert result.findings[0].remediation == rule.remediation
    assert f"Remediation: {rule.remediation}" in inspection
    assert f"Remediation: {rule.remediation}" in skill
    assert "marts require two focused tests" in inspection
    assert "marts require two focused tests" in skill
    assert not fresh_without_override
    for snippet in test_case.expected_snippets:
        assert snippet in result.findings[0].remediation
