//! The combined dbt and SQLBuild graph, `sqb dbt` selector resolution and dbt selector terms.

use sqlbuild_python_base::records::main::construct::new_error::new_error;
use std::collections::{HashMap, HashSet};

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_core::dbt_graph::constants::PATH_SELECTOR_ERROR_CODE;
use sqlbuild_core::dbt_graph::errors::DbtSelectionError;
use sqlbuild_core::dbt_graph::main::build_dbt_combined_graph::build_dbt_combined_graph as build_graph;
use sqlbuild_core::dbt_graph::main::dbt_downstream_sqlbuild_models::dbt_downstream_sqlbuild_models;
use sqlbuild_core::dbt_graph::main::dbt_fqn_selector_term::dbt_fqn_selector_term as fqn_selector_term;
use sqlbuild_core::dbt_graph::main::resolve_dbt_selection::resolve_dbt_selection;
use sqlbuild_core::dbt_graph::models::{
    DbtCombinedGraph as CombinedGraph, DbtGraphEdges, DbtGraphManifest, DbtGraphManifestModel,
    DbtGraphProjectModel, DbtSelection, DbtSelectionRequest, DbtSelectorExpansion,
};
use sqlbuild_core::dbt_graph::types::DbtSelectorResolver;

use crate::bindings::_helpers::dbt_interop::manifest::resolve_sqlbuild_model_dbt_refs;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::planner_selection::main::resolve_sqlbuild_model_selector_names::resolve_sqlbuild_model_selector_names;
use sqlbuild_python_project::planner_selection::main::split_selector_expansion::split_selector_expansion;

const MODEL_RESOURCE_TYPE: &str = "model";
const SELECTION_OPTIONS: (&[&str], &[&str]) = (
    &["project", "graph", "select"],
    &["manifest", "exclude", "dbt_anchor_unique_ids_by_term"],
);

/// The combined downstream-only dbt and SQLBuild graph, keyed by `owner:type:name` stable IDs.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct DbtCombinedGraph {
    graph: CombinedGraph,
}

fn stable_edges<'py>(py: Python<'py>, edges: &DbtGraphEdges) -> PyResult<Bound<'py, PyDict>> {
    let result: Bound<'py, PyDict> = PyDict::new(py);
    for key in &edges.order {
        let targets: Vec<String> = edges
            .targets
            .get(key)
            .into_iter()
            .flatten()
            .map(|target| target.stable_id())
            .collect();
        result.set_item(key.stable_id(), PyTuple::new(py, targets)?)?;
    }
    Ok(result)
}

impl DbtCombinedGraph {
    /// The native graph.
    pub(crate) fn native(&self) -> &CombinedGraph {
        &self.graph
    }
}

#[pymethods]
impl DbtCombinedGraph {
    /// Every node's stable ID.
    #[getter]
    pub(crate) fn nodes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyFrozenSet>> {
        PyFrozenSet::new(
            py,
            self.graph.upstream.order.iter().map(|key| key.stable_id()),
        )
    }

    /// Upstream edges by stable ID.
    #[getter]
    pub(crate) fn upstream_deps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        stable_edges(py, &self.graph.upstream)
    }

    /// Downstream edges by stable ID.
    #[getter]
    pub(crate) fn downstream_deps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        stable_edges(py, &self.graph.downstream)
    }

    /// Sorted SQLBuild model names transitively downstream of any of the dbt models.
    fn downstream_sqlbuild_model_names<'py>(
        &self,
        py: Python<'py>,
        unique_ids: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let unique_ids: Vec<String> = texts(unique_ids)?;
        PyTuple::new(py, dbt_downstream_sqlbuild_models(&self.graph, &unique_ids))
    }
}

