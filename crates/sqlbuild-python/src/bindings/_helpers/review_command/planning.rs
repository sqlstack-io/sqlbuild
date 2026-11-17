//! Resolve the review's settings, compile the project, and plan what it builds.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use serde_json::Value;
use sqlbuild_analysis::definition_changes::main::node_key::node_key;
use sqlbuild_analysis::review_command::constants::{
    DEFAULT_CRITICAL_TAG, DEFAULT_SCHEMA_PREFIX, MODEL_KIND, SEED_KIND,
};
use sqlbuild_analysis::review_command::main::changed_declarations::changed_declarations;
use sqlbuild_analysis::review_command::main::node_change_reasons::node_change_reasons;
use sqlbuild_analysis::review_command::main::parse_downstream_policy::parse_downstream_policy;
use sqlbuild_analysis::review_command::models::{
    ChangedNode, DownstreamPolicy, ReviewGraph, ReviewNode, SelectedToken,
};
use sqlbuild_config::project::models::ReviewConfig;

use crate::bindings::_helpers::change_selection::base_snapshot::{base_comparison, base_variant};
use crate::bindings::_helpers::change_selection::definition_snapshot::project_wide_keys;
use crate::bindings::_helpers::change_selection::selection::{ChangedProject, changed_project};
use crate::bindings::_helpers::review_command::support::review_error;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::build_command::main::boolean::boolean;
use sqlbuild_python_base::build_command::main::text::text;
use sqlbuild_python_base::command_runtime::main::resolve_cli_adapter::resolve_cli_adapter;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_compile::compile_driver::main::setup::build_project_graph::build_project_graph;
use sqlbuild_python_compile::compile_driver::main::stages::discover_project_inputs::discover_project_inputs;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_front::contracts::main::resolve_target_name::resolve_target_name;
use sqlbuild_python_front::discovery::main::project::configured_git_base_ref::configured_git_base_ref;
use sqlbuild_python_front::discovery::main::project::configured_review::configured_review;
use sqlbuild_python_front::execution_history::main::nullable_text::nullable_text;
use sqlbuild_python_project::compiled_views::main::compiled_models::dep_rows::dep_rows;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::compiled_views::main::project::seeds::seeds;
use sqlbuild_python_project::graph::main::python_nodes::python_node_graph::python_node_graph;
use sqlbuild_python_project::planner_selection::main::select_project_graph::select_project_graph;

const BASE_REF_ALIAS: &str = "@base";
const UDF_KIND: &str = "udf";
const TABLE_FUNCTION_KIND: &str = "table_fn";
const SOURCE_KIND: &str = "source";
const SNAPSHOT_MATERIALIZATION: &str = "snapshot";
const INCREMENTAL_MATERIALIZATION: &str = "incremental";
const LOADER_KIND: &str = "loader";
const HOOK_KEYS: [&str; 2] = ["pre_hooks", "post_hooks"];
/// A target database that keeps each node's own database.
const PRESERVE_DATABASE: &str = "preserve";

/// The flags and `[review]` defaults one review runs with.
pub(crate) struct ReviewSettings {
    pub(crate) since: String,
    pub(crate) against: String,
    pub(crate) target: String,
    pub(crate) policy: DownstreamPolicy,
    pub(crate) schema_prefix: String,
    critical_tags: Vec<String>,
}

/// The discovered project inputs, the adapter, and the compiled project graph, in that order.
pub(crate) type Compiled<'py> = (Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyAny>);

/// The project's changes since the base revision, and the graph a review plans over.
pub(crate) struct ReviewProject {
    pub(crate) project_dir: PathBuf,
    pub(crate) review_graph: ReviewGraph,
    pub(crate) changed: Vec<ChangedNode>,
    pub(crate) changed_declarations: Vec<(String, String, String)>,
    pub(crate) changes_json: Option<Value>,
    pub(crate) reference: String,
    pub(crate) merge_base: String,
    /// Incremental and snapshot models by name.
    pub(crate) stateful: BTreeSet<String>,
    /// Models and seeds whose compiled destination names a database, with that database.
    pub(crate) databases: BTreeMap<String, String>,
    /// The active target's expanded database, unless it keeps each node's own database.
    pub(crate) target_database: Option<String>,
    /// The active target's expanded schema.
    pub(crate) target_schema: Option<String>,
}

