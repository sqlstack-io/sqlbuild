use std::collections::BTreeMap;

use crate::models::RulesConfig;
use crate::rule_catalogue::main::format_rule_text::format_rule_text;
use crate::rule_catalogue::main::install_rendered_skills::install_rendered_skills;
use crate::rule_catalogue::main::render_skills_body::render_skills_body;
use crate::rule_catalogue::main::sorted_catalogue_order::sorted_catalogue_order;
use crate::rule_catalogue::models::{OptionValue, RuleText, SkillTarget};
use crate::rule_catalogue::tests::helpers::{
    catalogue_rule, configured, custom_rule, integer_option, overridden_config, skill_targets,
    staged_leftovers,
};
use crate::rule_catalogue::tests::test_types::{
    CatalogueTestCase, InstallTestCase, RuleTextTestCase, SkillsBodyTestCase,
};

#[test]
fn given_catalogue_when_validating_then_orders_or_rejects_like_python() {
    let test_cases = [
        CatalogueTestCase {
            description: "valid rules sort by code",
            rules: vec![
                catalogue_rule("XSQBRB", vec![]),
                catalogue_rule("SQBRA", vec![]),
            ],
            configured: BTreeMap::new(),
            expected_result: Ok(vec![1, 0]),
        },
        CatalogueTestCase {
            description: "duplicate codes are rejected",
            rules: vec![
                catalogue_rule("SQBRA", vec![]),
                catalogue_rule("SQBRA", vec![]),
            ],
            configured: BTreeMap::new(),
            expected_result: Err("duplicate rule codes: SQBRA"),
        },
        CatalogueTestCase {
            description: "a bool is not an integer option value",
            rules: vec![catalogue_rule("XSQBRA", vec![integer_option(2)])],
            configured: configured("XSQBRA", OptionValue::Bool(true)),
            expected_result: Err("rule XSQBRA option limit has the wrong type"),
        },
        CatalogueTestCase {
            description: "configured values respect the maximum",
            rules: vec![catalogue_rule("XSQBRA", vec![integer_option(2)])],
            configured: configured("XSQBRA", OptionValue::Int(9)),
            expected_result: Err("rule XSQBRA option limit exceeds its maximum"),
        },
        CatalogueTestCase {
            description: "defaults respect the minimum",
            rules: vec![catalogue_rule("XSQBRA", vec![integer_option(0)])],
            configured: BTreeMap::new(),
            expected_result: Err("rule XSQBRA option limit default is below its minimum"),
        },
        CatalogueTestCase {
            description: "options for unknown codes are rejected",
            rules: vec![catalogue_rule("XSQBRA", vec![integer_option(2)])],
            configured: configured("XSQBRZ", OptionValue::Int(2)),
            expected_result: Err("rule options target unknown codes: XSQBRZ"),
        },
    ];
    for test_case in test_cases {
        let result = sorted_catalogue_order(&test_case.rules, &test_case.configured);
        assert_eq!(
            result,
            test_case.expected_result.map_err(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_rule_when_formatting_then_matches_python_inspection_text() {
    let test_cases = [
        RuleTextTestCase {
            description: "a custom rule with options and no config",
            rule: custom_rule(),
            with_config: false,
            expected_text: "XSQBRTEST201: custom-check\nFamily: XSQBRTEST\nEnabled by default: no\nKind: custom\n\nCheck it.\n\nRemediation: Fix it.\n\nSource: rules/check.py\n\nOptions:\n- names: default=('a',) (Names.)",
        },
        RuleTextTestCase {
            description: "a test rule with config shows test paths only",
            rule: RuleText {
                code: "SQBRTEST101".into(),
                enabled_by_default: true,
                ..RuleText::default()
            },
            with_config: true,
            expected_text: "SQBRTEST101: \nFamily: \nEnabled by default: yes\nKind: built-in\n\n\n\nRemediation: \n\nEffective SQL test paths:\n- Unit tests: tests/unit/\n- Scenarios: tests/scenarios/\n- Cross-domain pipelines: tests/unit/pipelines/",
        },
        RuleTextTestCase {
            description: "a minimum rule shows effective thresholds and overrides",
            rule: RuleText {
                code: "SQBRTEST202".into(),
                ..RuleText::default()
            },
            with_config: true,
            expected_text: "SQBRTEST202: \nFamily: \nEnabled by default: no\nKind: built-in\n\n\n\nRemediation: \n\nEffective thresholds:\n- `max_constant_container_files` = `10`\n- `max_enum_container_files` = `10`\n- `max_macro_container_files` = `10`\n- `max_role_container_depth` = `1`\n- `max_subdomain_depth` = `1`\n- `min_audits_per_model` = `1`\n- `min_custom_rule_test_cases` = `1`\n- `min_shared_container_prefix_files` = `2`\n- `min_shared_owner_prefix_directories` = `2`\n- `min_tests_per_model` = `3`\n- Override 1 at `sql/models/a`: `{'min_audits_per_model': 2}` (Critical.)\n- Matching overrides apply in authored order; the last match wins.\n\nEffective SQL test paths:\n- Unit tests: tests/unit/\n- Scenarios: tests/scenarios/\n- Cross-domain pipelines: tests/unit/pipelines/",
        },
    ];
    let config: RulesConfig = overridden_config();
    for test_case in test_cases {
        let text: String =
            format_rule_text(&test_case.rule, test_case.with_config.then_some(&config));
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_rules_when_rendering_skills_then_lists_rules_and_sections() {
    let test_cases = [
        SkillsBodyTestCase {
            description: "custom options and test paths",
            rules: vec![custom_rule()],
            expected_lines: vec![
                "### XSQBRTEST201: custom-check",
                "Correct shape: Apply `custom-check` at the SQL node named by the finding.",
                "Effective options: {'names': ('b', \"it's\")}",
                "## Active Rules",
            ],
        },
        SkillsBodyTestCase {
            description: "a built-in example and the layout section",
            rules: vec![RuleText {
                code: "SQBRPROJECT203".into(),
                slug: "depth".into(),
                ..RuleText::default()
            }],
            expected_lines: vec![
                "Correct shape: domain/level/subdomain/model.sql at the default depth of one.",
                "## Effective Thresholds",
                "## Owner Layout",
                "- Configured levels: `staging, intermediate/clean, intermediate/enriched, mart`",
            ],
        },
    ];
    for test_case in test_cases {
        let body: String = render_skills_body(&test_case.rules, &RulesConfig::default());
        let lines: Vec<&str> = body.lines().collect();
        let found: Vec<&str> = test_case
            .expected_lines
            .iter()
            .copied()
            .filter(|line| lines.contains(line))
            .collect();
        assert_eq!(found, test_case.expected_lines, "{}", test_case.description);
    }
}

#[test]
fn given_skill_targets_when_installing_then_replaces_only_owned_content() {
    let test_cases = [
        InstallTestCase {
            description: "missing targets are created",
            existing: None,
            state: "missing",
            blocked_second: false,
            expected_error: None,
            expected_contents: [Some("new"), Some("new")],
        },
        InstallTestCase {
            description: "stale targets are replaced",
            existing: Some("old"),
            state: "stale",
            blocked_second: false,
            expected_error: None,
            expected_contents: [Some("new"), Some("new")],
        },
        InstallTestCase {
            description: "foreign content is refused and kept",
            existing: Some("mine"),
            state: "foreign",
            blocked_second: false,
            expected_error: Some("refusing to overwrite foreign rules skill: "),
            expected_contents: [Some("mine"), Some("mine")],
        },
        InstallTestCase {
            description: "a later staging failure replaces no target",
            existing: Some("old"),
            state: "stale",
            blocked_second: true,
            expected_error: Some(""),
            expected_contents: [Some("old"), None],
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("a temporary folder");
        let targets: [SkillTarget; 2] = skill_targets(directory.path(), &test_case);
        let error: Option<String> = install_rendered_skills("new", &targets)
            .err()
            .map(|error| error.message);
        assert_eq!(
            error.map(|message| message.starts_with(test_case.expected_error.unwrap_or("-"))),
            test_case.expected_error.map(|_| true),
            "{}",
            test_case.description
        );
        let contents: Vec<Option<String>> = targets
            .iter()
            .map(|target| std::fs::read_to_string(&target.path).ok())
            .collect();
        let expected: Vec<Option<String>> = test_case
            .expected_contents
            .iter()
            .map(|content| content.map(str::to_owned))
            .collect();
        assert_eq!(contents, expected, "{}", test_case.description);
        assert_eq!(
            staged_leftovers(directory.path()),
            0,
            "{}",
            test_case.description
        );
    }
}
