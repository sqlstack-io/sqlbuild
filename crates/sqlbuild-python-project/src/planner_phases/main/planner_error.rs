//! `PlannerInputError(message, code=code, help=help)`.

use crate::planner_phases::main::optional_text::optional_text;
use pyo3::prelude::{PyAnyMethods, PyErr, Python};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// `PlannerInputError(message, code=code, help=help)`.
pub fn planner_error(
    py: Python<'_>,
    message: &str,
    (code, help): (Option<&str>, Option<&str>),
) -> PyErr {
    let raised = error_class(py, ErrorId::PlannerInput).and_then(|class| {
        let fields = keywords(
            py,
            &[
                ("code", optional_text(py, code)),
                ("help", optional_text(py, help)),
            ],
        )?;
        class.call((message,), Some(&fields))
    });
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}
