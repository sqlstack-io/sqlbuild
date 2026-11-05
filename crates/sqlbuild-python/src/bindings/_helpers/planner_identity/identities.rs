//! Direct model version identities and honest write hashes over the planner scope.

use std::collections::{HashMap, HashSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_analysis::planner_identity::constants::{
    MODEL_RESOURCE, SEED_RESOURCE, SOURCE_RESOURCE, TABLE_FN_RESOURCE, UDF_RESOURCE,
};
use sqlbuild_analysis::planner_identity::main::expected_graph_identity_hashes::expected_graph_identity_hashes;
use sqlbuild_analysis::planner_identity::main::graph_write_identity_hashes::graph_write_identity_hashes;
use sqlbuild_analysis::planner_identity::models::{GraphKey, IdentityGraph, IdentityNode};
use sqlbuild_core::fingerprint_identity::main::model_local_identity_hash::model_local_identity_hash;
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compiled_views::function_reads;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::seed_reads;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_identity::hashing::{graph_key, model_query_hash};
use crate::bindings::_helpers::planner_identity::metadata::{
    function_local_hashes, model_version_identity_metadata_json, seed_identity,
};
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::fields::replaced;
use crate::bindings::_helpers::records::ids::RecordId;

fn key_of(kind: &str, name: &str) -> GraphKey {
    GraphKey {
        kind: kind.to_owned(),
        name: name.to_owned(),
    }
}

fn graph_keys(keys: &Bound<'_, PyAny>) -> PyResult<Vec<GraphKey>> {
    keys.try_iter()?.map(|key| graph_key(&key?)).collect()
}

fn string_pairs(mapping: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    mapping_items(mapping)?
        .try_iter()?
        .map(|item| item?.extract::<(String, String)>())
        .collect()
}

/// `{name: identity.version_hash}` of the discovered Python hooks.
pub(crate) fn hook_version_hashes<'py>(
    hook_functions: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = hook_functions.py();
    let hashes: Bound<'py, PyDict> = PyDict::new(py);
    let identities: Bound<'py, PyAny> =
        host::entry(py, UserCodeHost::BuildHookIdentities)?.call1((hook_functions,))?;
    for item in mapping_items(&identities)?.try_iter()? {
        let (name, identity): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        hashes.set_item(name, native_field(&identity, "version_hash")?)?;
    }
    Ok(hashes)
}

/// Current direct model identities from code and upstream identities.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_direct_model_version_identities<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_direct_model_version_identities",
        options,
        (
            &["functions", "scope", "dialect"],
            &["seeds", "source_version_hashes", "hook_functions"],
        ),
    )?;
    direct_model_version_identities(
        (&arguments.value("functions")?, &arguments.value("scope")?),
        arguments.value("dialect")?.extract()?,
        (
            &arguments.value("seeds")?,
            &arguments.value("source_version_hashes")?,
            &arguments.value("hook_functions")?,
        ),
    )
}

