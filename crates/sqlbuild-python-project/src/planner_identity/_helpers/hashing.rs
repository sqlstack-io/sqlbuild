//! Query and function definition fingerprints for change detection, failing as planner input errors.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;

use sqlbuild_analysis::planner_identity::constants::{FINGERPRINT_HELP, QUERY_FINGERPRINT_FAILED};
use sqlbuild_analysis::sql_tokens::main::baseline_fingerprint::baseline_fingerprint;
use sqlbuild_analysis::sql_tokens::main::compiler_query_fingerprint::compiler_query_fingerprint;
use sqlbuild_core::panics::main::is_compiler_panic::is_compiler_panic;

use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::types::CompilerDetach;

/// The query fingerprint of a model, or the planner input error naming the model.
pub(crate) fn model_query_hash(
    py: Python<'_>,
    model_name: &str,
    (query_sql, dialect): (&str, Option<&str>),
) -> PyResult<String> {
    py.compiler_detach(|| compiler_query_fingerprint(query_sql, dialect))
        .map_err(|error| fingerprint_error(py, &format!("Model '{model_name}' SQL"), error))
}

/// `current`, or the fingerprint recorded with a `Fingerprint` row whose definition fingerprints to `current`.
pub(crate) fn recorded_baseline(
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

pub(crate) fn fingerprint_error(py: Python<'_>, subject: &str, message: String) -> PyErr {
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