fn strings(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    value
        .try_iter()?
        .map(|item| item?.str()?.extract())
        .collect()
}

fn setting_error(py: Python<'_>, code: &str, message: &str, help: &str) -> pyo3::PyErr {
    review_error(py, code, message, Some(help))
}

/// Resolve `--since`, `--against`, the active target and the downstream policy.
pub(crate) fn review_settings(
    request: &Bound<'_, PyAny>,
    (project_dir, discovered): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<ReviewSettings> {
    let py: Python<'_> = request.py();
    let config: ReviewConfig = configured_review(project_dir)?;
    let since: String = match nullable_text(&native_field(request, "since")?)? {
        Some(since) => since,
        None if configured_git_base_ref(project_dir)?.is_some() => BASE_REF_ALIAS.to_owned(),
        None => {
            return Err(setting_error(
                py,
                "C766",
                "sqb review needs a code baseline: pass --since <ref>",
                "or set [git] base_ref in sqlbuild_project.toml",
            ));
        }
    };
    let against: String =
        match nullable_text(&native_field(request, "against")?)?.or(config.against.clone()) {
            Some(against) => against,
            None => {
                return Err(setting_error(
                    py,
                    "C767",
                    "sqb review needs a data baseline: pass --against <target>",
                    "or set [review] against in sqlbuild_project.toml",
                ));
            }
        };
    let project_config: Bound<'_, PyAny> = native_field(discovered, "project_config")?;
    if !native_field(&project_config, "targets")?.contains(&against)? {
        return Err(setting_error(
            py,
            "C768",
            &format!("--against '{against}' is not a configured target"),
            "name a target from [targets] in sqlbuild_project.toml",
        ));
    }
    let target: Option<String> = resolve_target_name(
        py,
        &project_config,
        &native_field(discovered, "local_config")?,
        &native_field(request, "selected_target")?,
    )?
    .map(|name| name.to_string());
    let Some(target) = target else {
        return Err(setting_error(
            py,
            "C769",
            "sqb review needs an active target whose connection builds the review",
            "pass --target, or set default_target or a local target",
        ));
    };
    let downstream: Option<String> =
        nullable_text(&native_field(request, "downstream")?)?.or(config.downstream.clone());
    let policy: DownstreamPolicy = match downstream.as_deref() {
        None => DownstreamPolicy::Critical,
        Some(value) => parse_downstream_policy(value).ok_or_else(|| {
            setting_error(
                py,
                "C770",
                &format!("unknown downstream policy '{value}'"),
                "use critical, all or none",
            )
        })?,
    };
    Ok(ReviewSettings {
        since,
        against,
        target,
        policy,
        schema_prefix: config
            .schema_prefix
            .unwrap_or_else(|| DEFAULT_SCHEMA_PREFIX.to_owned()),
        critical_tags: config
            .critical_tags
            .unwrap_or_else(|| vec![DEFAULT_CRITICAL_TAG.to_owned()]),
    })
}

/// The project directory and its discovered inputs.
pub(crate) fn discover<'py>(
    request: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = request.py();
    let project_dir: Bound<'py, PyAny> = match native_field(request, "project_dir")? {
        directory if !directory.is_none() => directory,
        _ => std::env::current_dir()?.into_pyobject(py)?,
    };
    let discovered: Bound<'py, PyAny> = discover_project_inputs(
        py,
        Some(keywords(py, &[("project_dir", project_dir.clone())])?),
    )?;
    Ok((project_dir, discovered))
}

fn config_value<'py>(values: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    mapping_get(values, key)
}

fn key_columns(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    if value.is_instance_of::<PyString>() {
        return Ok(vec![value.extract()?]);
    }
    strings(value)
}