/// Current direct model identities of `(functions, scope)` with optional seeds, source hashes and hooks.
pub(crate) fn direct_model_version_identities<'py>(
    (functions, scope): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    dialect: Option<String>,
    (seeds, source_hashes, hooks): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = functions.py();
    let function_hashes: Bound<'py, PyDict> = function_local_hashes(functions, dialect.as_deref())?;
    let hook_hashes: Bound<'py, PyDict> = if hooks.is_none() {
        PyDict::new(py)
    } else {
        hook_version_hashes(hooks)?
    };
    let mut graph = IdentityGraph::default();
    for function in functions.try_iter()? {
        let function: Bound<'py, PyAny> = function?;
        let name: String = function_reads::name(&function)?.extract()?;
        let Some(function_hash) = function_hashes.get_item(&name)? else {
            continue;
        };
        graph.nodes.insert(
            graph_key(&function_reads::key(&function)?)?,
            IdentityNode {
                upstream_keys: graph_keys(&function_reads::deps(&function)?)?,
                local_hash: Some(function_hash.extract()?),
            },
        );
    }
    let seed_hashes: Bound<'py, PyDict> = PyDict::new(py);
    let seed_metadata: Bound<'py, PyDict> = PyDict::new(py);
    if !seeds.is_none() {
        for seed in seeds.try_iter()? {
            let seed: Bound<'py, PyAny> = seed?;
            let (seed_hash, metadata_json): (String, String) = seed_identity(&seed)?;
            let name: Bound<'py, PyAny> = seed_reads::name(&seed)?;
            seed_hashes.set_item(&name, &seed_hash)?;
            seed_metadata.set_item(&name, metadata_json)?;
        }
    }
    for (name, seed_hash) in string_pairs(seed_hashes.as_any())? {
        graph.nodes.insert(
            key_of(SEED_RESOURCE, &name),
            IdentityNode {
                upstream_keys: Vec::new(),
                local_hash: Some(seed_hash),
            },
        );
    }
    if source_hashes.is_truthy()? {
        for (name, source_hash) in string_pairs(source_hashes)? {
            graph.nodes.insert(
                key_of(SOURCE_RESOURCE, &name),
                IdentityNode {
                    upstream_keys: Vec::new(),
                    local_hash: Some(source_hash),
                },
            );
        }
    }
    let model_metadata: Bound<'py, PyDict> = PyDict::new(py);
    let model_local: Bound<'py, PyDict> = PyDict::new(py);
    let scope = scope_data(scope)?;
    for key in &scope.order {
        let order_key = GraphKey::from(key);
        graph.execution_order.push(order_key.clone());
        if order_key.kind != MODEL_RESOURCE {
            continue;
        }
        let Some(model) = scope.model(py, &order_key.name) else {
            continue;
        };
        let name: String = model_reads::model_name(&model)?;
        let metadata_json: String = model_version_identity_metadata_json(
            &model,
            Some(function_hashes.as_any()),
            Some(hook_hashes.as_any()),
        )?;
        let query_fingerprint: String = model_query_hash(
            py,
            &name,
            (
                &model_reads::query_sql(&model)?.extract::<String>()?,
                dialect.as_deref(),
            ),
        )?;
        let local_hash: String = model_local_identity_hash(&query_fingerprint, &metadata_json);
        model_metadata.set_item(&name, metadata_json)?;
        model_local.set_item(&name, &local_hash)?;
        graph.nodes.insert(
            key_of(MODEL_RESOURCE, &name),
            IdentityNode {
                upstream_keys: graph_keys(&model_reads::deps(&model)?)?,
                local_hash: Some(local_hash),
            },
        );
    }
    let graph_hashes: HashMap<GraphKey, Option<String>> =
        expected_graph_identity_hashes(&graph).into_iter().collect();
    let version_hashes: Bound<'py, PyDict> = function_hashes.copy()?;
    version_hashes.update(seed_hashes.as_mapping())?;
    for name in model_local.keys() {
        let name: String = name.extract()?;
        if let Some(Some(model_hash)) = graph_hashes.get(&key_of(MODEL_RESOURCE, &name)) {
            version_hashes.set_item(&name, model_hash)?;
        }
    }
    new_record_with(
        py,
        RecordId::DirectModelVersionIdentities,
        &[
            ("function_local_hashes", function_hashes.into_any()),
            ("seed_version_hashes", seed_hashes.into_any()),
            ("seed_metadata_jsons", seed_metadata.into_any()),
            ("model_metadata_jsons", model_metadata.into_any()),
            ("model_local_hashes", model_local.into_any()),
            ("model_version_hashes", version_hashes.into_any()),
        ],
    )
}

