//! Parse a command with the CLI's table, keep only allowed flags and resolve selectors to exact node keys.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value as Json, json};
use sqlbuild_analysis::definition_changes::constants::PYTHON_NODE_KINDS;
use sqlbuild_analysis::graph::main::attach_jobs::attach_jobs;
use sqlbuild_analysis::graph::main::build_project_graph::build_project_graph;
use sqlbuild_analysis::graph::main::selectors_need_jobs::selectors_need_jobs;
use sqlbuild_analysis::graph::models::{GraphResource, JobSelection};
use sqlbuild_analysis::planner_selection::main::split_unit_test_selectors::split_unit_test_selectors;
use sqlbuild_analysis::planner_selection::models::{SelectorGroups, UnitTestSelectorNames};
use sqlbuild_analysis::python_selection::main::published_python_graph::published_python_graph;
use sqlbuild_analysis::python_selection::main::python_run_key::python_run_key;
use sqlbuild_analysis::python_selection::main::resolve_python_sql_run_selection::resolve_python_sql_run_selection;
use sqlbuild_analysis::python_selection::models::{PublishedPythonNode, SqlSelectionGraph};
use sqlbuild_core::text::main::python_str_repr::python_str_repr;
use sqlbuild_orchestration::runs::models::{CommandPlan, PlannerResource};
use sqlbuild_orchestration::runs::types::{CommandPlanner, PublishedJobs};

