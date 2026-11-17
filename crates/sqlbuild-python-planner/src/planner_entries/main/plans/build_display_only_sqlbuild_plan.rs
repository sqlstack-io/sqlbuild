//! Build a display-only plan when warehouse planning cannot fully resolve.

use crate::planner_entries::_helpers::entries::values::planner_model;
use crate::planner_entries::_helpers::plans::display_plan::display_entry;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyFrozenSet, PyTuple};
use sqlbuild_analysis::graph::main::downstream_edges::downstream_edges;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::graph::main::project::edge_dict::edge_dict;
use sqlbuild_python_project::graph::models::KeyCache;
use sqlbuild_python_project::planner_identity::main::execution_deps::execution_deps;

/// Build a display-only plan when warehouse planning cannot fully resolve.
#[pyfunction]
#[pyo3(signature = (*, project, selected_model_names, full_refresh))]
pub fn build_display_only_sqlbuild_plan<'py>(
    project: &Bound<'py, PyAny>,
    selected_model_names: &Bound<'py, PyAny>,
    full_refresh: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let selected = PyFrozenSet::new(
        py,
        selected_model_names
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?,
    )?;
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in compiled_models(project)?.try_iter()? {
        let model = model?;
        if selected.contains(name_object(&model)?)? {
            entries.push(display_entry(&model, full_refresh)?);
        }
    }
    let keys: Vec<Bound<'py, PyAny>> = entries
        .iter()
        .map(|entry| native_field(entry, "key"))
        .collect::<PyResult<_>>()?;
    let upstream = execution_deps(project)?;
    let cache = KeyCache::new(py)?;
    let downstream = edge_dict(&cache, &downstream_edges(&upstream))?.into_any();
    let upstream = edge_dict(&cache, &upstream)?.into_any();
    planner_model(
        py,
        RecordId::PlanOutput,
        &[
            ("execution_order", PyTuple::new(py, &keys)?.into_any()),
            ("model_entries", PyTuple::new(py, entries)?.into_any()),
            ("selected_keys", PyFrozenSet::new(py, keys)?.into_any()),
            ("upstream_deps", upstream),
            ("downstream_deps", downstream),
        ],
    )
}
