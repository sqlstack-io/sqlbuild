//! Parse a command with the CLI's table, keep only allowed flags and resolve selectors to exact node keys.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{Value as Json, json};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::constants::AUDIT_KIND;
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::graph::main::run_selector_ids::run_selector_ids;
use sqlbuild_analysis::graph::main::selectors_need_jobs::selectors_need_jobs;
use sqlbuild_analysis::planner_selection::main::split_unit_test_selectors::split_unit_test_selectors;
use sqlbuild_analysis::planner_selection::models::SelectorGroups;
use sqlbuild_analysis::python_selection::main::python_run_key::python_run_key;
use sqlbuild_analysis::python_selection::main::resolve_python_sql_run_selection::resolve_python_sql_run_selection;
use sqlbuild_analysis::python_selection::models::SqlSelectionGraph;
use sqlbuild_core::text::main::python_str_repr::python_str_repr;
use sqlbuild_orchestration::runs::models::{
    CommandPlan, PinnedJob, PlannerResource, PublicationCache, PublishedJobs, SelectionProblem,
    SelectionVocabulary,
};
use sqlbuild_orchestration::runs::types::PastRuns;
use sqlbuild_orchestration::runs::types::{CommandPlanner, PreparedPublication, Preparer};

use crate::adhoc::_helpers::evaluation::{evaluate, vocabulary};
use crate::adhoc::_helpers::job_pins::{named_job_settings, pinned_jobs};
use crate::adhoc::_helpers::past_runs::{with_audits_and_runs, with_past_runs};
use crate::adhoc::_helpers::publication::{IndexedPublication, python_kind};
use crate::adhoc::models::Refusal;

use crate::adhoc::constants::{
    ADHOC_VERBS, ALLOWED_DESTS, BUILD_VERB, CHANGED_COUNTS_DEST, CHANGED_DEST, COMMAND_DEST,
    FRESHNESS_VERB, INCLUDE_PYTHON_DEST, INTEGER_CONVERTERS, LOAD_BUILD_SETTINGS,
    LOAD_SOURCES_DEST, LOAD_VERB, POSITIVE_INTEGER_CONVERTER, REFUSED_DESTS, RELOAD_DEST,
    SEED_VERB, SELECTOR_DESTS, SQL_TEST_KIND, UNIT_TEST_VERBS, VARS_CONVERTER, VARS_DEST,
};
use crate::argv::main::command_settings_arguments::command_settings_arguments;
use crate::argv::main::job_settings_arguments::job_settings_arguments;
use crate::argv::main::parse_cli_arguments::parse_cli_arguments;
use crate::argv::models::{ArgvSettings, Namespace, TypeFailure, Value};
use crate::argv::types::ArgvHost;

/// The planner `sqb server` is built with; it keeps the publications it indexed.
pub(crate) struct CliPlanner {
    publications: PublicationCache,
}

impl CliPlanner {
    pub(crate) const fn new() -> Self {
        Self {
            publications: PublicationCache::new(),
        }
    }
}

impl CommandPlanner for CliPlanner {
    fn preparer(&self) -> Preparer {
        |resources: Vec<PlannerResource>, jobs: PublishedJobs| -> Arc<dyn PreparedPublication> {
            Arc::new(IndexedPublication::new(resources, jobs))
        }
    }

    fn publications(&self) -> &PublicationCache {
        &self.publications
    }
}

impl PreparedPublication for IndexedPublication {
    fn run_selector_ids(&self, texts: &[String]) -> BTreeSet<String> {
        run_selector_ids(texts)
    }

    fn plan(&self, command: &[String], past: &PastRuns) -> Result<CommandPlan, String> {
        with_past_runs(past, || self.planned(command))
    }

    fn evaluate(
        &self,
        (select, exclude): (&str, &str),
        past: &PastRuns,
    ) -> Result<Vec<String>, Box<SelectionProblem>> {
        with_past_runs(past, || evaluate(self, (select, exclude)))
    }

    fn vocabulary(&self) -> SelectionVocabulary {
        self.vocabulary.get_or_init(|| vocabulary(self)).clone()
    }
}

