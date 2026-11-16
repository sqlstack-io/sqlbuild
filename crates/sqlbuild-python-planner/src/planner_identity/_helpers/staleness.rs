//! Warnings for selected models stale through changed upstreams outside the run set.

use crate::planner_changes::main::native_enum::native_enum;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use std::collections::{HashMap, HashSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyString, PyTuple};

use sqlbuild_analysis::planner_identity::main::classify_selection_staleness::classify_selection_staleness;
use sqlbuild_analysis::planner_identity::main::stale_upstream_warning_message::stale_upstream_warning_message;
use sqlbuild_analysis::planner_identity::models::{
    GraphKey, StaleWarning, StalenessGraph, StalenessNames,
};

use crate::planner_changes::main::model_change::model_change;
use crate::planner_changes::models::ModelPass;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::planner_identity::main::graph_key::graph_key;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;

/// Change kinds that mean the model's own identity changed.
const OWN_CHANGE_KINDS: [&str; 5] = [
    "first_run",
    "query_changed",
    "function_changed",
    "config_changed",
    "schema_changed",
];

fn own_change(change: &Bound<'_, PyAny>) -> PyResult<bool> {
    let kind: String = native_field(change, "change_kind")?.str()?.extract()?;
    Ok(OWN_CHANGE_KINDS.contains(&kind.as_str()))
}

fn upstream_graph(upstream_deps: &Bound<'_, PyAny>) -> PyResult<HashMap<GraphKey, Vec<GraphKey>>> {
    let mut deps: HashMap<GraphKey, Vec<GraphKey>> = HashMap::new();
    for item in mapping_items(upstream_deps)?.try_iter()? {
        let (key, upstream): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
        let upstream_keys: Vec<GraphKey> = upstream
            .try_iter()?
            .map(|upstream_key| graph_key(&upstream_key?))
            .collect::<PyResult<_>>()?;
        deps.insert(graph_key(&key)?, upstream_keys);
    }
    Ok(deps)
}

/// Inputs of one stale-warning pass.
pub(crate) struct StalenessInputs<'py> {
    pub(crate) original_scope: Bound<'py, PyAny>,
    pub(crate) changes: Bound<'py, PyAny>,
    pub(crate) snapshot: Bound<'py, PyAny>,
    pub(crate) identities: Bound<'py, PyAny>,
    pub(crate) model_changes_complete: bool,
}

pub(crate) fn changed_model_names(inputs: &StalenessInputs<'_>) -> PyResult<HashSet<String>> {
    let py: Python<'_> = inputs.snapshot.py();
    let models: Bound<'_, PyAny> = native_field(&inputs.changes, "models")?;
    let mut changed: HashSet<String> = HashSet::new();
    for item in mapping_items(&models)?.try_iter()? {
        let (name, change): (String, Bound<'_, PyAny>) = item?.extract()?;
        if own_change(&change)? {
            changed.insert(name);
        }
    }
    let version_hashes: Bound<'_, PyAny> =
        native_field(&inputs.identities, "model_version_hashes")?;
    let metadata_jsons: Bound<'_, PyAny> =
        native_field(&inputs.identities, "model_metadata_jsons")?;
    let pass = ModelPass {
        snapshot: inputs.snapshot.clone(),
        query_change_tracking: true,
        full_refresh: false,
        function_local_hashes: py.None().into_bound(py),
        hook_version_hashes: py.None().into_bound(py),
    };
    let scope = scope_data(&inputs.original_scope)?;
    for (name, model) in &scope.models {
        let model = model.bind(py);
        let selected_change: Bound<'_, PyAny> = mapping_get(&models, name)?;
        if !selected_change.is_none()
            && (inputs.model_changes_complete || own_change(&selected_change)?)
        {
            if own_change(&selected_change)? {
                changed.insert(name.clone());
            }
            continue;
        }
        if model.is_none() {
            continue;
        }
        let change: Bound<'_, PyAny> = model_change(
            &pass,
            &model,
            (
                mapping_get(&version_hashes, name)?.extract()?,
                mapping_get(&metadata_jsons, name)?.extract()?,
            ),
        )?;
        if own_change(&change)? {
            changed.insert(name.clone());
        }
    }
    Ok(changed)
}

pub(crate) fn changed_seed_names(inputs: &StalenessInputs<'_>) -> PyResult<HashSet<String>> {
    let recorded: Bound<'_, PyAny> =
        native_field(&native_field(&inputs.snapshot, "fingerprints")?, "seeds")?;
    let mut changed: HashSet<String> = HashSet::new();
    for item in native_field(&inputs.identities, "seed_version_hashes")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, expected): (String, Bound<'_, PyAny>) = item?.extract()?;
        if expected.is_none() {
            continue;
        }
        let fingerprint: Bound<'_, PyAny> = mapping_get(&recorded, &name)?;
        if fingerprint.is_none() || !native_field(&fingerprint, "version_hash")?.eq(&expected)? {
            changed.insert(name.clone());
        }
    }
    Ok(changed)
}

pub(crate) fn plan_warnings<'py>(
    py: Python<'py>,
    warnings: Vec<StaleWarning>,
) -> PyResult<Bound<'py, PyTuple>> {
    let severity: Bound<'py, PyAny> = native_enum(py, RecordId::WarningSeverity, "warning")?;
    let mut rendered: Vec<Bound<'py, PyAny>> = Vec::with_capacity(warnings.len());
    for warning in warnings {
        let message: String = stale_upstream_warning_message(
            ("selected model", &warning.model_name),
            "upstream(s)",
            &warning.trigger_names,
        );
        rendered.push(new_record_with(
            py,
            RecordId::PlanWarning,
            &[
                (
                    "model_name",
                    PyString::new(py, &warning.model_name).into_any(),
                ),
                ("severity", severity.clone()),
                ("message", PyString::new(py, &message).into_any()),
            ],
        )?);
    }
    PyTuple::new(py, rendered)
}

fn name_set(values: &Bound<'_, PyAny>) -> PyResult<HashSet<String>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

/// Stale warnings for selected models with changed upstreams outside the run set.
#[pyfunction]
pub(crate) fn classify_selection_staleness_warnings<'py>(
    graph: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = graph.py();
    let staleness = StalenessGraph {
        upstream_deps: upstream_graph(&native_field(graph, "upstream_deps")?)?,
        selected_model_names: name_set(&native_field(graph, "selected_model_names")?)?
            .into_iter()
            .collect(),
        run: StalenessNames {
            models: name_set(&native_field(graph, "run_model_names")?)?,
            seeds: name_set(&native_field(graph, "run_seed_names")?)?,
            sources: name_set(&native_field(graph, "run_source_names")?)?,
        },
        changed: StalenessNames {
            models: name_set(&native_field(graph, "changed_model_names")?)?,
            seeds: name_set(&native_field(graph, "changed_seed_names")?)?,
            sources: name_set(&native_field(graph, "changed_source_names")?)?,
        },
    };
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    for warning in classify_selection_staleness(&staleness) {
        warnings.push(new_record_with(
            py,
            RecordId::SelectionStalenessWarning,
            &[
                (
                    "model_name",
                    PyString::new(py, &warning.model_name).into_any(),
                ),
                (
                    "trigger_names",
                    PyTuple::new(py, warning.trigger_names)?.into_any(),
                ),
            ],
        )?);
    }
    PyTuple::new(py, warnings)
}
