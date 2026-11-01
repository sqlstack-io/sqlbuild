"""Strict rules configuration behavior tests."""

from pathlib import Path

import pytest

from sqlbuild._native import (
    LayoutConfig,
    RulesConfig,
    RulesConfigError,
    RulesError,
    SqlTestRulesConfig,
    load_rules_config,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.engine._test_types import (
    PolicyLayoutConfigTestCase,
    RelationExceptionConfigTestCase,
    RuleIgnoreConfigTestCase,
    RulesConfigErrorTestCase,
    SqlTestRulesConfigTestCase,
)

_EXCEPTION_ENTRY: str = (
    '[[rules.rule_exceptions]]\nrule = "SQLCLEAN004"\npath = "models/orders.sql"\n'
    'reason = "A reviewed sample"\n'
)
_IGNORE_ENTRY: str = (
    '[[rules.rule_ignores]]\nrules = ["SQLCLEAN"]\npaths = ["models/examples/**"]\n'
    'reason = "Examples keep minimal SQL"\n'
)


@pytest.mark.parametrize(
    "test_case",
    (
        RulesConfigErrorTestCase(
            description="empty layout levels",
            source="[rules.layout]\nlevels = []\n",
            expected_error_pattern="rules.layout.levels must contain at least one",
        ),
        RulesConfigErrorTestCase(
            description="traversing layout level",
            source='[rules.layout]\nlevels = ["../staging"]\n',
            expected_error_pattern="must be normalized project-relative paths",
        ),
        RulesConfigErrorTestCase(
            description="duplicate layout level",
            source='[rules.layout]\nlevels = ["staging", "staging"]\n',
            expected_error_pattern="duplicate rules layout level: staging",
        ),
        RulesConfigErrorTestCase(
            description="overlapping layout levels",
            source=('[rules.layout]\nlevels = ["intermediate", "intermediate/enriched"]\n'),
            expected_error_pattern="rules.layout.levels entries must not overlap",
        ),
        RulesConfigErrorTestCase(
            description="overlapping explicit domain roots",
            source=('[rules.layout]\ndomain_roots = ["sales", "sales/partner"]\n'),
            expected_error_pattern="rules.layout.domain_roots entries must not overlap",
        ),
        RulesConfigErrorTestCase(
            description="unknown top-level key",
            source='[rules]\nselect = ["SQBRMODEL"]\nseverity = "warn"\n',
            expected_error_pattern="unknown rules config keys: severity",
        ),
        RulesConfigErrorTestCase(
            description="removed legacy rules section",
            source='[kata]\nselect = ["SQBK"]\n',
            expected_error_pattern="unsupported legacy rules configuration",
        ),
        RulesConfigErrorTestCase(
            description="unknown threshold",
            source="[rules.thresholds]\nmax_models = 3\n",
            expected_error_pattern="unknown rule thresholds: max_models",
        ),
        RulesConfigErrorTestCase(
            description="reasonless ignore",
            source=('[[rules.rule_ignores]]\nrules = ["SQBRMODEL"]\npaths = ["models/**"]\n'),
            expected_error_pattern="rules.rule_ignores.reason",
        ),
        RulesConfigErrorTestCase(
            description="ignore without resource scope",
            source=(
                '[[rules.rule_ignores]]\nrules = ["SQBRMODEL"]\nreason = "A reviewed exception"\n'
            ),
            expected_error_pattern="requires rules, paths or selectors, and reason",
        ),
        RulesConfigErrorTestCase(
            description="ignore with empty selector",
            source=(
                '[[rules.rule_ignores]]\nrules = ["SQBRMODEL"]\nselectors = [""]\n'
                'reason = "A reviewed exception"\n'
            ),
            expected_error_pattern="rule ignore selectors must be non-empty",
        ),
        RulesConfigErrorTestCase(
            description="ignore with empty path",
            source=(
                '[[rules.rule_ignores]]\nrules = ["SQBRMODEL"]\npaths = [""]\n'
                'reason = "A reviewed exception"\n'
            ),
            expected_error_pattern="rule ignore paths must be non-empty",
        ),
        RulesConfigErrorTestCase(
            description="reasonless threshold override",
            source=(
                '[[rules.threshold_overrides]]\npaths = ["models/mart/**"]\n'
                "thresholds = { min_tests_per_model = 2 }\n"
            ),
            expected_error_pattern="rules.threshold_overrides.reason",
        ),
        RulesConfigErrorTestCase(
            description="unknown scoped threshold",
            source=(
                '[[rules.threshold_overrides]]\npaths = ["models/mart/**"]\n'
                'reason = "marts need stronger coverage"\n'
                "thresholds = { max_models = 2 }\n"
            ),
            expected_error_pattern="unknown rule threshold override: max_models",
        ),
        RulesConfigErrorTestCase(
            description="project-wide threshold used as path override",
            source=(
                '[[rules.threshold_overrides]]\npaths = ["models/mart/**"]\n'
                'reason = "custom rule evidence is project-wide"\n'
                "thresholds = { min_custom_rule_test_cases = 2 }\n"
            ),
            expected_error_pattern=("unknown rule threshold override: min_custom_rule_test_cases"),
        ),
        RulesConfigErrorTestCase(
            description="invalid threshold override glob",
            source=(
                '[[rules.threshold_overrides]]\npaths = ["models/["]\n'
                'reason = "invalid migration pattern"\n'
                "thresholds = { min_tests_per_model = 2 }\n"
            ),
            expected_error_pattern="invalid rule threshold override path",
        ),
        RulesConfigErrorTestCase(
            description="empty threshold override glob",
            source=(
                '[[rules.threshold_overrides]]\npaths = [""]\n'
                'reason = "empty migration pattern"\n'
                "thresholds = { min_tests_per_model = 2 }\n"
            ),
            expected_error_pattern="threshold override paths must be non-empty globs",
        ),
        RulesConfigErrorTestCase(
            description="transitional rule namespace",
            source='[rules]\nselect = ["KTS"]\n',
            expected_error_pattern="malformed rule selector: KTS",
        ),
        RulesConfigErrorTestCase(
            description="absolute pipeline directory",
            source='[rules.sql_tests]\npipeline_directory = "/pipelines"\n',
            expected_error_pattern="must be a normalized path relative to tests/unit",
        ),
        RulesConfigErrorTestCase(
            description="traversing pipeline directory",
            source='[rules.sql_tests]\npipeline_directory = "../pipelines"\n',
            expected_error_pattern="must be a normalized path relative to tests/unit",
        ),
        RulesConfigErrorTestCase(
            description="unnormalized pipeline directory",
            source='[rules.sql_tests]\npipeline_directory = "chains//commerce"\n',
            expected_error_pattern="must be a normalized path relative to tests/unit",
        ),
        RulesConfigErrorTestCase(
            description="relation for a Rule that reports no relation",
            source=(
                '[[rules.rule_exceptions]]\nrule = "SQLCLEAN004"\npath = "models/orders.sql"\n'
                'relation = "customers"\nreason = "A reviewed sample"\n'
            ),
            expected_error_pattern="sets relation, but only SQLCLEAN038 report a joined relation",
        ),
        RulesConfigErrorTestCase(
            description="cte without relation",
            source=(
                '[[rules.rule_exceptions]]\nrule = "SQLCLEAN038"\npath = "models/orders.sql"\n'
                'cte = "checks"\nreason = "Deliberate"\n'
            ),
            expected_error_pattern="sets cte without relation",
        ),
        RulesConfigErrorTestCase(
            description="unknown published layer",
            source='[rules.rule_options.SQBRGRAPH103]\npublished_layers = ["gold"]\n',
            expected_error_pattern="published_layers entries must be model name layers",
        ),
        RulesConfigErrorTestCase(
            description="allowed qualifier without a reason",
            source=(
                "[rules.rule_options.SQBRPROJECT107]\n"
                'allowed_qualifiers = [{ qualifier = "csv_export" }]\n'
            ),
            expected_error_pattern="allowed_qualifiers entries must be",
        ),
        RulesConfigErrorTestCase(
            description="allowed qualifier entry with neither qualifier nor paths",
            source=(
                "[rules.rule_options.SQBRPROJECT107]\n"
                'allowed_qualifiers = [{ domain = "sales", reason = "Regional exports." }]\n'
            ),
            expected_error_pattern="allowed_qualifiers entries must be",
        ),
        RulesConfigErrorTestCase(
            description="allowed qualifier paths that are not a list",
            source=(
                "[rules.rule_options.SQBRPROJECT107]\n"
                'allowed_qualifiers = [{ paths = "models/sales/*", reason = "Exports." }]\n'
            ),
            expected_error_pattern="allowed_qualifiers entries must be",
        ),
        RulesConfigErrorTestCase(
            description="allowed qualifier with an empty paths list",
            source=(
                "[rules.rule_options.SQBRPROJECT107]\n"
                'allowed_qualifiers = [{ paths = [], reason = "Exports." }]\n'
            ),
            expected_error_pattern="need a non-empty list of non-empty paths",
        ),
        RulesConfigErrorTestCase(
            description="allowed paths entry with an empty reason",
            source=(
                "[rules.rule_options.SQBRPROJECT107]\n"
                'allowed_qualifiers = [{ paths = ["models/sales/*"], reason = " " }]\n'
            ),
            expected_error_pattern=r"allowed_qualifiers entries need a non-empty reason$",
        ),
        RulesConfigErrorTestCase(
            description="allowed qualifier that is empty",
            source=(
                "[rules.rule_options.SQBRPROJECT107]\n"
                'allowed_qualifiers = [{ qualifier = "", reason = "Exports." }]\n'
            ),
            expected_error_pattern=r"allowed_qualifiers entries need a non-empty qualifier$",
        ),
        RulesConfigErrorTestCase(
            description="allowed path with backslash separators",
            source=(
                "[rules.rule_options.SQBRPROJECT107]\n"
                "allowed_qualifiers = [{ paths = ['models\\sales\\*'], reason = \"Exports.\" }]\n"
            ),
            expected_error_pattern=r"paths must be project-relative globs with / separators",
        ),
        RulesConfigErrorTestCase(
            description="allowed path listed twice in one entry",
            source=(
                "[rules.rule_options.SQBRPROJECT107]\n"
                'allowed_qualifiers = [{ paths = ["models/sales/*", "models/sales/*"], '
                'reason = "Exports." }]\n'
            ),
            expected_error_pattern=r"allowed_qualifiers entry lists path models/sales/\* twice",
        ),
        RulesConfigErrorTestCase(
            description="duplicate allowed qualifier glob",
            source=(
                "[rules.rule_options.SQBRPROJECT107]\n"
                "allowed_qualifiers = [\n"
                '  { qualifier = "[a-z][a-z][a-z]", reason = "Per country." },\n'
                '  { qualifier = "[a-z][a-z][a-z]", reason = "Per region." },\n'
                "]\n"
            ),
            expected_error_pattern=r"duplicate .*allowed_qualifiers entry: \[a-z\]\[a-z\]\[a-z\]",
        ),
        RulesConfigErrorTestCase(
            description="unknown SQL test rules key",
            source='[rules.sql_tests]\npipeline_directory = "pipelines"\nroot = "tests"\n',
            expected_error_pattern="unknown field `root`",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_rules_config_when_loading_then_raises_clear_error(
    tmp_path: Path,
    test_case: RulesConfigErrorTestCase,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(test_case.source, encoding="utf-8")

    with pytest.raises(RulesError, match=test_case.expected_error_pattern):
        load_rules_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (
        RulesConfigErrorTestCase(
            description="false",
            source=f"[rules]\nallow_exceptions = false\n{_EXCEPTION_ENTRY}",
            expected_error_pattern=(
                r"sets \[rules\] allow_exceptions, which SQLBuild no longer supports"
            ),
        ),
        RulesConfigErrorTestCase(
            description="true",
            source="[rules]\nallow_exceptions = true\n",
            expected_error_pattern=r"inline -- sqb: ignore suppressions were removed",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_removed_allow_exceptions_when_loading_then_raises_error_explaining_the_removal(
    tmp_path: Path,
    test_case: RulesConfigErrorTestCase,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(test_case.source, encoding="utf-8")

    with pytest.raises(RulesConfigError, match=test_case.expected_error_pattern) as raised:
        load_rules_config(project_dir=tmp_path)

    assert "remove the allow_exceptions line from [rules]" in raised.value.help
    assert "[[rules.rule_exceptions]]" in raised.value.help


@pytest.mark.parametrize(
    "test_case",
    (
        RelationExceptionConfigTestCase(
            description="file-wide exception and scoped ignore",
            source=_EXCEPTION_ENTRY + _IGNORE_ENTRY,
            expected_relation=None,
            expected_cte=None,
        ),
        RelationExceptionConfigTestCase(
            description="relation-level exception narrowed to a CTE",
            source=(
                '[[rules.rule_exceptions]]\nrule = "SQLCLEAN038"\npath = "models/orders.sql"\n'
                'relation = "order_policy"\ncte = "order_checks"\n'
                'reason = "Every order is checked against every policy"\n'
            ),
            expected_relation="order_policy",
            expected_cte="order_checks",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_rule_exceptions_when_loading_then_config_records_their_scope(
    tmp_path: Path,
    test_case: RelationExceptionConfigTestCase,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(test_case.source, encoding="utf-8")

    config: RulesConfig = load_rules_config(project_dir=tmp_path)

    assert config.rule_exceptions[0].relation == test_case.expected_relation
    assert config.rule_exceptions[0].cte == test_case.expected_cte


@pytest.mark.parametrize(
    "test_case",
    (
        SqlTestRulesConfigTestCase(
            description="nested pipeline directory",
            source='[rules.sql_tests]\npipeline_directory = "chains/commerce"\n',
            expected_pipeline_directory="chains/commerce",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_pipeline_directory_when_loading_then_returns_typed_relative_configuration(
    tmp_path: Path,
    test_case: SqlTestRulesConfigTestCase,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(test_case.source, encoding="utf-8")

    config: RulesConfig = load_rules_config(project_dir=tmp_path)

    assert config.sql_tests == SqlTestRulesConfig(
        pipeline_directory=test_case.expected_pipeline_directory
    )


@pytest.mark.parametrize(
    "test_case",
    (
        RuleIgnoreConfigTestCase(
            description="graph selectors and paths can share one scoped ignore",
            source=(
                '[[rules.rule_ignores]]\nrules = ["SQLCLEAN021"]\n'
                'paths = ["tests/**"]\nselectors = ["+orders", "tag:interface+"]\n'
                'reason = "Reviewed interface resources"\n'
            ),
            expected_paths=("tests/**",),
            expected_selectors=("+orders", "tag:interface+"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scoped_ignore_selectors_when_loading_then_returns_typed_configuration(
    test_case: RuleIgnoreConfigTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(test_case.source, encoding="utf-8")

    config: RulesConfig = load_rules_config(project_dir=tmp_path)

    assert config.rule_ignores[0].paths == test_case.expected_paths
    assert config.rule_ignores[0].selectors == test_case.expected_selectors


@pytest.mark.parametrize(
    "test_case",
    (
        PolicyLayoutConfigTestCase(
            description="custom levels and thresholds",
            source=(
                '[rules.layout]\nlevels = ["raw", "conformed/clean", "reporting"]\n'
                'domain_roots = ["sales/partner", "inventory/forecasting"]\n'
                "[rules.thresholds]\n"
                "max_subdomain_depth = 2\n"
                "min_shared_owner_prefix_directories = 2\n"
                "max_role_container_depth = 1\n"
                "max_macro_container_files = 12\n"
                "max_constant_container_files = 8\n"
                "max_enum_container_files = 6\n"
                "min_shared_container_prefix_files = 2\n"
            ),
            expected_levels=("raw", "conformed/clean", "reporting"),
            expected_thresholds={
                "max_subdomain_depth": 2,
                "min_shared_owner_prefix_directories": 2,
                "max_role_container_depth": 1,
                "max_macro_container_files": 12,
                "max_constant_container_files": 8,
                "max_enum_container_files": 6,
                "min_shared_container_prefix_files": 2,
            },
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_layout_and_thresholds_when_loading_then_returns_typed_configuration(
    test_case: PolicyLayoutConfigTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        test_case.source,
        encoding="utf-8",
    )

    config: RulesConfig = load_rules_config(project_dir=tmp_path)

    assert config.layout == LayoutConfig(
        levels=test_case.expected_levels,
        domain_roots=("sales/partner", "inventory/forecasting"),
    )
    assert config.thresholds == test_case.expected_thresholds