/// One model as a review node, critical by its `critical` key or one of the critical tags.
fn model_node(model: &Bound<'_, PyAny>, critical_tags: &[String]) -> PyResult<ReviewNode> {
    let values: Bound<'_, PyAny> = native_field(
        &sqlbuild_python_project::compiled_views::main::compiled_models::config::config(model)?,
        "values",
    )?;
    let tags: Vec<String> = strings(&config_value(&values, "tags")?)?;
    let marked: Bound<'_, PyAny> = config_value(&values, "critical")?;
    let materialized: Bound<'_, PyAny> = config_value(&values, "materialized")?;
    Ok(ReviewNode {
        name: model_name(model)?,
        kind: MODEL_KIND.to_owned(),
        critical: (!marked.is_none() && marked.is_truthy()?)
            || tags.iter().any(|tag| critical_tags.contains(tag)),
        materialized: if materialized.is_none() {
            String::new()
        } else {
            materialized.str()?.to_string().to_ascii_lowercase()
        },
        unique_key: key_columns(&config_value(&values, "unique_key")?)?,
        has_cursor: !config_value(&values, "cursor")?.is_none(),
        has_hooks: HOOK_KEYS
            .iter()
            .map(|key| config_value(&values, key))
            .collect::<PyResult<Vec<_>>>()?
            .iter()
            .any(|hooks| !hooks.is_none() && hooks.is_truthy().unwrap_or(false)),
        ..ReviewNode::default()
    })
}

/// The database a model's or seed's compiled destination names, if any.
fn destination_database(destination: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    nullable_text(&native_field(destination, "database")?)
}

/// Dependency order over `nodes`, ties broken by name.
fn topological(
    nodes: &BTreeSet<String>,
    downstream: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let mut incoming: BTreeMap<&str, usize> = nodes.iter().map(|name| (name.as_str(), 0)).collect();
    for children in downstream.values() {
        for child in children {
            if let Some(count) = incoming.get_mut(child.as_str()) {
                *count += 1;
            }
        }
    }
    let mut ready: BTreeSet<&str> = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(name, _)| *name)
        .collect();
    let mut order: Vec<String> = Vec::new();
    while let Some(name) = ready.pop_first() {
        order.push(name.to_owned());
        for child in downstream.get(name).map_or(&[][..], Vec::as_slice) {
            if let Some(count) = incoming.get_mut(child.as_str()) {
                *count -= 1;
                if *count == 0 {
                    ready.insert(child.as_str());
                }
            }
        }
    }
    order
}

/// The review graph with each node's definition key and direct reads of sources and functions.
struct ProjectGraph {
    graph: ReviewGraph,
    keys: BTreeMap<String, String>,
    reads: BTreeMap<String, Vec<String>>,
    stateful: BTreeSet<String>,
    databases: BTreeMap<String, String>,
}

/// A Python node's name, kind, and `(kind, name)` of the SQL resources it depends on.
type PythonNode = (String, String, Vec<(String, String)>);

/// `graph` with Python nodes as `kind:name`, each loader feeding the readers of its source.
fn with_python_nodes(
    discovered: &Bound<'_, PyAny>,
    project_graph: ProjectGraph,
    sources: &BTreeMap<String, Vec<String>>,
) -> PyResult<ProjectGraph> {
    let mut project_graph: ProjectGraph = project_graph;
    let python_graph: Bound<'_, PyAny> = python_node_graph(discovered)?;
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut nodes: Vec<PythonNode> = Vec::new();
    for node in native_field(&python_graph, "nodes")?.try_iter()? {
        let node: Bound<'_, PyAny> = node?;
        let name: String = native_field(&node, "name")?.extract()?;
        let kind: String = native_field(&native_field(&node, "kind")?, "value")?.extract()?;
        let mut sql_deps: Vec<(String, String)> = Vec::new();
        for dependency in native_field(&node, "sql_deps")?.try_iter()? {
            let dependency: Bound<'_, PyAny> = dependency?;
            sql_deps.push((
                native_field(&native_field(&dependency, "kind")?, "value")?.extract()?,
                native_field(&dependency, "name")?.extract()?,
            ));
        }
        names.insert(name.clone(), format!("{kind}:{name}"));
        nodes.push((name, kind, sql_deps));
    }
    let upstream: Bound<'_, PyAny> = native_field(&python_graph, "upstream_deps")?;
    let upstream: &Bound<'_, PyDict> = upstream.cast()?;
    let graph: &mut ReviewGraph = &mut project_graph.graph;
    for (name, kind, sql_deps) in nodes {
        let node_name: String = names[&name].clone();
        let mut parents: Vec<String> = Vec::new();
        for (dependency_kind, dependency) in sql_deps {
            if dependency_kind == SOURCE_KIND {
                parents.extend(names.get(&dependency).cloned());
            } else if graph.nodes.contains_key(&dependency) {
                parents.push(dependency);
            }
        }
        if let Some(names_upstream) = upstream.get_item(&name)? {
            for parent in names_upstream.try_iter()? {
                let parent: String = parent?.extract()?;
                parents.extend(names.get(&parent).cloned());
            }
        }
        for parent in parents {
            graph
                .downstream
                .entry(parent)
                .or_default()
                .push(node_name.clone());
        }
        if kind == LOADER_KIND {
            for reader in sources.get(&name).into_iter().flatten() {
                graph
                    .downstream
                    .entry(node_name.clone())
                    .or_default()
                    .push(reader.clone());
            }
        }
        project_graph
            .keys
            .insert(node_name.clone(), node_key(&kind, &name));
        graph.nodes.insert(
            node_name.clone(),
            ReviewNode {
                name: node_name,
                kind,
                python: true,
                ..ReviewNode::default()
            },
        );
    }
    Ok(project_graph)
}

