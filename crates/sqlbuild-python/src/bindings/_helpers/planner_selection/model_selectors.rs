//! `+` marker splitting and SQLBuild-owned model selectors for dbt interop.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::PyBool;
use pyo3::types::{PyList, PyString, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_analysis::planner_selection::main::split_selector_expansion::split_selector_expansion as split;
use sqlbuild_analysis::planner_selection::main::sqlbuild_model_selector_names::sqlbuild_model_selector_names;
use sqlbuild_analysis::planner_selection::models::{
    ModelSelectorNames, SelectableModel, SelectorExpansion,
};

use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::model_config::typed_config::{config_value, model_values};
use crate::bindings::_helpers::planner_selection::carrier::Carrier;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const TAGS_KEY: &str = "tags";

const SHARED_EXCEPTIONS_MODULE: &str = "sqlbuild.errors.contracts.exceptions";

fn shared_input_error(carrier: &Carrier<'_>, message: &str) -> PyErr {
    let raised = imported(carrier.py(), SHARED_EXCEPTIONS_MODULE, "SharedInputError")
        .and_then(|class| class.call1((carrier.back(message)?,)));
    match raised {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}

/// Split `+selector+` marker syntax from the selector core without resolving the core.
#[pyfunction]
pub(crate) fn split_selector_expansion<'py>(
    raw: &Bound<'py, PyString>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    let carrier: Carrier<'py> = Carrier::single(raw)?;
    let expansion: SelectorExpansion =
        split(&carrier.send(raw)?).map_err(|message| shared_input_error(&carrier, &message))?;
    new_record_with(
        py,
        RecordId::SelectorExpansion,
        &[
            ("core", carrier.back(&expansion.core)?.into_any()),
            (
                "upstream",
                PyBool::new(py, expansion.upstream).to_owned().into_any(),
            ),
            (
                "downstream",
                expansion
                    .downstream
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any(),
            ),
        ],
    )
}

fn model_tags(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let tags: Bound<'_, PyAny> = config_value(model.py(), &model_values(model)?, TAGS_KEY)?;
    if !tags.is_instance_of::<PyTuple>() && !tags.is_instance_of::<PyList>() {
        return Ok(Vec::new());
    }
    tags.try_iter()?.map(|tag| tag?.str()?.extract()).collect()
}

fn selectable_model(model: &Bound<'_, PyAny>) -> PyResult<SelectableModel> {
    Ok(SelectableModel {
        name: native_field(model, "name")?.extract()?,
        tags: model_tags(model)?,
        folder: native_field(&native_field(model, "relative_path")?, "parent")?
            .call_method0("as_posix")?
            .extract()?,
    })
}

/// One SQLBuild-owned model selector core's model names, and its path selector when translated.
pub(crate) fn resolve_sqlbuild_model_selector_names<'py>(
    project: &Bound<'py, PyAny>,
    term: &Bound<'py, PyString>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project.py();
    let models: Vec<SelectableModel> = project_reads::models(project)?
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(split_selector_expansion, module)?)?;
    Ok(())
}