pub(crate) fn texts(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

fn key_set(mapping: &Bound<'_, PyAny>) -> PyResult<HashSet<String>> {
    Ok(texts(mapping)?.into_iter().collect())
}

fn graph_manifest(manifest: &Bound<'_, PyAny>) -> PyResult<DbtGraphManifest> {
    let mut models: Vec<DbtGraphManifestModel> = Vec::new();
    for model in native_field(manifest, "models_by_unique_id")?
        .call_method0("values")?
        .try_iter()?
    {
        let model: Bound<'_, PyAny> = model?;
        models.push(DbtGraphManifestModel {
            unique_id: native_field(&model, "unique_id")?.extract()?,
            name: native_field(&model, "name")?.extract()?,
            depends_on_nodes: texts(&native_field(&model, "depends_on_nodes")?)?,
        });
    }
    Ok(DbtGraphManifest {
        models,
        source_ids: key_set(&native_field(manifest, "sources_by_unique_id")?)?,
        seed_ids: key_set(&native_field(manifest, "seeds_by_unique_id")?)?,
    })
}

fn project_models<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
    manifest: &Bound<'py, PyAny>,
) -> PyResult<Vec<DbtGraphProjectModel>> {
    let mut refs_by_name: HashMap<String, Vec<String>> = HashMap::new();
    for pair in
        resolve_sqlbuild_model_dbt_refs(py, project.clone(), manifest.clone(), None)?.try_iter()?
    {
        let (model, dbt_model): (Bound<'py, PyAny>, Bound<'py, PyAny>) = pair?.extract()?;
        refs_by_name
            .entry(native_field(&model, "name")?.extract()?)
            .or_default()
            .push(native_field(&dbt_model, "unique_id")?.extract()?);
    }
    let mut models: Vec<DbtGraphProjectModel> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let name: String = native_field(&model, "name")?.extract()?;
        let mut model_deps: Vec<String> = Vec::new();
        for dependency in native_field(&model, "deps")?.try_iter()? {
            let dependency: Bound<'py, PyAny> = dependency?;
            if native_field(&dependency, "resource_type")?.eq(MODEL_RESOURCE_TYPE)? {
                model_deps.push(native_field(&dependency, "name")?.extract()?);
            }
        }
        let dbt_ref_unique_ids: Vec<String> = refs_by_name.get(&name).cloned().unwrap_or_default();
        models.push(DbtGraphProjectModel {
            name,
            model_deps,
            dbt_ref_unique_ids,
        });
    }
    Ok(models)
}

/// Build the combined graph from a dbt manifest index and a compiled SQLBuild project.
#[pyfunction]
#[pyo3(signature = (*, manifest, project))]
pub(crate) fn build_dbt_combined_graph<'py>(
    py: Python<'py>,
    manifest: Bound<'py, PyAny>,
    project: Bound<'py, PyAny>,
) -> PyResult<DbtCombinedGraph> {
    let models: Vec<DbtGraphProjectModel> = project_models(py, &project, &manifest)?;
    Ok(DbtCombinedGraph {
        graph: build_graph(&graph_manifest(&manifest)?, &models),
    })
}

pub(crate) struct PlannerResolver<'py> {
    project: Bound<'py, PyAny>,
}

