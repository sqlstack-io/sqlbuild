//! Return whether a planned build has runtime work to execute.

use crate::compile_driver::_helpers::setup::pipeline_entries::PLAN_WORK_ENTRIES;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Return whether a planned build has runtime work to execute.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn plan_has_executable_work<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<bool> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "plan_has_executable_work",
        options,
        (&["plan"], &["python_plan_entries"]),
    )?;
    let plan: Bound<'py, PyAny> = arguments.value("plan")?;
    for name in PLAN_WORK_ENTRIES {
        if native_field(&plan, name)?.is_truthy()? {
            return Ok(true);
        }
    }
    if !arguments.passed("python_plan_entries")? {
        return Ok(false);
    }
    arguments.value("python_plan_entries")?.is_truthy()
}