/// The graph of models, seeds and Python nodes, with reads, stateful models and own databases.
fn review_graph(
    (project, discovered): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    critical_tags: &[String],
) -> PyResult<ProjectGraph> {
    let mut project_graph = ProjectGraph {
        graph: ReviewGraph::default(),
        keys: BTreeMap::new(),
        reads: BTreeMap::new(),
        stateful: BTreeSet::new(),
        databases: BTreeMap::new(),
    };
    for seed in seeds(project)?.try_iter()? {
        let seed: Bound<'_, PyAny> = seed?;
        let name: String =
            sqlbuild_python_project::resource_views::main::seed_reads::name::name(&seed)?
                .extract()?;
        if let Some(database) = destination_database(
            &sqlbuild_python_project::resource_views::main::seed_reads::destination::destination(
                &seed,
            )?,
        )? {
            project_graph.databases.insert(name.clone(), database);
        }
        project_graph
            .keys
            .insert(name.clone(), node_key(SEED_KIND, &name));
        project_graph.graph.nodes.insert(
            name.clone(),
            ReviewNode {
                name,
                kind: SEED_KIND.to_owned(),
                materialized: SEED_KIND.to_owned(),
                ..ReviewNode::default()
            },
        );
    }
    let mut dependencies: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for model in compiled_models(project)?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        let node: ReviewNode = model_node(&model, critical_tags)?;
        if node.materialized == INCREMENTAL_MATERIALIZATION
            || node.materialized == SNAPSHOT_MATERIALIZATION
        {
            project_graph.stateful.insert(node.name.clone());
        }
        if let Some(database) = destination_database(&sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&model)?)? {
            project_graph.databases.insert(node.name.clone(), database);
        }
        project_graph
            .keys
            .insert(node.name.clone(), node_key(MODEL_KIND, &node.name));
        dependencies.push((node.name.clone(), dep_rows(&model)?));
        project_graph.graph.nodes.insert(node.name.clone(), node);
    }
    let mut sources: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, rows) in dependencies {
        let mut read: Vec<String> = Vec::new();
        for (kind, upstream) in rows {
            if (kind == MODEL_KIND || kind == SEED_KIND)
                && project_graph.graph.nodes.contains_key(&upstream)
            {
                project_graph
                    .graph
                    .downstream
                    .entry(upstream)
                    .or_default()
                    .push(name.clone());
            } else if kind == SOURCE_KIND || kind == UDF_KIND || kind == TABLE_FUNCTION_KIND {
                if kind == SOURCE_KIND {
                    sources
                        .entry(upstream.clone())
                        .or_default()
                        .push(name.clone());
                }
                read.push(node_key(&kind, &upstream));
            }
        }
        project_graph.reads.insert(name, read);
    }
    let mut project_graph: ProjectGraph = with_python_nodes(discovered, project_graph, &sources)?;
    let graph: &mut ReviewGraph = &mut project_graph.graph;
    for children in graph.downstream.values_mut() {
        children.sort();
        children.dedup();
    }
    let names: BTreeSet<String> = graph.nodes.keys().cloned().collect();
    graph.order = topological(&names, &graph.downstream);
    Ok(project_graph)
}

