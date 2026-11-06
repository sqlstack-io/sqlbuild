//! Warnings for selected models stale through changed upstreams outside the run set.

use crate::bindings::_helpers::planner_changes::results::native_enum;
use crate::bindings::_helpers::records::construct::new_record_with;
use std::collections::{HashMap, HashSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_analysis::planner_identity::constants::{
    MODEL_RESOURCE, SEED_RESOURCE, SOURCE_RESOURCE,
};
use sqlbuild_analysis::planner_identity::main::classify_selection_staleness::classify_selection_staleness;
use sqlbuild_analysis::planner_identity::main::stale_upstream_warning_message::stale_upstream_warning_message;
use sqlbuild_analysis::planner_identity::models::{
    GraphKey, StaleWarning, StalenessGraph, StalenessNames,
};

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::planner_changes::detect::{ModelPass, model_change};
use crate::bindings::_helpers::planner_identity::hashing::graph_key;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::RecordId;

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
struct StalenessInputs<'py> {
    original_scope: Bound<'py, PyAny>,
    changes: Bound<'py, PyAny>,
    snapshot: Bound<'py, PyAny>,
    identities: Bound<'py, PyAny>,
    model_changes_complete: bool,
}

fn changed_model_names(inputs: &StalenessInputs<'_>) -> PyResult<HashSet<String>> {
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

fn changed_seed_names(inputs: &StalenessInputs<'_>) -> PyResult<HashSet<String>> {
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

fn plan_warnings<'py>(
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

/// Warn for selected models stale through changed upstreams outside the run set.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_stale_out_of_selection_warnings<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_stale_out_of_selection_warnings",
        options,
        (
            &[
                "original_scope",
                "execution_scope",
                "changes",
                "snapshot",
                "version_identities",
                "source_freshness",
            ],
            &["include_sources", "model_changes_complete"],
        ),
    )?;
    let include_sources: bool =
        !arguments.passed("include_sources")? || arguments.flag("include_sources")?;
    let inputs = StalenessInputs {
        original_scope: arguments.value("original_scope")?,
        changes: arguments.value("changes")?,
        snapshot: arguments.value("snapshot")?,
        identities: arguments.value("version_identities")?,
        model_changes_complete: arguments.flag("model_changes_complete")?,
    };
    let run = scope_data(&arguments.value("execution_scope")?)?;
    let original = scope_data(&inputs.original_scope)?;
    let names = |kind: &str| -> HashSet<String> {
        run.selected
            .iter()
            .filter(|key| key.0 == kind)
            .map(|key| key.1.clone())
            .collect()
    };
    let source_freshness: Bound<'py, PyAny> = arguments.value("source_freshness")?;
    let mut changed_sources: HashSet<String> = HashSet::new();
    if include_sources && !source_freshness.is_none() {
        for identity in native_field(&source_freshness, "changed_identities")?.try_iter()? {
            changed_sources.insert(native_field(&identity?, "source_name")?.extract()?);
        }
    }
    let graph = StalenessGraph {
        upstream_deps: original
            .upstream
            .iter()
            .map(|(key, deps)| {
                (
                    GraphKey::from(key),
                    deps.iter().map(GraphKey::from).collect(),
                )
            })
            .collect(),
        selected_model_names: original
            .selected
            .iter()
            .filter(|key| key.0 == MODEL_RESOURCE)
            .map(|key| key.1.clone())
            .collect(),
        run: StalenessNames {
            models: names(MODEL_RESOURCE),
            seeds: names(SEED_RESOURCE),
            sources: names(SOURCE_RESOURCE),
        },
        changed: StalenessNames {
            models: changed_model_names(&inputs)?,
            seeds: changed_seed_names(&inputs)?,
            sources: changed_sources,
        },
    };
    plan_warnings(py, classify_selection_staleness(&graph))
}

fn name_set(values: &Bound<'_, PyAny>) -> PyResult<HashSet<String>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

/// Stale warnings for selected models with changed upstreams outside the run set.
#[pyfunction]
fn classify_selection_staleness_warnings<'py>(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_stale_out_of_selection_warnings,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        classify_selection_staleness_warnings,
        module
    )?)
}
