//! A job group's `[dependencies]` table, read with positions for every error.

use std::collections::BTreeSet;

use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::orchestration_files::_helpers::reading::{error_at, toml_error};
use crate::orchestration_files::constants::{
    DEPENDENCIES_KEY, DEPENDENCY_FAILURE_KEY, DEPENDENCY_FAILURE_VALUES, WAIT_FOR_KEY,
    WAIT_FOR_VALUES,
};
use crate::orchestration_files::errors::FileError;
use crate::orchestration_files::models::{
    DependencyFailure, DependencySettings, GroupDependencies, WaitFor,
};

/// The group's `[dependencies]`, each override naming one of `members`; defaults when absent.
pub(crate) fn group_dependencies(
    text: &str,
    members: &BTreeSet<String>,
) -> Result<GroupDependencies, FileError> {
    let table = DeTable::parse(text).map_err(|error| toml_error(text, &error))?;
    let Some(written) = table.get_ref().get(DEPENDENCIES_KEY) else {
        return Ok(GroupDependencies::default());
    };
    let entries = as_table(text, written, DEPENDENCIES_KEY)?;
    let mut dependencies = GroupDependencies::default();
    for (key, value) in entries {
        let name: &str = key.get_ref();
        if [WAIT_FOR_KEY, DEPENDENCY_FAILURE_KEY].contains(&name) {
            setting(text, (name, value), &mut dependencies.defaults)?;
            continue;
        }
        if !members.contains(name) {
            return Err(error_at(
                text,
                key.span().start,
                format!(
                    "dependencies.{name} is neither `{WAIT_FOR_KEY}`, `{DEPENDENCY_FAILURE_KEY}` nor a job in `jobs`"
                ),
            ));
        }
        let mut overrides = DependencySettings::default();
        for (key, value) in as_table(text, value, &format!("{DEPENDENCIES_KEY}.{name}"))? {
            let setting_name: &str = key.get_ref();
            if ![WAIT_FOR_KEY, DEPENDENCY_FAILURE_KEY].contains(&setting_name) {
                return Err(error_at(
                    text,
                    key.span().start,
                    format!(
                        "unknown dependencies.{name} key '{setting_name}'; expected `{WAIT_FOR_KEY}` or `{DEPENDENCY_FAILURE_KEY}`"
                    ),
                ));
            }
            setting(text, (setting_name, value), &mut overrides)?;
        }
        dependencies.members.insert(name.to_owned(), overrides);
    }
    Ok(dependencies)
}

fn as_table<'a, 'i>(
    text: &str,
    value: &'a Spanned<DeValue<'i>>,
    what: &str,
) -> Result<&'a DeTable<'i>, FileError> {
    value
        .get_ref()
        .as_table()
        .ok_or_else(|| error_at(text, value.span().start, format!("{what} must be a table")))
}

/// One `wait_for` or `on_failure` value into `settings`.
fn setting(
    text: &str,
    (name, value): (&str, &Spanned<DeValue<'_>>),
    settings: &mut DependencySettings,
) -> Result<(), FileError> {
    let allowed: &[&str] = if name == WAIT_FOR_KEY {
        &WAIT_FOR_VALUES
    } else {
        &DEPENDENCY_FAILURE_VALUES
    };
    let written: Option<&str> = value.get_ref().as_str();
    let Some(chosen) = written.filter(|chosen| allowed.contains(chosen)) else {
        let quoted: Vec<String> = allowed.iter().map(|value| format!("\"{value}\"")).collect();
        return Err(error_at(
            text,
            value.span().start,
            format!("{name} must be one of {}", quoted.join(", ")),
        ));
    };
    match chosen {
        "node" => settings.wait_for = Some(WaitFor::Node),
        "job" => settings.wait_for = Some(WaitFor::Job),
        "skip" => settings.on_failure = Some(DependencyFailure::Skip),
        "run" => settings.on_failure = Some(DependencyFailure::Run),
        _ => settings.on_failure = Some(DependencyFailure::Partial),
    }
    Ok(())
}