/// Planned write hashes replaced by hashes based on upstreams available this run.
pub(crate) fn with_honest_model_write_hashes<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "with_honest_model_write_hashes",
        options,
        (
            &["scope", "snapshot", "changes", "version_identities"],
            &["available_model_hashes"],
        ),
    )?;
    let scope: Bound<'py, PyAny> = arguments.value("scope")?;
    let snapshot: Bound<'py, PyAny> = arguments.value("snapshot")?;
    let changes: Bound<'py, PyAny> = arguments.value("changes")?;
    let identities: Bound<'py, PyAny> = arguments.value("version_identities")?;
    let scope = scope_data(&scope)?;
    let selected: HashSet<GraphKey> = scope.selected.iter().map(GraphKey::from).collect();
    let function_hashes: Vec<(String, String)> =
        string_pairs(&native_field(&identities, "function_local_hashes")?)?;
    let mut graph = IdentityGraph::default();
    let mut base_hashes: Vec<(GraphKey, String)> = Vec::new();
    let mut function_deps: HashMap<GraphKey, Vec<GraphKey>> = HashMap::new();
    for (key, deps) in &scope.upstream {
        let key = GraphKey::from(key);
        if key.kind == UDF_RESOURCE || key.kind == TABLE_FN_RESOURCE {
            function_deps.insert(key, deps.iter().map(GraphKey::from).collect());
        }
    }
    for (name, function_hash) in &function_hashes {
        for kind in [UDF_RESOURCE, TABLE_FN_RESOURCE] {
            let function_key: GraphKey = key_of(kind, name);
            graph.nodes.insert(
                function_key.clone(),
                IdentityNode {
                    upstream_keys: function_deps
                        .get(&function_key)
                        .cloned()
                        .unwrap_or_default(),
                    local_hash: Some(function_hash.clone()),
                },
            );
            base_hashes.push((function_key, function_hash.clone()));
        }
    }
    let recorded_seeds: Bound<'py, PyAny> =
        native_field(&native_field(&snapshot, "fingerprints")?, "seeds")?;
    for (name, seed_hash) in string_pairs(&native_field(&identities, "seed_version_hashes")?)? {
        let seed_key: GraphKey = key_of(SEED_RESOURCE, &name);
        graph.nodes.insert(
            seed_key.clone(),
            IdentityNode {
                upstream_keys: Vec::new(),
                local_hash: Some(seed_hash.clone()),
            },
        );
        let fingerprint: Bound<'py, PyAny> = mapping_get(&recorded_seeds, &name)?;
        let write_hash: String = if selected.contains(&seed_key) || fingerprint.is_none() {
            seed_hash
        } else {
            native_field(&fingerprint, "version_hash")?.extract()?
        };
        base_hashes.push((seed_key, write_hash));
    }
    let local_hashes: Bound<'py, PyAny> = native_field(&identities, "model_local_hashes")?;
    for model in scope.models.values() {
        let model = model.bind(py);
        let name: Bound<'py, PyAny> = model_reads::name_object(&model)?;
        graph.nodes.insert(
            key_of(MODEL_RESOURCE, &name.extract::<String>()?),
            IdentityNode {
                upstream_keys: graph_keys(&model_reads::deps(&model)?)?,
                local_hash: mapping_get(&local_hashes, &name)?.extract()?,
            },
        );
    }
    let recorded_models: Bound<'py, PyAny> =
        native_field(&native_field(&snapshot, "fingerprints")?, "models")?;
    for item in mapping_items(&recorded_models)?.try_iter()? {
        let (name, fingerprint): (String, Bound<'py, PyAny>) = item?.extract()?;
        let version_hash: Bound<'py, PyAny> = native_field(&fingerprint, "version_hash")?;
        if version_hash.is_truthy()? {
            base_hashes.push((key_of(MODEL_RESOURCE, &name), version_hash.extract()?));
        }
    }
    let available: Bound<'py, PyAny> = arguments.value("available_model_hashes")?;
    if available.is_truthy()? {
        for (name, model_hash) in string_pairs(&available)? {
            base_hashes.push((key_of(MODEL_RESOURCE, &name), model_hash));
        }
    }
    graph.execution_order = scope.order.iter().map(GraphKey::from).collect();
    let write_hashes: HashMap<GraphKey, String> =
        graph_write_identity_hashes(&graph, &selected, base_hashes)
            .into_iter()
            .collect();
    let model_changes: Bound<'py, PyDict> = PyDict::new(py);
    model_changes.update(
        native_field(&changes, "models")?
            .downcast::<PyDict>()?
            .as_mapping(),
    )?;
    for key in &scope.order {
        let order_key = GraphKey::from(key);
        if order_key.kind != MODEL_RESOURCE || !selected.contains(&order_key) {
            continue;
        }
        let Some(model) = scope.model(py, &order_key.name) else {
            continue;
        };
        let name: Bound<'py, PyAny> = model_reads::name_object(&model)?;
        if mapping_get(&local_hashes, &name)?.is_none() {
            continue;
        }
        let Some(write_hash) =
            write_hashes.get(&key_of(MODEL_RESOURCE, &name.extract::<String>()?))
        else {
            continue;
        };
        let Some(change) = model_changes.get_item(&name)? else {
            continue;
        };
        model_changes.set_item(
            &name,
            replace_with(
                &change,
                (
                    "fingerprint_version_hash",
                    PyString::new(py, write_hash).into_any(),
                ),
            )?,
        )?;
    }
    replace_with(&changes, ("models", model_changes.into_any()))
}

/// Resolved actions with recomputed model changes patched in where available.
#[pyfunction]
#[pyo3(signature = (*, resolved_actions, changes))]
pub(crate) fn merge_recomputed_model_changes<'py>(
    resolved_actions: &Bound<'py, PyAny>,
    changes: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = resolved_actions.py();
    let recomputed: Bound<'py, PyAny> = native_field(changes, "models")?;
    let merged: Bound<'py, PyDict> = PyDict::new(py);
    for item in native_field(resolved_actions, "models")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, resolved): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        let mut change: Bound<'py, PyAny> = mapping_get(&recomputed, &name)?;
        if change.is_none() {
            change = native_field(&resolved, "change")?;
        }
        merged.set_item(name, replace_with(&resolved, ("change", change))?)?;
    }
    replace_with(resolved_actions, ("models", merged.into_any()))
}

fn replace_with<'py>(
    instance: &Bound<'py, PyAny>,
    (field, value): (&str, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    replaced(instance, &[(field, value)])
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_direct_model_version_identities,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(merge_recomputed_model_changes, module)?)
}
