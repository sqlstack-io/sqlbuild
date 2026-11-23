use std::collections::BTreeMap;
use std::path::Path;

use crate::models::{RulesConfig, ThresholdOverride};
use crate::rule_catalogue::models::{
    CatalogueRule, OptionKind, OptionSpec, OptionText, OptionValue, RuleText, SkillTarget,
};
use crate::rule_catalogue::tests::test_types::InstallTestCase;

pub(crate) fn integer_option(default: i64) -> OptionSpec {
    OptionSpec {
        name: "limit".into(),
        description: "The limit.".into(),
        value_type: OptionKind::Int,
        default: OptionValue::Int(default),
        choices: vec![],
        minimum: Some(1),
        maximum: Some(5),
        minimum_items: None,
        item_type: None,
    }
}

pub(crate) fn catalogue_rule(code: &str, options: Vec<OptionSpec>) -> CatalogueRule {
    CatalogueRule {
        code: code.into(),
        options,
    }
}

pub(crate) fn configured(
    code: &str,
    value: OptionValue,
) -> BTreeMap<String, BTreeMap<String, OptionValue>> {
    BTreeMap::from([(code.into(), BTreeMap::from([("limit".into(), value)]))])
}

pub(crate) fn custom_rule() -> RuleText {
    RuleText {
        code: "XSQBRTEST201".into(),
        family: "XSQBRTEST".into(),
        slug: "custom-check".into(),
        message: "Check it.".into(),
        remediation: "Fix it.".into(),
        custom: true,
        source: Some("rules/check.py".into()),
        options: vec![OptionText {
            name: "names".into(),
            description: "Names.".into(),
            default_repr: "('a',)".into(),
            effective_repr: "('b', \"it's\")".into(),
        }],
        ..RuleText::default()
    }
}

pub(crate) fn overridden_config() -> RulesConfig {
    RulesConfig {
        thresholds: BTreeMap::from([("min_tests_per_model".into(), 3)]),
        threshold_overrides: vec![ThresholdOverride {
            paths: vec!["sql/models/a".into()],
            thresholds: BTreeMap::from([("min_audits_per_model".into(), 2)]),
            reason: "Critical.".into(),
        }],
        ..RulesConfig::default()
    }
}

/// Two skill targets under `root`, seeded with the case's existing content.
pub(crate) fn skill_targets(root: &Path, test_case: &InstallTestCase) -> [SkillTarget; 2] {
    let targets = [root.join("a/SKILL.md"), root.join("b/SKILL.md")].map(|path| SkillTarget {
        path,
        state: test_case.state.into(),
    });
    std::fs::create_dir_all(root.join("a")).expect("a first skill folder");
    let _ = test_case
        .existing
        .map(|existing| std::fs::write(&targets[0].path, existing).expect("a first skill"));
    let _ = test_case
        .blocked_second
        .then(|| std::fs::write(root.join("b"), "").expect("a blocking file"));
    let _ = test_case
        .existing
        .filter(|_| !test_case.blocked_second)
        .map(|existing| {
            std::fs::create_dir_all(root.join("b")).expect("a second skill folder");
            std::fs::write(&targets[1].path, existing).expect("a second skill")
        });
    targets
}

/// The number of staged temporary or backup files left beside the targets.
pub(crate) fn staged_leftovers(root: &Path) -> usize {
    ["a", "b"]
        .iter()
        .filter_map(|folder| std::fs::read_dir(root.join(folder)).ok())
        .flatten()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with('.'))
        .count()
}
