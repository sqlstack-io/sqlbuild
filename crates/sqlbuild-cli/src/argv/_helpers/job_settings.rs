//! Render one authored setting and check emitted options against the build parser.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::argv::_helpers::table::root_parser;
use crate::argv::constants::CONCURRENCY_DEST;
use crate::argv::models::{ActionKind, ActionSpec, Default, ParserSpec, Value};

pub(crate) fn setting_argument(
    parser: &ParserSpec,
    name: &str,
    value: &Json,
) -> Result<Option<String>, String> {
    if matches!(
        name,
        "help"
            | "target"
            | "project_dir"
            | "select"
            | "exclude"
            | "select_file"
            | "positional_select"
            | "json"
            | "json_output"
            | "event_output"
            | "manifest"
            | "dbt_project_dir"
            | "dbt_profiles_dir"
            | "dbt_target"
            | "dbt_target_path"
    ) {
        return Err(format!("job setting {name} is owned by the run request"));
    }
    let actions: Vec<&ActionSpec> = parser
        .actions
        .iter()
        .filter(|action| action.dest.as_deref() == Some(name))
        .collect();
    let Some(action) = actions.first() else {
        return Err(format!("unknown build setting {name}"));
    };
    if matches!(action.kind, ActionKind::StoreTrue | ActionKind::StoreFalse) {
        return boolean_argument(name, value, &actions);
    }
    let option = &action.option_strings[0];
    let text = setting_text(name, value, action)?;
    if action
        .choices
        .as_ref()
        .is_some_and(|choices| !choices.contains(&text))
    {
        return Err(format!("job setting {name} is not an accepted choice"));
    }
    Ok(Some(format!("{option}={text}")))
}

fn boolean_argument(
    name: &str,
    value: &Json,
    actions: &[&ActionSpec],
) -> Result<Option<String>, String> {
    let action = actions[0];
    let flag = value
        .as_bool()
        .ok_or_else(|| format!("job setting {name} must be boolean"))?;
    if let Some(chosen) = actions
        .iter()
        .find(|action| action.constant == Value::Bool(flag))
    {
        return Ok(Some(chosen.option_strings[0].clone()));
    }
    if action.default != Default::Value(Value::Bool(flag))
        && !(action.default == Default::Suppress
            && root_parser().actions.iter().any(|global| {
                global.dest.as_deref() == Some(name)
                    && global.default == Default::Value(Value::Bool(flag))
            }))
    {
        return Err(format!("job setting {name} cannot be set to {flag}"));
    }
    Ok(None)
}

fn setting_text(name: &str, value: &Json, action: &ActionSpec) -> Result<String, String> {
    match action.converter.as_deref() {
        Some("int" | "_positive_integer") => {
            let integer = value
                .as_i64()
                .ok_or_else(|| format!("job setting {name} must be an integer"))?;
            if integer < 1
                && (name == CONCURRENCY_DEST
                    || action.converter.as_deref() == Some("_positive_integer"))
            {
                return Err(format!("job setting {name} must be positive"));
            }
            Ok(integer.to_string())
        }
        Some("parse_cli_vars") => {
            if !value.is_object() {
                return Err(format!("job setting {name} must be a JSON object"));
            }
            Ok(value.to_string())
        }
        _ => value
            .as_str()
            .ok_or_else(|| format!("job setting {name} must be text"))
            .map(str::to_owned),
    }
}

pub(crate) fn validate_mutex(parser: &ParserSpec, arguments: &[String]) -> Result<(), String> {
    let mut mutex_groups: BTreeMap<usize, &str> = BTreeMap::new();
    for argument in arguments {
        let option = argument
            .split_once('=')
            .map_or(argument.as_str(), |(option, _)| option);
        if let Some(group) = parser.actions[parser.option_index[option]].mutex_group
            && let Some(previous) = mutex_groups.insert(group, option)
        {
            return Err(format!(
                "job settings {previous} and {option} are mutually exclusive"
            ));
        }
    }
    Ok(())
}
