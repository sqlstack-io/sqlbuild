//! The generated rules skill body for the selected rules.

use crate::models::RulesConfig;
use crate::rule_catalogue::_helpers::python_repr::dict_repr;
use crate::rule_catalogue::_helpers::thresholds::threshold_lines;
use crate::rule_catalogue::constants::{
    GRAPH_EDGE_RULE_CODE, LAYOUT_RULE_PREFIXES, LAYOUT_THRESHOLD_RULE_CODES, MINIMUM_RULES_CODES,
    RULE_EXAMPLES, TEST_RULE_PREFIX,
};
use crate::rule_catalogue::models::RuleText;

/// Stable guidance text for `rules` under `config`, ending in one newline.
pub fn render_skills_body(rules: &[RuleText], config: &RulesConfig) -> String {
    let mut body: Vec<String> = [
        "# SQLBuild Rules",
        "",
        "Rules enforce SQL model architecture and never modifies SQL files.",
        "Run `sqb rules run SQBR` and `sqb rules run SQLCLEAN` before handing off model changes.",
        "Fix findings in SQL. A deliberate deviation is a reviewed [[rules.rule_exceptions]] entry with a reason in sqlbuild_project.toml; inline `-- sqb: ignore` comments are errors.",
        "",
        "## Active Rules",
    ]
    .map(String::from)
    .to_vec();
    for rule in rules {
        body.extend([
            String::new(),
            format!("### {}: {}", rule.code, rule.slug),
            String::new(),
            rule.message.clone(),
            String::new(),
            format!("Correct shape: {}", rule_example(rule)),
            String::new(),
            format!("Remediation: {}", rule.remediation),
        ]);
        if !rule.options.is_empty() {
            let effective: String = dict_repr(
                rule.options
                    .iter()
                    .map(|option| (option.name.as_str(), option.effective_repr.clone())),
            );
            body.extend([String::new(), format!("Effective options: {effective}")]);
        }
    }
    if !config.rule_exceptions.is_empty()
        || !config.graph_edge_exceptions.is_empty()
        || !config.rule_ignores.is_empty()
        || !config.select_star_allow.is_empty()
    {
        body.extend([String::new(), "## Scoped Deviations".into(), String::new()]);
        for entry in &config.rule_exceptions {
            let scope: String = match (&entry.relation, &entry.cte) {
                (Some(relation), Some(cte)) => format!(" (relation `{relation}` in CTE `{cte}`)"),
                (Some(relation), None) => format!(" (relation `{relation}`)"),
                _ => String::new(),
            };
            body.push(format!(
                "- Exception `{}` at `{}`{scope}: {}",
                entry.rule, entry.path, entry.reason
            ));
        }
        for entry in &config.rule_ignores {
            let scopes: Vec<String> = entry
                .paths
                .iter()
                .map(|path| format!("path:{path}"))
                .chain(
                    entry
                        .selectors
                        .iter()
                        .map(|selector| format!("selector:{selector}")),
                )
                .collect();
            body.push(format!(
                "- Ignore `{}` at `{}`: {}",
                entry.rules.join(","),
                scopes.join(","),
                entry.reason
            ));
        }
        for entry in &config.graph_edge_exceptions {
            body.push(format!(
                "- `{GRAPH_EDGE_RULE_CODE}` exact edge `{}` -> `{}`: {}",
                entry.consumer, entry.dependency, entry.reason
            ));
        }
        for entry in &config.select_star_allow {
            body.push(format!(
                "- Lone-star allowance `{}`: {}",
                entry.paths.join(","),
                entry.reason
            ));
        }
    }
    let any_rule = |matches: &dyn Fn(&str) -> bool| rules.iter().any(|rule| matches(&rule.code));
    if any_rule(&|code| {
        MINIMUM_RULES_CODES.contains(&code) || LAYOUT_THRESHOLD_RULE_CODES.contains(&code)
    }) {
        body.extend([
            String::new(),
            "## Effective Thresholds".into(),
            String::new(),
        ]);
        body.extend(threshold_lines(config));
    }
    if any_rule(&|code| {
        LAYOUT_RULE_PREFIXES
            .iter()
            .any(|prefix| code.starts_with(prefix))
    }) {
        body.extend([String::new(), "## Owner Layout".into(), String::new()]);
        body.push(format!(
            "- Configured levels: `{}`",
            config.layout.levels.join(", ")
        ));
        body.push("- Every model owner is a leaf or a branch, never both.".into());
        body.push("- Declaration-role buckets organize files without changing visibility.".into());
    }
    if any_rule(&|code| code.starts_with(TEST_RULE_PREFIX)) {
        body.extend([String::new(), "## SQL Test Paths".into(), String::new()]);
        body.push("- Unit tests: `tests/unit/`".into());
        body.push("- Scenarios: `tests/scenarios/`".into());
        body.push(format!(
            "- Cross-domain pipelines: `tests/unit/{}/`",
            config.sql_tests.pipeline_directory
        ));
    }
    if !config.domains.is_empty()
        || !config.approved_source_tokens.is_empty()
        || !config.retired_source_tokens.is_empty()
    {
        body.extend([String::new(), "## Naming Vocabulary".into(), String::new()]);
        body.push(format!("- Domains: `{}`", config.domains.join(", ")));
        body.push(format!(
            "- Approved source tokens: `{}`",
            config.approved_source_tokens.join(", ")
        ));
        for (retired, replacement) in &config.retired_source_tokens {
            body.push(format!(
                "- Replace retired `{retired}` with `{replacement}`"
            ));
        }
    }
    format!("{}\n", body.join("\n").trim_end())
}

fn rule_example(rule: &RuleText) -> String {
    if let Some(example) = &rule.good_example {
        return example.clone();
    }
    RULE_EXAMPLES
        .iter()
        .find(|(code, _)| *code == rule.code)
        .map(|(_, example)| (*example).to_owned())
        .unwrap_or_else(|| {
            format!(
                "Apply `{}` at the SQL node named by the finding.",
                rule.slug
            )
        })
}
