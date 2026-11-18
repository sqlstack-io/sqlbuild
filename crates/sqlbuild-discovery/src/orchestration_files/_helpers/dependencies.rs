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
            dependencies.defaults = setting(text, (name, value), dependencies.defaults)?;
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
            overrides = setting(text, (setting_name, value), overrides)?;
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

/// `settings` with one `wait_for` or `on_failure` value set.
fn setting(
    text: &str,
    (name, value): (&str, &Spanned<DeValue<'_>>),
    settings: DependencySettings,
) -> Result<DependencySettings, FileError> {
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
    let failure = |on_failure: DependencyFailure| DependencySettings {
        on_failure: Some(on_failure),
        ..settings
    };
    match chosen {
        "node" => Err(error_at(
            text,
            value.span().start,
            "wait_for = \"node\" is not supported yet; it is planned, so use \"job\" for now"
                .to_owned(),
        )),
        "job" => Ok(DependencySettings {
            wait_for: Some(WaitFor::Job),
            ..settings
        }),
        "skip" => Ok(failure(DependencyFailure::Skip)),
        "run" => Ok(failure(DependencyFailure::Run)),
        _ => Ok(failure(DependencyFailure::Partial)),
    }
}
