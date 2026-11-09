//! Query and function definition fingerprints for change detection, failing as planner input errors.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::PyString;
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_analysis::planner_identity::constants::{FINGERPRINT_HELP, QUERY_FINGERPRINT_FAILED};
use sqlbuild_analysis::planner_identity::models::GraphKey;
use sqlbuild_analysis::sql_tokens::main::baseline_fingerprint::baseline_fingerprint;
use sqlbuild_analysis::sql_tokens::main::compiler_query_fingerprint::compiler_query_fingerprint;
use sqlbuild_analysis::sql_tokens::main::definition_fingerprint::definition_fingerprint;
use sqlbuild_core::panics::main::is_compiler_panic::is_compiler_panic;

use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::ErrorId;
use crate::bindings::types::CompilerDetach;

/// The query fingerprint of a model, or the planner input error naming the model.
pub(crate) fn model_query_hash(
    py: Python<'_>,
    model_name: &str,
    (query_sql, dialect): (&str, Option<&str>),
) -> PyResult<String> {
    py.compiler_detach(|| compiler_query_fingerprint(query_sql, dialect))
        .map_err(|error| fingerprint_error(py, &format!("Model '{model_name}' SQL"), error))
}

/// The model's query fingerprint, or its recorded one while the recorded SQL still fingerprints the same.
pub(crate) fn baselined_model_query_hash(
    py: Python<'_>,
    model_name: &str,
    (query_sql, dialect): (&str, Option<&str>),
    recorded: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let current: String = model_query_hash(py, model_name, (query_sql, dialect))?;
    recorded_baseline(py, current, recorded, |sql| {
        compiler_query_fingerprint(sql, dialect)
    })
}

/// The function's definition fingerprint, or its recorded one while the recorded definition still fingerprints the same.
pub(crate) fn baselined_function_definition_hash(
    py: Python<'_>,
    function_name: &str,
    (fingerprint_sql, language, dialect): (&str, &str, Option<&str>),
    recorded: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let current: String =
        function_definition_hash(py, function_name, (fingerprint_sql, language, dialect))?;
    recorded_baseline(py, current, recorded, |sql| {
        definition_fingerprint(sql, language, dialect)
    })
}

/// `current`, or the fingerprint recorded with a `Fingerprint` row whose definition fingerprints to `current`.
fn recorded_baseline(
    py: Python<'_>,
    current: String,
    recorded: &Bound<'_, PyAny>,
    fingerprint: impl FnOnce(&str) -> Result<String, String> + Send,
) -> PyResult<String> {
    if recorded.is_none() {
        return Ok(current);
    }
    let definition: String = native_field(recorded, "definition")?.extract()?;
    let recorded_hash: String = native_field(recorded, "definition_hash")?.extract()?;
    py.compiler_detach(|| {
        Ok(baseline_fingerprint(
            current,
            (&definition, &recorded_hash),
            fingerprint,
        ))
    })
    .map_err(compiler_error)
}

/// The definition fingerprint of a function, or the planner input error naming the function.
pub(crate) fn function_definition_hash(
    py: Python<'_>,
    function_name: &str,
    (fingerprint_sql, language, dialect): (&str, &str, Option<&str>),
) -> PyResult<String> {
    py.compiler_detach(|| definition_fingerprint(fingerprint_sql, language, dialect))
        .map_err(|error| fingerprint_error(py, &format!("Function '{function_name}'"), error))
}

fn fingerprint_error(py: Python<'_>, subject: &str, message: String) -> PyErr {
    if is_compiler_panic(&message) {
        return compiler_error(message);
    }
    let text = |value: &str| PyString::new(py, value).into_any();
    let raised: PyResult<Bound<'_, PyAny>> =
        error_class(py, ErrorId::PlannerInput).and_then(|class| {
            class.call(
                (format!(
                    "{subject} cannot be fingerprinted for change detection: {message}"
                ),),
                Some(&keywords(
                    py,
                    &[
                        ("code", text(QUERY_FINGERPRINT_FAILED)),
                        ("help", text(FINGERPRINT_HELP)),
                    ],
                )?),
            )
        });
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// The graph key of a `CompiledObjectKey`-like value: its resource type text and name.
pub(crate) fn graph_key(key: &Bound<'_, PyAny>) -> PyResult<GraphKey> {
    Ok(GraphKey {
        kind: native_field(key, "resource_type")?.str()?.extract()?,
        name: native_field(key, "name")?.extract()?,
    })
}

/// Fingerprint a model query for change detection, naming the model when it cannot.
#[pyfunction]
#[pyo3(signature = (*, model_name, query_sql, dialect))]
pub(crate) fn model_definition_hash(
    py: Python<'_>,
    model_name: &str,
    query_sql: &str,
    dialect: Option<&str>,
) -> PyResult<String> {
    model_query_hash(py, model_name, (query_sql, dialect))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(model_definition_hash, module)?)
}
