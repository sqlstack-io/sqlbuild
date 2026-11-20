//! Render job settings as options, check them against the build parser and parse without Python.

use sqlbuild_analysis::job_settings::main::setting_scope::setting_scope;
use sqlbuild_analysis::job_settings::models::SettingScope;
use std::collections::BTreeMap;

use serde_json::Value as Json;
use sqlbuild_core::text::main::python_str_repr::python_str_repr;

use crate::argv::_helpers::table::root_parser;
use crate::argv::constants::CONCURRENCY_DEST;
use crate::argv::models::{ActionKind, ActionSpec, Default, Nargs, ParserSpec, TypeFailure, Value};
use crate::argv::types::ArgvHost;

pub(crate) fn setting_argument(
    parser: &ParserSpec,
    name: &str,
    value: &Json,
) -> Result<Option<String>, String> {
    if setting_scope(name) == SettingScope::RunRequest {
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
    if action.nargs == Nargs::Attached {
        return attached_argument(name, value, option);
    }
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

/// `true` as the bare option, `false` as nothing, and a list of strings as `--option=a,b`.
fn attached_argument(name: &str, value: &Json, option: &str) -> Result<Option<String>, String> {
    match value {
        Json::Bool(true) => Ok(Some(option.to_owned())),
        Json::Bool(false) => Ok(None),
        Json::Array(items) if !items.is_empty() => {
            let texts: Vec<&str> = items
                .iter()
                .map(Json::as_str)
                .collect::<Option<_>>()
                .ok_or_else(|| format!("job setting {name} must be true or a list of strings"))?;
            Ok(Some(format!("{option}={}", texts.join(","))))
        }
        _ => Err(format!(
            "job setting {name} must be true or a list of strings"
        )),
    }
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

/// Converts `int`-like options so the parse validates them, and keeps every other value as text.
pub(crate) struct IntegerHost;

impl ArgvHost for IntegerHost {
    fn convert(&mut self, _converter: &str, text: &str) -> Result<Value, TypeFailure> {
        Ok(text.trim().parse::<i64>().map_or_else(
            |_| Value::Str(text.to_owned()),
            |number| Value::Int(number.to_string()),
        ))
    }

    fn repr(&self, text: &str) -> String {
        python_str_repr(text)
    }

    fn program_version(&mut self) -> String {
        String::new()
    }
}