impl IndexedPublication {
    /// [`PreparedPublication::plan`] with the past runs in place.
    fn planned(&self, command: &[String]) -> Result<CommandPlan, String> {
        let (verb, mut selectors, settings) = command_parts(command)?;
        let mut select: Vec<String> = selectors.remove("select").unwrap_or_default();
        select.extend(selectors.remove("positional_select").unwrap_or_default());
        let exclude: Vec<String> = selectors.remove("exclude").unwrap_or_default();
        let (named, job_settings) = if verb == BUILD_VERB {
            named_job_settings(self, &select, &settings)?
        } else {
            (Vec::new(), BTreeMap::new())
        };
        let selection: BTreeSet<String> = match verb.as_str() {
            SEED_VERB => {
                let seeds: BTreeSet<String> = resolve(self, &select, &exclude)?
                    .into_iter()
                    .filter(|key| key.starts_with("seed:"))
                    .collect();
                if seeds.is_empty() {
                    return Err("the selectors match no seeds in the current publication".into());
                }
                seeds
            }
            FRESHNESS_VERB => freshness_sources(self, &select, &exclude)?,
            LOAD_VERB => managed_sources(self, &select, &exclude)?,
            _ => resolve_with_tests(
                (&verb, runs_python(&verb, (&settings, &job_settings))),
                self,
                &select,
                &exclude,
            )?,
        };
        let jobs: BTreeMap<String, PinnedJob> = if named.is_empty() {
            BTreeMap::new()
        } else {
            let python = runs_python(&verb, (&settings, &job_settings));
            pinned_jobs(self, (&select, &exclude), (&settings, python), &selection)?
        };
        Ok(CommandPlan {
            verb,
            selection,
            settings,
            jobs,
            job_settings,
        })
    }
}

/// Whether a command runs Python nodes: a build does unless its flags or named jobs turn them off.
fn runs_python(
    verb: &str,
    (settings, job_settings): (&BTreeMap<String, Json>, &BTreeMap<String, Json>),
) -> bool {
    verb == BUILD_VERB
        && settings
            .get(INCLUDE_PYTHON_DEST)
            .or_else(|| job_settings.get(INCLUDE_PYTHON_DEST))
            != Some(&Json::Bool(false))
}

/// A command's verb, selectors by destination and validated settings.
type CommandParts = (
    String,
    BTreeMap<&'static str, Vec<String>>,
    BTreeMap<String, Json>,
);

/// Parse `command` and keep only allowed flags, as an ad-hoc run or plan freezes it before resolving selectors.
pub(crate) fn command_parts(command: &[String]) -> Result<CommandParts, String> {
    let verb = command
        .first()
        .ok_or_else(|| "enter a command, such as build --select orders".to_owned())?;
    if !ADHOC_VERBS.contains(&verb.as_str()) {
        return Err(format!(
            "sqb {verb} is not available as an ad-hoc run; use build, test, seed, audit, freshness or load"
        ));
    }
    let parsed = parse(command)?;
    let bare = parse(std::slice::from_ref(verb))?;
    let mut selectors: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut settings: BTreeMap<String, Json> = BTreeMap::new();
    for (dest, value) in &parsed.entries {
        if bare.value(dest) == *value || dest == COMMAND_DEST {
            continue;
        }
        if let Some(selector) = SELECTOR_DESTS.iter().find(|name| **name == dest) {
            selectors.insert(*selector, texts(value));
            continue;
        }
        if dest == CHANGED_COUNTS_DEST {
            settings.insert(CHANGED_DEST.to_owned(), json!(texts(value)));
            continue;
        }
        if dest == CHANGED_DEST && settings.contains_key(CHANGED_DEST) {
            continue;
        }
        let setting = setting(dest, value)?;
        if dest == VARS_DEST && setting.as_object().is_some_and(serde_json::Map::is_empty) {
            continue;
        }
        settings.insert(dest.clone(), setting);
    }
    if verb == LOAD_VERB {
        for (dest, value) in LOAD_BUILD_SETTINGS {
            settings.insert(dest.to_owned(), json!(value));
        }
        if settings.get(RELOAD_DEST) == Some(&Json::Bool(true)) {
            settings.remove(LOAD_SOURCES_DEST);
        }
    }
    if verb == BUILD_VERB || verb == LOAD_VERB {
        job_settings_arguments(&settings)?;
    } else {
        command_settings_arguments(verb, &settings)?;
    }
    Ok((verb.clone(), selectors, settings))
}

/// Whole numbers convert as the parser needs them; `--vars` must be a JSON object; paths stay text so the refusal can name them.
struct PlannerHost;

