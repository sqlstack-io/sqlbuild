//! One SQLBuild-owned model selector core's model names, and its path selector when translated.

use crate::compiled_views::main::project::compiled_models::compiled_models;
use crate::planner_selection::_helpers::model_selectors::selectable_model;
use crate::planner_selection::models::Carrier;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_analysis::planner_selection::main::sqlbuild_model_selector_names::sqlbuild_model_selector_names;
use sqlbuild_analysis::planner_selection::models::{ModelSelectorNames, SelectableModel};

/// One SQLBuild-owned model selector core's model names, and its path selector when translated.
pub fn resolve_sqlbuild_model_selector_names<'py>(
    project: &Bound<'py, PyAny>,
    term: &Bound<'py, PyString>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project.py();
    let models: Vec<SelectableModel> = compiled_models(project)?
        .try_iter()?
        .map(|model| selectable_model(&model?))
        .collect::<PyResult<_>>()?;
    let carrier: Carrier<'py> = Carrier::new(py, std::slice::from_ref(term), || {
        Ok(models
            .iter()
            .flat_map(|model| [model.name.clone(), model.folder.clone()])
            .chain(models.iter().flat_map(|model| model.tags.clone()))
            .collect())
    })?;
    let resolved: ModelSelectorNames = sqlbuild_model_selector_names(&models, &carrier.send(term)?);
    let translated: Option<Bound<'py, PyString>> = resolved
        .translated
        .as_deref()
        .map(|path| carrier.back(path))
        .transpose()?;
    PyTuple::new(
        py,
        [
            PyTuple::new(py, resolved.names)?.into_any(),
            translated.into_pyobject(py)?.into_any(),
        ],
    )
}
