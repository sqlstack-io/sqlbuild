use std::collections::BTreeSet;

use fensu_facts::extension::models::ProgramHandle;
use fensu_native::{NativeProjectPlane, NativeRuleContext, evaluate_core_rules};

use crate::models::{RuleMetadata, RuleScope};
use crate::rules::_helpers::catalogue::{catalogue, select};

/// How many distinct `SQBRPYTHON` codes the catalogue holds.
pub(super) fn distinct_python_codes(rules: &[RuleMetadata]) -> usize {
    rules
        .iter()
        .filter(|rule| rule.code.starts_with("SQBRPYTHON"))
        .map(|rule| rule.code.as_str())
        .collect::<BTreeSet<&str>>()
        .len()
}

/// How many catalogue rules are project-wide Python rules.
pub(super) fn project_wide_python_rules(rules: &[RuleMetadata]) -> usize {
    rules
        .iter()
        .filter(|rule| {
            rule.scope == RuleScope::Python && rule.project_wide && rule.family == "python"
        })
        .count()
}

/// Whether selecting `select` runs the rule `code`.
pub(super) fn is_selected(selectors: &[String], code: &str) -> bool {
    let rules: Vec<RuleMetadata> = catalogue();
    select(&rules, Some(selectors), &[])
        .expect("valid selection")
        .iter()
        .any(|rule| rule.code == code)
}

/// The faults one Fensu core rule reports on a root-scope module, as code and line.
pub(super) fn core_rule_faults(source: &str, fensu_code: &str) -> Vec<(String, u32)> {
    let program: ProgramHandle = ProgramHandle::parse_source(source, 3, 12).expect("source parses");
    let context = NativeRuleContext {
        scope: "root".to_owned(),
        ..NativeRuleContext::default()
    };
    evaluate_core_rules(
        &program,
        &[fensu_code.to_owned()],
        &context,
        &NativeProjectPlane::new(Vec::new(), Vec::new()),
    )
    .expect("rule evaluates")
    .into_iter()
    .map(|fault| (fault.code, fault.line))
    .collect()
}
