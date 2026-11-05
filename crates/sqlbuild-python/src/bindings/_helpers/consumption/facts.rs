//! The facts `--changed` and the consumption recorder read from the plan and compiled project.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_core::fingerprint_identity::main::model_local_identity_hash::model_local_identity_hash;
use sqlbuild_state::consumption::models::{ForcedRun, InputFact, NodeFacts, NodeKey, RelationKey};

use crate::bindings::_helpers::compiled_views::{
    function_reads, model_reads, project_reads, seed_reads,
};
use crate::bindings::_helpers::consumption::relation_reads::{Locations, sql_reads_undeclared};
use crate::bindings::_helpers::planner_identity::hashing::{graph_key, model_query_hash};
use crate::bindings::_helpers::planner_identity::identities::hook_version_hashes;
use crate::bindings::_helpers::planner_identity::metadata::{
    function_local_hashes, model_version_identity_metadata_json, seed_identity,
};
use crate::bindings::_helpers::records::fields::field as native_field;

const MODEL: &str = "model";
const SEED: &str = "seed";
const SOURCE: &str = "source";
const FUNCTION_KINDS: [&str; 2] = ["udf", "table_fn"];
/// Plan reasons that mean the planner sees no change of the model itself.
const UNCHANGED_REASONS: [&str; 2] = ["no_change", "normal_incremental"];
const FIRST_RUN: &str = "first_run";
const FULL_REFRESH: &str = "full_refresh";
const SKIP_ACTION: &str = "skip";
const SQL_LANGUAGE: &str = "sql";
const UDF: &str = "udf";

/// The selection's nodes and their ancestry, and the selected ones that execute.
pub(crate) struct BuildFacts {
    pub(crate) nodes: Vec<NodeFacts>,
    pub(crate) runnable: BTreeSet<NodeKey>,
}

/// One compiled model or seed of the ancestry walk.
enum Compiled<'py> {
    Model(Bound<'py, PyAny>),
    Seed(Bound<'py, PyAny>),
}

fn node_key(kind: &str, name: &str) -> NodeKey {
    NodeKey {
        kind: kind.to_owned(),
        name: name.to_owned(),
    }
}

fn enum_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let value: Bound<'_, PyAny> = value.getattr_opt("value")?.unwrap_or_else(|| value.clone());
    value.str()?.extract()
}

fn relation(destination: &Bound<'_, PyAny>) -> PyResult<Option<RelationKey>> {
    let schema: Bound<'_, PyAny> = native_field(destination, "schema")?;
    if schema.is_none() {
        return Ok(None);
    }
    let database: Bound<'_, PyAny> = native_field(destination, "database")?;
    Ok(Some(RelationKey {
        database: if database.is_none() {
            None
        } else {
            Some(database.extract()?)
        },
        schema: schema.extract()?,
        name: native_field(destination, "name")?.extract()?,
    }))
}

/// Each observed source's `data_version_hash`; sources loaded in this build are left out.
fn source_versions(plan: &Bound<'_, PyAny>) -> PyResult<HashMap<String, Option<String>>> {
    let mut versions: HashMap<String, Option<String>> = HashMap::new();
    let freshness: Bound<'_, PyAny> = native_field(plan, "source_freshness")?;
    if !freshness.is_none() {
        for record in native_field(&freshness, "observed_records")?.try_iter()? {
            let record: Bound<'_, PyAny> = record?;
            let name: String = native_field(&record, "source_name")?.extract()?;
            let hash: String = native_field(&record, "data_version_hash")?.extract()?;
            let entry = versions.entry(name).or_insert_with(|| Some(hash.clone()));
            if entry.as_deref() != Some(hash.as_str()) {
                *entry = None;
            }
        }
    }
    for load in native_field(plan, "source_load_entries")?.try_iter()? {
        versions.insert(native_field(&load?, "name")?.extract()?, None);
    }
    Ok(versions)
}

/// The direct inputs of a node, through functions to what they read.
fn inputs(
    deps: Vec<(String, String)>,
    function_deps: &HashMap<(String, String), Vec<(String, String)>>,
    sources: &HashMap<String, Option<String>>,
) -> Vec<InputFact> {
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut pending: VecDeque<(String, String)> = deps.into();
    let mut facts: Vec<InputFact> = Vec::new();
    while let Some((kind, name)) = pending.pop_front() {
        if !seen.insert((kind.clone(), name.clone())) {
            continue;
        }
        match kind.as_str() {
            MODEL | SEED => facts.push(InputFact::Node(node_key(&kind, &name))),
            SOURCE => facts.push(InputFact::Source {
                version: sources.get(&name).cloned().flatten(),
                name,
            }),
            function if FUNCTION_KINDS.contains(&function) => pending.extend(
                function_deps
                    .get(&(kind.clone(), name.clone()))
                    .cloned()
                    .unwrap_or_default(),
            ),
            _ => facts.push(InputFact::Opaque(format!("{kind}:{name}"))),
        }
    }
    facts
}

