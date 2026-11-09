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
    ADHOC_VERBS, ALLOWED_DESTS, BUILD_VERB, COMMAND_DEST, FRESHNESS_VERB, INTEGER_CONVERTERS,
    LOAD_SOURCES_DEST, POSITIVE_INTEGER_CONVERTER, REFUSED_DESTS, SEED_VERB, SELECTOR_DESTS,
};
use crate::argv::main::command_settings_arguments::command_settings_arguments;
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
            if bare.value(dest) == *value || dest == COMMAND_DEST {
                continue;
            }
            if let Some(selector) = SELECTOR_DESTS.iter().find(|name| **name == dest) {
                selectors.insert(selector, texts(value));
                continue;
            }
            settings.insert(dest.clone(), setting(dest, value)?);
        }
        if verb == BUILD_VERB {
            job_settings_arguments(&settings)?;
        } else {
            command_settings_arguments(verb, &settings)?;
        }
        let mut select: Vec<String> = selectors.remove("select").unwrap_or_default();
        select.extend(selectors.remove("positional_select").unwrap_or_default());
        let exclude: Vec<String> = selectors.remove("exclude").unwrap_or_default();
        let selection: BTreeSet<String> = match verb.as_str() {
            SEED_VERB => {
                let seeds: BTreeSet<String> = resolve(resources, &select, &exclude)?
                    .into_iter()
                    .filter(|key| key.starts_with("seed:"))
                    .collect();
                if seeds.is_empty() {
                    return Err("the selectors match no seeds in the current publication".into());
                }
                seeds
            }
            FRESHNESS_VERB => freshness_sources(resources, &select, &exclude)?,
            _ => resolve(resources, &select, &exclude)?,
        };
        Ok(CommandPlan {
            verb: verb.clone(),
            selection,
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
    if dest == LOAD_SOURCES_DEST && *value == Value::Bool(true) {
        return Err(
            "--load is not available for ad-hoc runs: source loads are not part of ad-hoc runs yet"
                .into(),
        );
    }
    if !ALLOWED_DESTS.contains(&dest) {
        return Err(format!(
            "{} is not available for ad-hoc runs",
            flag_of(dest)
        ));
    }
    match value {
        Value::Bool(flag) => Ok(json!(flag)),
        Value::Int(text) => text
            .parse::<i64>()
            .map(|number| json!(number))
            .map_err(|_| format!("{} must be a whole number", flag_of(dest))),
        Value::Str(text) => Ok(json!(text)),
        _ => Err(format!(
            "{} is not available for ad-hoc runs",
            flag_of(dest)
        )),
    }
}

/// The flag people type for a parser destination that has its own `--name` flag.
fn flag_of(dest: &str) -> String {
    format!("--{}", dest.replace('_', "-"))
}

/// Exact `kind:name` keys, as `sqb build` would select them from this publication.
fn resolve(
    resources: &[PlannerResource],
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, String> {
    let selected: BTreeSet<String> = resolved(resources, select, exclude)?;
    if selected.is_empty() {
        return Err("the selectors match no nodes in the current publication".into());
    }
    Ok(selected)
}

fn resolved(
    resources: &[PlannerResource],
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, String> {
    let graph_resources: Vec<GraphResource> = resources.iter().filter_map(graph_resource).collect();
    let graph = build_project_graph(&graph_resources);
    Ok(resolve_selectors(&graph, select, exclude)
        .map_err(|error| format!("{}: {}", error.code, error.message))?
        .into_iter()
        .map(|(kind, name)| format!("{kind}:{name}"))
        .collect())
}

/// Sources `sqb freshness` observes: selected or upstream sources, less those the exclusions reach.
fn freshness_sources(
    resources: &[PlannerResource],
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, String> {
    let selected: BTreeSet<String> =
        upstream_sources(resources, &resolved(resources, select, &[])?);
    let excluded: BTreeSet<String> = if exclude.is_empty() {
        BTreeSet::new()
    } else {
        upstream_sources(resources, &resolved(resources, exclude, &[])?)
    };
    let sources: BTreeSet<String> = selected.difference(&excluded).cloned().collect();
    if sources.is_empty() {
        return Err("no sources are selected or upstream of the selection".into());
    }
    Ok(sources)
}

fn upstream_sources(resources: &[PlannerResource], keys: &BTreeSet<String>) -> BTreeSet<String> {
    let deps: BTreeMap<&str, &[String]> = resources
        .iter()
        .map(|resource| (resource.key.as_str(), resource.deps.as_slice()))
        .collect();
    let mut reached: BTreeSet<&str> = BTreeSet::new();
    let mut pending: Vec<&str> = keys.iter().map(String::as_str).collect();
    while let Some(key) = pending.pop() {
        if reached.insert(key) {
            pending.extend(
                deps.get(key)
                    .into_iter()
                    .flat_map(|deps| deps.iter().map(String::as_str)),
            );
        }
    }
    reached
        .into_iter()
        .filter(|key| key.starts_with("source:"))
        .map(str::to_owned)
        .collect()
}

/// A published node as the selector graph sees it; keys without a kind are not selectable.
fn graph_resource(resource: &PlannerResource) -> Option<GraphResource> {
    Some(GraphResource {
        key: object_key(&resource.key)?,
        deps: resource
            .deps
            .iter()
            .map(String::as_str)
            .filter_map(object_key)
            .collect(),
        tags: resource.tags.clone(),
        folder: resource.folder.clone(),
    })
}

fn object_key(key: &str) -> Option<(String, String)> {
    key.split_once(':')
        .map(|(kind, name)| (kind.to_owned(), name.to_owned()))
}
