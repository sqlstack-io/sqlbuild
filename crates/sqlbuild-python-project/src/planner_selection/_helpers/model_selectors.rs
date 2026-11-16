//! `+` marker splitting and SQLBuild-owned model selectors for dbt interop.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult};
use pyo3::types::{PyList, PyTuple};
use sqlbuild_analysis::planner_selection::models::SelectableModel;

use crate::planner_selection::models::Carrier;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;

const TAGS_KEY: &str = "tags";

const SHARED_EXCEPTIONS_MODULE: &str = "sqlbuild.errors.contracts.exceptions";

pub(crate) fn shared_input_error(carrier: &Carrier<'_>, message: &str) -> PyErr {
    let raised = imported(carrier.py(), SHARED_EXCEPTIONS_MODULE, "SharedInputError")
        .and_then(|class| class.call1((carrier.back(message)?,)));
    match raised {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}

fn model_tags(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let tags: Bound<'_, PyAny> = config_value(model.py(), &model_values(model)?, TAGS_KEY)?;
    if !tags.is_instance_of::<PyTuple>() && !tags.is_instance_of::<PyList>() {
        return Ok(Vec::new());
    }
    tags.try_iter()?.map(|tag| tag?.str()?.extract()).collect()
}

pub(crate) fn selectable_model(model: &Bound<'_, PyAny>) -> PyResult<SelectableModel> {
    Ok(SelectableModel {
        name: native_field(model, "name")?.extract()?,
        tags: model_tags(model)?,
        folder: native_field(&native_field(model, "relative_path")?, "parent")?
            .call_method0("as_posix")?
            .extract()?,
    })
}
