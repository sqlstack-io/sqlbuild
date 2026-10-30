//! Which parsed invocations run without Python, decided from the parsed values alone.

use std::path::PathBuf;

use crate::argv::constants::{JSON_OPTION, REVIEW_PROG};
use crate::argv::models::{ArgvOutcome, Namespace, Value};
use crate::dispatch::main::invocation_policy::invocation_policy;
use crate::dispatch::models::{CommandFacts, InvocationPolicy};
use crate::native_commands::constants::{
    CLEAN_COMMAND, INIT_COMMAND, RULES_COMMAND, RULES_LIST_ACTION, RULES_SHOW_ACTION,
    SKILLS_COMMAND,
};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::{
    CostOptions, NativeCommand, NativeInvocation, RulesAction, RulesOptions, SkillsOptions,
};
use sqlbuild_core::cost_command::constants::DESCENDING_ORDER;

/// Whether `sqb review --json` failed to parse, which Python reports as a JSON document.
pub(crate) fn review_json_failure(arguments: &[String], outcome: &ArgvOutcome) -> bool {
    arguments.iter().any(|argument| argument == JSON_OPTION)
        && outcome
            .usage_failure
            .as_ref()
            .is_some_and(|failure| failure.prog == REVIEW_PROG)
}

pub(crate) fn text(namespace: &Namespace, name: &str) -> Option<String> {
    match namespace.get(name) {
        Some(Value::Str(text) | Value::Path(text)) => Some(text.clone()),
        _ => None,
    }
}

pub(crate) fn flag(namespace: &Namespace, name: &str) -> bool {
    namespace.get(name) == Some(&Value::Bool(true))
}

/// The strings of a list attribute, or `None` when it holds anything else.
fn texts(namespace: &Namespace, name: &str) -> Option<Vec<String>> {
    match namespace.get(name) {
        None | Some(Value::None) => Some(Vec::new()),
        Some(Value::List(items)) => items
            .iter()
            .map(|item| match item {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        Some(_) => None,
    }
}

fn rules_command(namespace: &Namespace) -> Result<NativeCommand, PythonNeeded> {
    let action: RulesAction = match text(namespace, "rules_command").as_deref() {
        None | Some(RULES_LIST_ACTION) => RulesAction::List,
        Some(RULES_SHOW_ACTION) => RulesAction::Show,
        Some(_) => return Err(PythonNeeded::Command),
    };
    Ok(NativeCommand::Rules(RulesOptions {
        action,
        selector: text(namespace, "rules_rule_selector"),
        json: flag(namespace, "json"),
    }))
}

fn native_command(
    command: Option<&str>,
    namespace: &Namespace,
) -> Result<NativeCommand, PythonNeeded> {
    match command {
        None | Some(CLEAN_COMMAND) => Ok(NativeCommand::Nothing),
        Some(INIT_COMMAND) => Ok(NativeCommand::Init),
        Some(SKILLS_COMMAND) => Ok(NativeCommand::Skills(SkillsOptions {
            global_install: flag(namespace, "skills_global"),
            targets: texts(namespace, "skills_target").ok_or(PythonNeeded::Arguments)?,
            force: flag(namespace, "skills_force"),
        })),
        Some(RULES_COMMAND) => rules_command(namespace),
        Some("cost") => Ok(NativeCommand::Cost(CostOptions {
            selector: text(namespace, "cost_selector").unwrap_or_else(|| "latest".to_owned()),
            limit: match namespace.get("cost_limit") {
                None | Some(Value::None) => None,
                Some(Value::Int(value)) => {
                    Some(value.parse().map_err(|_| PythonNeeded::Arguments)?)
                }
                _ => return Err(PythonNeeded::Arguments),
            },
            no_limit: flag(namespace, "cost_no_limit"),
            sort: text(namespace, "cost_sort"),
            descending: text(namespace, "cost_order").is_none_or(|order| order == DESCENDING_ORDER),
            since: text(namespace, "cost_since"),
            until: text(namespace, "cost_until"),
            json: flag(namespace, "json"),
            json_path: text(namespace, "json_output").map(PathBuf::from),
            no_color: flag(namespace, "no_color"),
        })),
        Some(_) => Err(PythonNeeded::Command),
    }
}

/// The command `namespace` runs, when it runs without Python; `--debug` always uses Python.
pub(crate) fn native_invocation(namespace: &Namespace) -> Result<NativeInvocation, PythonNeeded> {
    if flag(namespace, "debug") {
        return Err(PythonNeeded::Command);
    }
    let command_name: Option<String> = text(namespace, "command");
    let command: NativeCommand = native_command(command_name.as_deref(), namespace)?;
    let policy: InvocationPolicy = invocation_policy(&CommandFacts {
        command: command_name.clone(),
        ..CommandFacts::default()
    });
    Ok(NativeInvocation {
        command,
        command_name,
        project_dir: text(namespace, "project_dir").map(PathBuf::from),
        target: text(namespace, "target"),
        creates_project: policy.creates_project,
        maintains_skills: policy.maintains_skills,
    })
}