impl ArgvHost for PlannerHost {
    fn convert(&mut self, converter: &str, text: &str) -> Result<Value, TypeFailure> {
        if converter == VARS_CONVERTER {
            return match serde_json::from_str::<Json>(text) {
                Ok(Json::Object(_)) => Ok(Value::Str(text.to_owned())),
                Ok(_) => Err(TypeFailure::ArgumentType(
                    "--vars must be a JSON object".to_owned(),
                )),
                Err(error) => Err(TypeFailure::ArgumentType(format!(
                    "--vars must be valid JSON: {error}"
                ))),
            };
        }
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
        Value::Str(text) if dest == VARS_DEST => serde_json::from_str(text)
            .map_err(|error| format!("--vars must be valid JSON: {error}")),
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
pub(crate) fn resolve(
    publication: &IndexedPublication,
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, Refusal> {
    let selected: BTreeSet<String> = resolved(publication, select, exclude, false)?;
    if selected.is_empty() {
        return Err("the selectors match no nodes in the current publication".into());
    }
    Ok(selected)
}

/// [`resolve`] with `test:` and unit-test names selecting `sql_test:` keys, as `sqb test` does.
pub(crate) fn resolve_with_tests(
    (verb, python): (&str, bool),
    publication: &IndexedPublication,
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, Refusal> {
    let split = split_unit_test_selectors(
        SelectorGroups { select, exclude },
        &publication.names,
        UNIT_TEST_VERBS.contains(&verb),
    )?;
    if !split.excluded_test_names.is_empty() {
        return Err(
            "an ad-hoc run cannot exclude unit tests; select the unit tests to run instead".into(),
        );
    }
    let mut keys: BTreeSet<String> = if split.models_selected {
        resolved(publication, &split.select, &split.exclude, python)?
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

/// The published keys the selectors match; Python nodes only when the command runs them, round trips allowed because the launched command enforces the project's setting.
fn resolved(
    publication: &IndexedPublication,
    select: &[String],
    exclude: &[String],
    include_python: bool,
) -> Result<BTreeSet<String>, Refusal> {
    Ok(selected_keys(publication, select, exclude, include_python)?
        .iter()
        .flat_map(|key| node_keys(publication, key))
        .collect())
}

/// The node keys a selected graph key freezes as: an audit's under each of its targets, else one.
fn node_keys(publication: &IndexedPublication, key: &ObjectKey) -> Vec<String> {
    if key.0 == AUDIT_KIND {
        return publication.audits.get(&key.1).cloned().unwrap_or_default();
    }
    vec![key_text(publication, key)]
}

/// A selected graph key as a frozen node key; a loader's Python key names the source it loads.
pub(crate) fn key_text(publication: &IndexedPublication, (kind, name): &ObjectKey) -> String {
    if python_kind(&format!("{kind}:{name}")).is_some() {
        return python_run_key(&publication.sources, (kind, name));
    }
    format!("{kind}:{name}")
}

/// The graph keys the selectors match, SQL and Python alike.
pub(crate) fn selected_keys(
    publication: &IndexedPublication,
    select: &[String],
    exclude: &[String],
    include_python: bool,
) -> Result<BTreeSet<ObjectKey>, SelectorError> {
    let python = publication.python(include_python);
    let selectors: Vec<String> = select.iter().chain(exclude).cloned().collect();
    let node_names: Vec<&str> = python.names.iter().map(String::as_str).collect();
    let graph = if selectors_need_jobs(&publication.graph, &selectors, &node_names) {
        publication.graph_with_jobs(include_python)
    } else {
        &publication.graph
    };
    let graph = with_audits_and_runs(publication, graph, (&selectors, include_python));
    let graph = graph.as_ref();
    let sources = &publication.sources;
    let selection = resolve_python_sql_run_selection(
        SqlSelectionGraph {
            graph,
            sources,
            allow_warehouse_round_trips: true,
        },
        &python.graph,
        select,
        exclude,
    )?;
    Ok(selection
        .sql_keys
        .into_iter()
        .chain(
            selection
                .python_node_names
                .iter()
                .filter_map(|name| Some((python.kinds.get(name.as_str())?.clone(), name.clone()))),
        )
        .collect())
}

/// Managed sources `sqb load` runs; a source only a loader writes is not published as a resource but is managed.
fn managed_sources(
    publication: &IndexedPublication,
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, Refusal> {
    let unmanaged: BTreeSet<&str> = publication
        .resources
        .iter()
        .filter(|resource| resource.loader.is_none())
        .map(|resource| resource.key.as_str())
        .collect();
    let sources: BTreeSet<String> = resolved(publication, select, exclude, true)?
        .into_iter()
        .filter(|key| key.starts_with("source:") && !unmanaged.contains(key.as_str()))
        .collect();
    if sources.is_empty() {
        return Err("the selectors match no managed sources in the current publication".into());
    }
    Ok(sources)
}

/// Sources `sqb freshness` observes: selected or upstream sources, less those the exclusions reach.
fn freshness_sources(
    publication: &IndexedPublication,
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, Refusal> {
    let selected: BTreeSet<String> = upstream_sources(
        &publication.resources,
        &resolved(publication, select, &[], false)?,
    );
    let excluded: BTreeSet<String> = if exclude.is_empty() {
        BTreeSet::new()
    } else {
        upstream_sources(
            &publication.resources,
            &resolved(publication, exclude, &[], false)?,
        )
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