/// Checks that functions a model calls, transitively, read only their declared inputs.
struct FunctionReads<'a> {
    /// Each function's SQL to check, or `None` for a Python function, which reads no relation.
    bodies: &'a HashMap<(String, String), Option<String>>,
    function_deps: &'a HashMap<(String, String), Vec<(String, String)>>,
    sources: &'a HashMap<String, Option<String>>,
    locations: &'a Locations,
    dialect: Option<&'a str>,
    checked: HashMap<(String, String), bool>,
}

impl FunctionReads<'_> {
    fn undeclared(&mut self, py: Python<'_>, deps: &[(String, String)]) -> PyResult<bool> {
        let mut pending: VecDeque<(String, String)> = deps
            .iter()
            .filter(|(kind, _)| FUNCTION_KINDS.contains(&kind.as_str()))
            .cloned()
            .collect();
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        while let Some(key) = pending.pop_front() {
            if !seen.insert(key.clone()) {
                continue;
            }
            let own_deps: Vec<(String, String)> =
                self.function_deps.get(&key).cloned().unwrap_or_default();
            if self.function_undeclared(py, &key, &own_deps)? {
                return Ok(true);
            }
            pending.extend(
                own_deps
                    .into_iter()
                    .filter(|(kind, _)| FUNCTION_KINDS.contains(&kind.as_str())),
            );
        }
        Ok(false)
    }

    fn function_undeclared(
        &mut self,
        py: Python<'_>,
        key: &(String, String),
        own_deps: &[(String, String)],
    ) -> PyResult<bool> {
        if let Some(undeclared) = self.checked.get(key) {
            return Ok(*undeclared);
        }
        let undeclared: bool = match self.bodies.get(key) {
            None => true,
            Some(None) => false,
            Some(Some(sql)) => {
                let allowed: Vec<InputFact> =
                    inputs(own_deps.to_vec(), self.function_deps, self.sources);
                sql_reads_undeclared(
                    py,
                    sql.clone(),
                    (&allowed, None),
                    (self.locations, self.dialect),
                )?
            }
        };
        self.checked.insert(key.clone(), undeclared);
        Ok(undeclared)
    }
}

fn key_rows(keys: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    keys.try_iter()?
        .map(|key| {
            let key = graph_key(&key?)?;
            Ok((key.kind, key.name))
        })
        .collect()
}

/// Why the planner runs a selected entry regardless of its records.
fn entry_forced(entry: &Bound<'_, PyAny>, full_refresh: bool) -> PyResult<Vec<ForcedRun>> {
    let mut forced: Vec<ForcedRun> = Vec::new();
    if full_refresh {
        forced.push(ForcedRun::FullRefresh);
    }
    match enum_text(&native_field(entry, "reason")?)?.as_str() {
        FIRST_RUN => forced.push(ForcedRun::RelationMissing),
        FULL_REFRESH => forced.push(ForcedRun::FullRefresh),
        reason if UNCHANGED_REASONS.contains(&reason) => {}
        reason => forced.push(ForcedRun::PlannerChange(reason.to_owned())),
    }
    for field in ["start_cursor_override", "end_cursor_override"] {
        if entry.hasattr(field)? && !native_field(entry, field)?.is_none() {
            forced.push(ForcedRun::CursorInterval);
            break;
        }
    }
    Ok(forced)
}