/// The planner's selector parsing and SQLBuild model selector resolution over `project`.
pub(crate) fn planner_resolver(project: Bound<'_, PyAny>) -> PyResult<PlannerResolver<'_>> {
    Ok(PlannerResolver { project })
}

impl DbtSelectorResolver for PlannerResolver<'_> {
    type Error = PyErr;

    fn split(&self, term: &str) -> PyResult<DbtSelectorExpansion> {
        let parsed: Bound<'_, PyAny> =
            split_selector_expansion(&PyString::new(self.project.py(), term))?;
        Ok(DbtSelectorExpansion {
            core: native_field(&parsed, "core")?.extract()?,
            upstream: native_field(&parsed, "upstream")?.is_truthy()?,
            downstream: native_field(&parsed, "downstream")?.is_truthy()?,
        })
    }

    fn model_names(&self, core: &str) -> PyResult<(Vec<String>, Option<String>)> {
        let py: Python<'_> = self.project.py();
        let (names, translated): (Bound<'_, PyAny>, Option<String>) =
            resolve_sqlbuild_model_selector_names(&self.project, &PyString::new(py, core))?
                .extract()?;
        Ok((texts(&names)?, translated))
    }
}

fn selection_error(py: Python<'_>, error: DbtSelectionError<PyErr>) -> PyErr {
    match error {
        DbtSelectionError::Resolver(error) => error,
        DbtSelectionError::PathSelector(message) => new_error(
            py,
            ErrorId::DbtInteropArgument,
            &message,
            (Some(PATH_SELECTOR_ERROR_CODE), None),
        ),
    }
}

fn anchors(mapping: &Bound<'_, PyAny>) -> PyResult<HashMap<String, Vec<String>>> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    if mapping.is_none() {
        return Ok(result);
    }
    for item in mapping_items(mapping)?.try_iter()? {
        let (term, unique_ids): (String, Bound<'_, PyAny>) = item?.extract()?;
        result.insert(term, texts(&unique_ids)?);
    }
    Ok(result)
}

fn manifest_models(manifest: &Bound<'_, PyAny>) -> PyResult<Option<Vec<(String, String)>>> {
    if manifest.is_none() {
        return Ok(None);
    }
    let models: Vec<(String, String)> = graph_manifest(manifest)?
        .models
        .into_iter()
        .map(|model| (model.unique_id, model.name))
        .collect();
    Ok(Some(models))
}

fn selection_result<'py>(py: Python<'py>, selection: DbtSelection) -> PyResult<Bound<'py, PyAny>> {
    let anchors_by_term: Bound<'py, PyDict> = PyDict::new(py);
    for (term, unique_ids) in selection.dbt_anchor_unique_ids_by_term {
        anchors_by_term.set_item(term, PyTuple::new(py, unique_ids)?)?;
    }
    let translations: Vec<Bound<'py, PyTuple>> = selection
        .path_translations
        .into_iter()
        .map(|(term, translated)| PyTuple::new(py, [term, translated]))
        .collect::<PyResult<_>>()?;
    new_record_with(
        py,
        RecordId::DbtInteropSelectionResult,
        &[
            (
                "sqlbuild_model_names",
                PyTuple::new(py, selection.sqlbuild_model_names)?.into_any(),
            ),
            (
                "dbt_required_unique_ids",
                PyTuple::new(py, selection.dbt_required_unique_ids)?.into_any(),
            ),
            (
                "dbt_anchor_terms",
                PyTuple::new(py, selection.dbt_anchor_terms)?.into_any(),
            ),
            ("dbt_anchor_unique_ids_by_term", anchors_by_term.into_any()),
            (
                "path_translations",
                PyTuple::new(py, translations)?.into_any(),
            ),
        ],
    )
}

/// Resolve selected SQLBuild models and required dbt nodes from `sqb dbt` selectors.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_dbt_interop_sqlbuild_selection<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_dbt_interop_sqlbuild_selection",
        options,
        SELECTION_OPTIONS,
    )?;
    let manifest_models: Option<Vec<(String, String)>> =
        manifest_models(&arguments.value("manifest")?)?;
    let anchor_unique_ids_by_term: HashMap<String, Vec<String>> =
        anchors(&arguments.value("dbt_anchor_unique_ids_by_term")?)?;
    let select: Vec<String> = texts(&arguments.value("select")?)?;
    let exclude_terms: Bound<'py, PyAny> = arguments.value("exclude")?;
    let exclude: Vec<String> = if exclude_terms.is_none() {
        Vec::new()
    } else {
        texts(&exclude_terms)?
    };
    let request: DbtSelectionRequest<'_> = DbtSelectionRequest {
        select: &select,
        exclude: &exclude,
        anchor_unique_ids_by_term: &anchor_unique_ids_by_term,
        manifest_models: manifest_models.as_deref(),
    };
    let graph: Bound<'py, DbtCombinedGraph> = arguments.value("graph")?.cast_into()?;
    let resolver: PlannerResolver<'py> = planner_resolver(arguments.value("project")?)?;
    let selection: DbtSelection = resolve_dbt_selection(&resolver, &graph.get().graph, &request)
        .map_err(|error| selection_error(py, error))?;
    selection_result(py, selection)
}

/// The exact dbt FQN selector when the node has an FQN, else `fallback`.
#[pyfunction]
#[pyo3(signature = (*, fqn, fallback))]
fn dbt_fqn_selector_term(fqn: &Bound<'_, PyAny>, fallback: &str) -> PyResult<String> {
    Ok(fqn_selector_term(&texts(fqn)?, fallback))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<DbtCombinedGraph>()?;
    module.add_function(wrap_pyfunction!(build_dbt_combined_graph, module)?)?;
    module.add_function(wrap_pyfunction!(
        resolve_dbt_interop_sqlbuild_selection,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(dbt_fqn_selector_term, module)?)
}
