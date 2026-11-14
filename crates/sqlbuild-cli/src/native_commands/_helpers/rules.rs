//! `sqb rules list|show` over the built-in catalogue, when no custom rule or option needs Python.

use std::collections::BTreeMap;
use std::path::Path;

use sqlbuild_rules::configuration::main::load_rules_config::load_rules_config;
use sqlbuild_rules::models::{RuleMetadata, RulesConfig};
use sqlbuild_rules::rule_catalogue::main::format_rule_text::format_rule_text;
use sqlbuild_rules::rule_catalogue::main::render_rule_list::render_rule_list;
use sqlbuild_rules::rule_catalogue::main::sorted_catalogue_order::sorted_catalogue_order;
use sqlbuild_rules::rule_catalogue::main::string_tuple_repr::string_tuple_repr;
use sqlbuild_rules::rule_catalogue::models::{
    CatalogueRule, OptionKind, OptionSpec, OptionText, OptionValue, RuleListing, RuleText,
};
use sqlbuild_rules::rules::main::builtin_options::builtin_options;
use sqlbuild_rules::rules::main::catalogue::catalogue;
use sqlbuild_rules::rules::models::{BuiltinOption, BuiltinOptionKind};

use crate::native_commands::_helpers::project_facts::has_custom_rules;
use crate::native_commands::constants::{
    DICT_TYPE_NAME, EMPTY_TUPLE_REPR, FALSE_REPR, FAMILY_SUFFIX_LENGTH, MODEL_SUBJECT,
    PROJECT_SUBJECT, RULE_SELECTOR_REQUIRED, RULES_USAGE_EXIT_CODE, UNKNOWN_RULE_PREFIX,
};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::{CommandOutput, RulesAction, RulesOptions};

/// `RuleOption.boolean(...)`, `.string_list(...)` or a table list, as the catalogue validates it.
fn option_spec(option: &BuiltinOption) -> OptionSpec {
    let (value_type, default, item_type) = match option.kind {
        BuiltinOptionKind::Boolean => (OptionKind::Bool, OptionValue::Bool(false), None),
        BuiltinOptionKind::StringList(defaults) => (
            OptionKind::Tuple,
            OptionValue::Tuple(
                defaults
                    .iter()
                    .map(|item| OptionValue::Str((*item).to_owned()))
                    .collect(),
            ),
            Some(OptionKind::Str),
        ),
        BuiltinOptionKind::TableList => (
            OptionKind::Tuple,
            OptionValue::Tuple(Vec::new()),
            Some(OptionKind::Other(DICT_TYPE_NAME.to_owned())),
        ),
        BuiltinOptionKind::Integer { default, .. } => {
            (OptionKind::Int, OptionValue::Int(i64::from(default)), None)
        }
    };
    let minimum: Option<i64> = match option.kind {
        BuiltinOptionKind::Integer { minimum, .. } => Some(i64::from(minimum)),
        _ => None,
    };
    OptionSpec {
        name: option.name.to_owned(),
        description: option.description.to_owned(),
        value_type,
        default,
        choices: Vec::new(),
        minimum,
        maximum: None,
        minimum_items: None,
        item_type,
    }
}

/// The option as `rules show` prints it: the `repr` of its default, which is also effective.
fn option_text(option: &BuiltinOption) -> OptionText {
    let default_repr: String = match option.kind {
        BuiltinOptionKind::Boolean => FALSE_REPR.to_owned(),
        BuiltinOptionKind::StringList(defaults) => string_tuple_repr(defaults),
        BuiltinOptionKind::TableList => EMPTY_TUPLE_REPR.to_owned(),
        BuiltinOptionKind::Integer { default, .. } => default.to_string(),
    };
    OptionText {
        name: option.name.to_owned(),
        description: option.description.to_owned(),
        effective_repr: default_repr.clone(),
        default_repr,
    }
}

/// The rule code without its three-digit number, as the built-in `Rule` records it.
fn family(rule: &RuleMetadata) -> String {
    rule.code[..rule.code.len().saturating_sub(FAMILY_SUFFIX_LENGTH)].to_owned()
}

fn listing(rule: &RuleMetadata) -> RuleListing {
    RuleListing {
        code: rule.code.clone(),
        family: family(rule),
        message: rule.message.clone(),
        custom: false,
        subject: Some(
            if rule.project_wide {
                PROJECT_SUBJECT
            } else {
                MODEL_SUBJECT
            }
            .to_owned(),
        ),
    }
}

fn rule_text(rule: &RuleMetadata) -> RuleText {
    RuleText {
        code: rule.code.clone(),
        family: family(rule),
        slug: rule.slug.clone(),
        message: rule.message.clone(),
        remediation: rule.remediation.clone(),
        enabled_by_default: rule.enabled_by_default,
        custom: false,
        source: None,
        options: builtin_options(&rule.code)
            .iter()
            .map(option_text)
            .collect(),
        good_example: rule
            .guidance
            .as_ref()
            .map(|guidance| guidance.good_example.clone()),
    }
}

/// The built-in catalogue in the order `build_rule_catalogue` sorts it.
fn sorted_catalogue() -> Result<Vec<RuleMetadata>, PythonNeeded> {
    let rules: Vec<RuleMetadata> = catalogue();
    let views: Vec<CatalogueRule> = rules
        .iter()
        .map(|rule| CatalogueRule {
            code: rule.code.clone(),
            options: builtin_options(&rule.code)
                .iter()
                .map(option_spec)
                .collect(),
        })
        .collect();
    let order: Vec<usize> =
        sorted_catalogue_order(&views, &BTreeMap::new()).map_err(|_| PythonNeeded::Failure)?;
    Ok(order
        .into_iter()
        .filter_map(|index| rules.get(index).cloned())
        .collect())
}

fn printed(stdout: String, exit_code: i32) -> CommandOutput {
    CommandOutput {
        stdout: format!("{stdout}\n"),
        stderr: String::new(),
        exit_code,
    }
}

fn show(selector: Option<&str>, rules: &[RuleMetadata], config: &RulesConfig) -> CommandOutput {
    let Some(selector) = selector else {
        return printed(RULE_SELECTOR_REQUIRED.to_owned(), RULES_USAGE_EXIT_CODE);
    };
    match rules.iter().find(|rule| rule.code == selector) {
        Some(rule) => printed(format_rule_text(&rule_text(rule), Some(config)), 0),
        None => printed(
            format!("{UNKNOWN_RULE_PREFIX}{selector}"),
            RULES_USAGE_EXIT_CODE,
        ),
    }
}

/// What `rules list|show` prints for `project_dir`, unless it needs Python.
pub(crate) fn run_rules(
    project_dir: &Path,
    options: &RulesOptions,
) -> Result<CommandOutput, PythonNeeded> {
    if has_custom_rules(project_dir) {
        return Err(PythonNeeded::Project);
    }
    let config: RulesConfig = load_rules_config(project_dir).map_err(|_| PythonNeeded::Failure)?;
    if !config.rule_options.is_empty() {
        return Err(PythonNeeded::Project);
    }
    let rules: Vec<RuleMetadata> = sorted_catalogue()?;
    Ok(match options.action {
        RulesAction::List => {
            let listings: Vec<RuleListing> = rules.iter().map(listing).collect();
            printed(render_rule_list(&listings, options.json), 0)
        }
        RulesAction::Show => show(options.selector.as_deref(), &rules, &config),
    })
}