/// Facts of the selected models and seeds and of every model and seed they read from.
pub(crate) fn build_facts(
    (plan, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (changed, full_refresh): (bool, bool),
) -> PyResult<BuildFacts> {
    let py: Python<'_> = plan.py();
    let mut compiled: HashMap<NodeKey, Compiled<'_>> = HashMap::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        compiled.insert(
            node_key(MODEL, &model_reads::model_name(&model)?),
            Compiled::Model(model),
        );
    }
    for seed in project_reads::seeds(project)?.try_iter()? {
        let seed: Bound<'_, PyAny> = seed?;
        compiled.insert(
            node_key(SEED, &seed_reads::name(&seed)?.extract::<String>()?),
            Compiled::Seed(seed),
        );
    }
    let mut function_deps: HashMap<(String, String), Vec<(String, String)>> = HashMap::new();
    let mut function_bodies: HashMap<(String, String), Option<String>> = HashMap::new();
    let functions: Bound<'_, PyAny> = project_reads::functions(project)?;
    for function in functions.try_iter()? {
        let function: Bound<'_, PyAny> = function?;
        let key = graph_key(&function_reads::key(&function)?)?;
        function_deps.insert(
            (key.kind.clone(), key.name.clone()),
            key_rows(&function_reads::deps(&function)?)?,
        );
        let body: Option<String> =
            if enum_text(&function_reads::language(&function)?)? == SQL_LANGUAGE {
                let body: String = function_reads::body_sql(&function)?.extract()?;
                Some(if key.kind == UDF {
                    format!("SELECT ({body})")
                } else {
                    body
                })
            } else {
                None
            };
        function_bodies.insert((key.kind, key.name), body);
    }
    let mut selected: BTreeMap<NodeKey, Vec<ForcedRun>> = BTreeMap::new();
    let mut runnable: BTreeSet<NodeKey> = BTreeSet::new();
    for (entries, kind) in [("model_entries", MODEL), ("seed_entries", SEED)] {
        for entry in native_field(plan, entries)?.try_iter()? {
            let entry: Bound<'_, PyAny> = entry?;
            let key: NodeKey = node_key(kind, &native_field(&entry, "name")?.extract::<String>()?);
            if enum_text(&native_field(&entry, "action")?)? != SKIP_ACTION {
                runnable.insert(key.clone());
            }
            let forced: Vec<ForcedRun> = if kind == MODEL {
                entry_forced(&entry, full_refresh)?
            } else {
                seed_forced(&entry, full_refresh)?
            };
            selected.insert(key, forced);
        }
    }
    let sources: HashMap<String, Option<String>> = source_versions(plan)?;
    let existing: BTreeSet<String> = native_field(plan, "existing_model_names")?
        .try_iter()?
        .map(|name| name?.extract())
        .collect::<PyResult<_>>()?;
    let dialect: Option<String> = native_field(project, "sql_analysis_dialect")?.extract()?;
    let function_hashes: Bound<'_, PyDict> = function_local_hashes(&functions, dialect.as_deref())?;
    let hooks: Bound<'_, PyAny> = native_field(project, "hook_functions")?;
    let hook_hashes: Bound<'_, PyDict> = hook_version_hashes(&hooks)?;
    let locations: Option<Locations> = if changed {
        Some(Locations::read(plan, project)?)
    } else {
        None
    };
    let mut function_checks: Option<FunctionReads<'_>> =
        locations.as_ref().map(|locations| FunctionReads {
            bodies: &function_bodies,
            function_deps: &function_deps,
            sources: &sources,
            locations,
            dialect: dialect.as_deref(),
            checked: HashMap::new(),
        });
    let mut nodes: Vec<NodeFacts> = Vec::new();
    let mut pending: VecDeque<NodeKey> = selected.keys().cloned().collect();
    let mut visited: BTreeSet<NodeKey> = BTreeSet::new();
    while let Some(key) = pending.pop_front() {
        if !visited.insert(key.clone()) {
            continue;
        }
        let Some(node) = compiled.get(&key) else {
            continue;
        };
        let (destination, deps, version_hash) = match node {
            Compiled::Model(model) => (
                model_reads::destination(model)?,
                model_reads::dep_rows(model)?,
                model_local_identity_hash(
                    &model_query_hash(
                        py,
                        &key.name,
                        (
                            &model_reads::query_sql(model)?.extract::<String>()?,
                            dialect.as_deref(),
                        ),
                    )?,
                    &model_version_identity_metadata_json(
                        model,
                        Some(function_hashes.as_any()),
                        Some(hook_hashes.as_any()),
                    )?,
                ),
            ),
            Compiled::Seed(seed) => (
                seed_reads::destination(seed)?,
                Vec::new(),
                seed_identity(seed)?.0,
            ),
        };
        let node_inputs: Vec<InputFact> = inputs(deps.clone(), &function_deps, &sources);
        for input in &node_inputs {
            if let InputFact::Node(upstream) = input {
                pending.push_back(upstream.clone());
            }
        }
        let mut forced: Vec<ForcedRun> = match selected.get(&key) {
            Some(forced) => forced.clone(),
            None if key.kind == MODEL && !existing.contains(&key.name) => {
                vec![ForcedRun::RelationMissing]
            }
            None => Vec::new(),
        };
        let relation: Option<RelationKey> = relation(&destination)?;
        if let (Compiled::Model(model), Some(locations), Some(function_checks)) =
            (node, &locations, &mut function_checks)
            && (sql_reads_undeclared(
                py,
                model_reads::query_sql(model)?.extract()?,
                (&node_inputs, relation.as_ref()),
                (locations, dialect.as_deref()),
            )? || function_checks.undeclared(py, &deps)?)
        {
            forced.push(ForcedRun::InputsNotEnumerable);
        }
        let Some(relation) = relation else {
            continue;
        };
        nodes.push(NodeFacts {
            selected: selected.contains_key(&key),
            key,
            relation,
            version_hash,
            forced,
            inputs: node_inputs,
        });
    }
    runnable.retain(|key| nodes.iter().any(|node| node.key == *key));
    Ok(BuildFacts { nodes, runnable })
}

fn seed_forced(entry: &Bound<'_, PyAny>, full_refresh: bool) -> PyResult<Vec<ForcedRun>> {
    let mut forced: Vec<ForcedRun> = Vec::new();
    if full_refresh {
        forced.push(ForcedRun::FullRefresh);
    }
    match enum_text(&native_field(entry, "reason")?)?.as_str() {
        FIRST_RUN => forced.push(ForcedRun::RelationMissing),
        reason if UNCHANGED_REASONS.contains(&reason) => {}
        reason => forced.push(ForcedRun::PlannerChange(reason.to_owned())),
    }
    Ok(forced)
}
