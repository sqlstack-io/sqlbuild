//! Index Python compiler fact views for native Rule-fix verification.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};

use crate::rules::_helpers::fact::fact_replay::project_attribute;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as record_field;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Index a compilation by model key and diagnostic owner without changing its fact views.
#[pyfunction]
#[pyo3(signature = (*, project, project_dir))]
pub(crate) fn compile_facts<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let models = PyDict::new(py);
    let model_paths = PyDict::new(py);
    let paths_by_name = PyDict::new(py);
    for model in native_field(project, "models")?.try_iter()? {
        let model = model?;
        let key = native_field(&model, "key")?;
        let path = project_dir
            .call_method1("__truediv__", (native_field(&model, "relative_path")?,))?
            .call_method0("resolve")?;
        models.set_item(&key, &model)?;
        model_paths.set_item(&key, &path)?;
        paths_by_name.set_item(native_field(&model, "name")?, &path)?;
    }
    let diagnostics = PyDict::new(py);
    let counter = py.import("collections")?.getattr("Counter")?;
    for diagnostic in project_attribute(project, "diagnostics")?.try_iter()? {
        let diagnostic = diagnostic?;
        let owner = diagnostic_owner(&diagnostic, &paths_by_name, project_dir)?;
        let counts = if let Some(counts) = diagnostics.get_item(&owner)? {
            counts
        } else {
            let counts = counter.call0()?;
            diagnostics.set_item(&owner, &counts)?;
            counts
        };
        let identity = PyTuple::new(
            py,
            [
                record_field(&diagnostic, "code")?,
                text_or_empty(&record_field(&diagnostic, "resource_type")?)?,
                text_or_empty(&record_field(&diagnostic, "resource_name")?)?,
                record_field(&diagnostic, "message")?,
            ],
        )?;
        let count: i64 = counts.get_item(&identity)?.extract()?;
        counts.set_item(identity, count + 1)?;
    }
    new_record_with(
        py,
        RecordId::CompileFacts,
        &[
            ("models", models.into_any()),
            ("model_paths", model_paths.into_any()),
            ("diagnostics", diagnostics.into_any()),
        ],
    )
}

fn diagnostic_owner<'py>(
    diagnostic: &Bound<'py, PyAny>,
    paths_by_name: &Bound<'py, PyDict>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let resource_type = record_field(diagnostic, "resource_type")?;
    if resource_type.eq("model")?
        && let Some(path) = paths_by_name.get_item(record_field(diagnostic, "resource_name")?)?
    {
        return Ok(path);
    }
    let mut path = record_field(diagnostic, "path")?;
    if !path.is_truthy()? {
        let location = record_field(diagnostic, "location")?;
        path = if location.is_none() {
            location
        } else {
            record_field(&location, "path")?
        };
    }
    if path.is_none() {
        project_dir.call_method0("resolve")
    } else {
        project_dir
            .call_method1("__truediv__", (path,))?
            .call_method0("resolve")
    }
}

fn text_or_empty<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_truthy()? {
        Ok(value.str()?.into_any())
    } else {
        Ok(pyo3::types::PyString::new(value.py(), "").into_any())
    }
}