use crate::adhoc::constants::{
    ADHOC_VERBS, ALLOWED_DESTS, BUILD_VERB, CHANGED_COUNTS_DEST, CHANGED_DEST, COMMAND_DEST,
    FRESHNESS_VERB, INCLUDE_PYTHON_DEST, INTEGER_CONVERTERS, LOAD_SOURCES_DEST,
    POSITIVE_INTEGER_CONVERTER, REFUSED_DESTS, SEED_VERB, SELECTOR_DESTS, SQL_TEST_KIND,
    UNIT_TEST_VERBS,
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
        publication: (&[PlannerResource], &PublishedJobs),
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
            if dest == CHANGED_COUNTS_DEST {
                settings.insert(CHANGED_DEST.to_owned(), json!(texts(value)));
                continue;
            }
            if dest == CHANGED_DEST && settings.contains_key(CHANGED_DEST) {
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
                let seeds: BTreeSet<String> = resolve(publication, &select, &exclude)?
                    .into_iter()
                    .filter(|key| key.starts_with("seed:"))
                    .collect();
                if seeds.is_empty() {
                    return Err("the selectors match no seeds in the current publication".into());
                }
                seeds
            }
            FRESHNESS_VERB => freshness_sources(publication, &select, &exclude)?,
            _ => {
                let python = verb == BUILD_VERB
                    && settings.get(INCLUDE_PYTHON_DEST) != Some(&Json::Bool(false));
                resolve_with_tests((verb, python), publication, &select, &exclude)?
            }
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
    publication: (&[PlannerResource], &PublishedJobs),
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, String> {
    let selected: BTreeSet<String> = resolved(publication, select, exclude, false)?;
    if selected.is_empty() {
        return Err("the selectors match no nodes in the current publication".into());
    }
    Ok(selected)
}

/// [`resolve`] with `test:` and unit-test names selecting `sql_test:` keys, as `sqb test` does.
fn resolve_with_tests(
    (verb, python): (&str, bool),
    (resources, jobs): (&[PlannerResource], &PublishedJobs),
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, String> {
    let names = UnitTestSelectorNames {
        tests: resources
            .iter()
            .filter(|resource| is_sql_test(resource))
            .map(|resource| name_of(&resource.key).to_owned())
            .collect(),
        resources: resources
            .iter()
            .filter(|resource| !is_sql_test(resource))
            .map(|resource| name_of(&resource.key).to_owned())
            .chain(jobs.keys().cloned())
            .collect(),
    };
    let split = split_unit_test_selectors(
        SelectorGroups { select, exclude },
        &names,
        UNIT_TEST_VERBS.contains(&verb),
    )
    .map_err(|error| format!("{}: {}", error.code, error.message))?;
    if !split.excluded_test_names.is_empty() {
        return Err(
            "an ad-hoc run cannot exclude unit tests; select the unit tests to run instead".into(),
        );
    }
    let mut keys: BTreeSet<String> = if split.models_selected {
        resolved((resources, jobs), &split.select, &split.exclude, python)?
    } else {
        BTreeSet::new()
    };
    keys.extend(
        split
            .test_names
            .iter()
            .map(|name| format!("{SQL_TEST_KIND}:{name}")),
    );
    if keys.is_empty() {
        return Err("the selectors match no nodes in the current publication".into());
    }
    Ok(keys)
}

fn is_sql_test(resource: &PlannerResource) -> bool {
    resource.key.split_once(':').map(|(kind, _)| kind) == Some(SQL_TEST_KIND)
}

/// The published keys the selectors match; Python nodes only when the command runs them.
fn resolved(
    (resources, jobs): (&[PlannerResource], &PublishedJobs),
    select: &[String],
    exclude: &[String],
    include_python: bool,
) -> Result<BTreeSet<String>, String> {
    let (mut python, sql): (Vec<&PlannerResource>, Vec<&PlannerResource>) = resources
        .iter()
        .filter(|resource| !is_sql_test(resource))
        .partition(|resource| python_kind(&resource.key).is_some());
    if !include_python {
        python.clear();
    }
    let graph_resources: Vec<GraphResource> = sql
        .iter()
        .filter_map(|resource| graph_resource(resource))
        .collect();
    let mut graph = build_project_graph(&graph_resources);
    let selectors: Vec<String> = select.iter().chain(exclude).cloned().collect();
    let node_names: Vec<&str> = python
        .iter()
        .map(|resource| name_of(&resource.key))
        .collect();
    if selectors_need_jobs(&graph, &selectors, &node_names) {
        graph = attach_jobs(graph, published_jobs(jobs, include_python));
    }
    let sources: Vec<(String, Option<String>)> = sql
        .iter()
        .filter(|resource| resource.key.starts_with("source:"))
        .map(|resource| (name_of(&resource.key).to_owned(), resource.loader.clone()))
        .collect();
    let nodes: Vec<PublishedPythonNode> = python
        .iter()
        .filter_map(|resource| published_python_node(resource))
        .collect();
    let python_graph = published_python_graph(&nodes);
    let selection = resolve_python_sql_run_selection(
        SqlSelectionGraph {
            graph: &graph,
            sources: &sources,
        },
        &python_graph,
        select,
        exclude,
    )
    .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let kinds: BTreeMap<&str, &str> = nodes
        .iter()
        .map(|node| (node.name.as_str(), node.kind.as_str()))
        .collect();
    Ok(selection
        .sql_keys
        .into_iter()
        .map(|(kind, name)| format!("{kind}:{name}"))
        .chain(
            selection.python_node_names.iter().filter_map(|name| {
                Some(python_run_key(&sources, (kinds.get(name.as_str())?, name)))
            }),
        )
        .collect())
}

/// The Python node kind of a published key, such as `task` for `task:export_orders`.
fn python_kind(key: &str) -> Option<&'static str> {
    let (kind, _) = key.split_once(':')?;
    PYTHON_NODE_KINDS.into_iter().find(|python| *python == kind)
}

fn published_python_node(resource: &PlannerResource) -> Option<PublishedPythonNode> {
    Some(PublishedPythonNode {
        name: name_of(&resource.key).to_owned(),
        kind: python_kind(&resource.key)?.to_owned(),
        tags: resource.tags.clone(),
        folder: resource.folder.clone().unwrap_or_else(|| ".".to_owned()),
        deps: resource
            .deps
            .iter()
            .filter_map(|dep| object_key(dep))
            .collect(),
    })
}

fn name_of(key: &str) -> &str {
    key.split_once(':').map_or(key, |(_, name)| name)
}

/// Published jobs as already-resolved graph selections.
fn published_jobs(jobs: &PublishedJobs, include_python: bool) -> BTreeMap<String, JobSelection> {
    let mut selections: BTreeMap<String, JobSelection> = BTreeMap::new();
    for (name, keys) in jobs {
        let keys: Vec<(String, String)> = keys
            .iter()
            .filter(|key| include_python || python_kind(key).is_none())
            .filter_map(|key| object_key(key))
            .collect();
        selections.insert(name.clone(), JobSelection::Resolved { keys });
    }
    selections
}

/// Sources `sqb freshness` observes: selected or upstream sources, less those the exclusions reach.
fn freshness_sources(
    publication: (&[PlannerResource], &PublishedJobs),
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, String> {
    let selected: BTreeSet<String> =
        upstream_sources(publication.0, &resolved(publication, select, &[], false)?);
    let excluded: BTreeSet<String> = if exclude.is_empty() {
        BTreeSet::new()
    } else {
        upstream_sources(publication.0, &resolved(publication, exclude, &[], false)?)
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
