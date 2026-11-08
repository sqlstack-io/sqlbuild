//! Parse a command with the CLI's table, keep only allowed flags and resolve selectors to exact node keys.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value as Json, json};
use sqlbuild_analysis::graph::main::build_project_graph::build_project_graph;
use sqlbuild_analysis::graph::main::resolve_selectors::resolve_selectors;
use sqlbuild_analysis::graph::models::GraphResource;
use sqlbuild_core::text::main::python_str_repr::python_str_repr;
use sqlbuild_orchestration::runs::models::{CommandPlan, PlannerResource};
use sqlbuild_orchestration::runs::types::CommandPlanner;

use crate::adhoc::constants::{
    ADHOC_VERBS, ALLOWED_DESTS, INTEGER_CONVERTERS, POSITIVE_INTEGER_CONVERTER, REFUSED_DESTS,
    SELECTOR_DESTS,
};
use crate::argv::main::job_settings_arguments::job_settings_arguments;
use crate::argv::main::parse_cli_arguments::parse_cli_arguments;
use crate::argv::models::{ArgvSettings, Namespace, TypeFailure, Value};
use crate::argv::types::ArgvHost;

/// The planner `sqb server` is built with.
pub(crate) struct CliPlanner;

impl CommandPlanner for CliPlanner {
    fn plan(
        &self,
        command: &[String],
        resources: &[PlannerResource],
    ) -> Result<CommandPlan, String> {
        let verb = command
            .first()
            .ok_or_else(|| "enter a command, such as build --select orders".to_owned())?;
        if !ADHOC_VERBS.contains(&verb.as_str()) {
            return Err(format!(
                "sqb {verb} is not available as an ad-hoc run; use build, test, seed, audit or freshness"
            ));
        }
        let parsed = parse(command)?;
        let bare = parse(std::slice::from_ref(verb))?;
        let mut selectors: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mut settings: BTreeMap<String, Json> = BTreeMap::new();
        for (dest, value) in &parsed.entries {
            if bare.value(dest) == *value || dest == "command" {
                continue;
            }
            if let Some(selector) = SELECTOR_DESTS.iter().find(|name| **name == dest) {
                selectors.insert(selector, texts(value));
                continue;
            }
            settings.insert(dest.clone(), setting(dest, value)?);
        }
        if verb == "build" {
            job_settings_arguments(&settings)?;
        }
        let mut select: Vec<String> = selectors.remove("select").unwrap_or_default();
        select.extend(selectors.remove("positional_select").unwrap_or_default());
        let exclude: Vec<String> = selectors.remove("exclude").unwrap_or_default();
        Ok(CommandPlan {
            verb: verb.clone(),
            selection: resolve(resources, &select, &exclude)?,
            settings,
        })
    }
}

/// Whole numbers convert as the parser needs them; paths and variables stay text so the refusal can name them.
struct PlannerHost;

impl ArgvHost for PlannerHost {
    fn convert(&mut self, converter: &str, text: &str) -> Result<Value, TypeFailure> {
        if !INTEGER_CONVERTERS.contains(&converter) {
            return Ok(Value::Str(text.to_owned()));
        }
        let number: i64 = text.trim().parse().map_err(|_| TypeFailure::Invalid)?;
        if converter == POSITIVE_INTEGER_CONVERTER && number < 1 {
            return Err(TypeFailure::ArgumentType(
                "must be a positive integer".to_owned(),
            ));
        }
        Ok(Value::Int(number.to_string()))
    }

    fn repr(&self, text: &str) -> String {
        python_str_repr(text)
    }

    fn program_version(&mut self) -> String {
        String::new()
    }
}

fn parse(command: &[String]) -> Result<Namespace, String> {
    let settings = ArgvSettings {
        columns: 100,
        use_color: false,
    };
    let outcome = parse_cli_arguments(command, Vec::new(), settings, &mut PlannerHost);
    outcome.parsed.map_err(|_| {
        let written: String = outcome.writes.into_iter().map(|(_, text)| text).collect();
        written
            .lines()
            .rev()
            .find(|line| line.contains("error"))
            .map_or_else(
                || "help is not a run; enter a command to check it".to_owned(),
                |line| line.trim().to_owned(),
            )
    })
}

fn texts(value: &Value) -> Vec<String> {
    match value {
        Value::List(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// One allowed flag as a frozen setting, or why the flag is refused.
fn setting(dest: &str, value: &Value) -> Result<Json, String> {
    if let Some((_, flag, reason)) = REFUSED_DESTS.iter().find(|(name, _, _)| *name == dest) {
        return Err(format!("{flag} is not available for ad-hoc runs: {reason}"));
    }
    if dest == "load_sources" && *value == Value::Bool(true) {
        return Err(
            "--load is not available for ad-hoc runs: source loads are not part of ad-hoc runs yet"
                .into(),
        );
    }
    if !ALLOWED_DESTS.contains(&dest) {
        return Err(format!("{dest} is not available for ad-hoc runs"));
    }
    match value {
        Value::Bool(flag) => Ok(json!(flag)),
        Value::Int(text) => text
            .parse::<i64>()
            .map(|number| json!(number))
            .map_err(|_| format!("{dest} must be a whole number")),
        Value::Str(text) => Ok(json!(text)),
        _ => Err(format!("{dest} is not available for ad-hoc runs")),
    }
}

/// Exact `kind:name` keys, as `sqb build` would select them from this publication.
fn resolve(
    resources: &[PlannerResource],
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, String> {
    let graph_resources: Vec<GraphResource> = resources
        .iter()
        .filter_map(|resource| {
            Some(GraphResource {
                key: object_key(&resource.key)?,
                deps: resource
                    .deps
                    .iter()
                    .filter_map(|dep| object_key(dep))
                    .collect(),
                tags: resource.tags.clone(),
                folder: resource.folder.clone(),
            })
        })
        .collect();
    let graph = build_project_graph(&graph_resources);
    let selected: BTreeSet<String> = resolve_selectors(&graph, select, exclude)
        .map_err(|error| format!("{}: {}", error.code, error.message))?
        .into_iter()
        .map(|(kind, name)| format!("{kind}:{name}"))
        .collect();
    if selected.is_empty() {
        return Err("the selectors match no nodes in the current publication".into());
    }
    Ok(selected)
}

fn object_key(key: &str) -> Option<(String, String)> {
    key.split_once(':')
        .map(|(kind, name)| (kind.to_owned(), name.to_owned()))
}