/// Discover, compare with the base revision, compile, and find the changed models and seeds.
pub(crate) fn review_project<'py>(
    request: &Bound<'py, PyAny>,
    (project_dir, discovered): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    settings: &ReviewSettings,
    stream: &Bound<'py, PyAny>,
) -> PyResult<(ReviewProject, Compiled<'py>)> {
    let py: Python<'py> = request.py();
    let base = base_comparison(
        (stream, &project_dir),
        &settings.since,
        &base_variant(
            native_field(request, "selected_target")?.extract()?,
            &native_field(request, "cli_vars")?,
        )?,
    )?;
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    let adapter: Bound<'py, PyAny> = resolve_cli_adapter(&adapter_name, &project_dir)?;
    let graph: Bound<'py, PyAny> = build_project_graph(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("adapter", adapter.clone()),
                ("selected_target", native_field(request, "selected_target")?),
                (
                    "no_sql_validation",
                    native_field(request, "no_sql_validation")?,
                ),
                ("cli_vars", native_field(request, "cli_vars")?),
                ("skip_column_inference", boolean(py, false)),
            ],
        )?),
    )?;
    let project: Bound<'py, PyAny> = native_field(&graph, "project")?;
    let directory: PathBuf = project_dir.extract()?;
    let changed: ChangedProject = changed_project((&project, &discovered), &directory, base)?;
    let ProjectGraph {
        graph: review_graph,
        keys,
        reads,
        stateful,
        databases,
    } = review_graph((&project, &discovered), &settings.critical_tags)?;
    let project_wide: Vec<String> = project_wide_keys();
    let mut changed_nodes: Vec<ChangedNode> = Vec::new();
    for name in &review_graph.order {
        let node: &ReviewNode = &review_graph.nodes[name];
        let key: &str = &keys[name];
        let node_reads: &[String] = reads.get(name).map_or(&[], Vec::as_slice);
        let reasons: Vec<String> = node_change_reasons(
            key,
            (node_reads, &project_wide),
            (&changed.current, &changed.changes),
        );
        if !reasons.is_empty() {
            changed_nodes.push(ChangedNode {
                name: name.clone(),
                kind: node.kind.clone(),
                reasons,
                added: changed
                    .changes
                    .changed
                    .get(key)
                    .is_some_and(|change| change.added),
            });
        }
    }
    let changes_json: Value = serde_json::from_str(&changed.json((0, &[])))
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
    let target_database: Option<String> =
        nullable_text(&native_field(&project, "effective_target_database")?)?
            .filter(|database| !database.eq_ignore_ascii_case(PRESERVE_DATABASE));
    let review = ReviewProject {
        changed_declarations: changed_declarations(&changed.changes),
        reference: changed.reference.clone(),
        merge_base: changes_json
            .get("merge_base")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        changes_json: Some(changes_json),
        project_dir: directory,
        review_graph,
        changed: changed_nodes,
        stateful,
        databases,
        target_database,
        target_schema: nullable_text(&native_field(&project, "effective_target_schema")?)?,
    };
    Ok((review, (discovered, adapter, graph)))
}

/// Each `--select` token and the model and seed names it resolves to.
pub(crate) fn selected_tokens(
    request: &Bound<'_, PyAny>,
    graph: &Bound<'_, PyAny>,
) -> PyResult<Option<Vec<SelectedToken>>> {
    let py: Python<'_> = request.py();
    let tokens: Vec<String> = strings(&native_field(request, "select")?)?;
    if tokens.is_empty() {
        return Ok(None);
    }
    let mut selected: Vec<SelectedToken> = Vec::new();
    for token in tokens {
        let select: Bound<'_, PyAny> = PyTuple::new(py, [text(py, &token)])?.into_any();
        let exclude: Bound<'_, PyAny> = PyTuple::empty(py).into_any();
        let names: Vec<String> = match select_project_graph(graph, &select, &exclude) {
            Ok(keys) => key_names(&keys)?,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyException>(py) => Vec::new(),
            Err(error) => return Err(error),
        };
        selected.push(SelectedToken { token, names });
    }
    Ok(Some(selected))
}

fn key_names(keys: &Bound<'_, PyFrozenSet>) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for key in keys.try_iter()? {
        let key: Bound<'_, PyAny> = key?;
        let kind: String = native_field(&key, "resource_type")?.str()?.extract()?;
        if kind == MODEL_KIND || kind == SEED_KIND {
            names.push(native_field(&key, "name")?.extract()?);
        }
    }
    names.sort();
    Ok(names)
}
