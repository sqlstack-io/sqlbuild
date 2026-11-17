//! `warnings` followed by the future-end and automatic-start warnings `bounds` carries.

use crate::planner_resolution::_helpers::cursor::cursor_warnings::FUTURE_CAPPED_CODE;
use crate::planner_resolution::_helpers::cursor::cursor_warnings::START_CAPPED_CODE;
use crate::planner_resolution::_helpers::cursor::cursor_warnings::future_cap_message;
use crate::planner_resolution::_helpers::cursor::cursor_warnings::maximum_start_message;
use crate::planner_resolution::_helpers::references::values::string;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyTuple;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// `warnings` followed by the future-end and automatic-start warnings `bounds` carries.
pub fn with_cursor_safety_warnings<'py>(
    warnings: &Bound<'py, PyAny>,
    model_name: &Bound<'py, PyAny>,
    bounds: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = warnings.py();
    let mut all: Vec<Bound<'py, PyAny>> = warnings
        .try_iter()?
        .collect::<PyResult<Vec<Bound<'py, PyAny>>>>()?;
    for (message, code) in [
        (future_cap_message(bounds)?, FUTURE_CAPPED_CODE),
        (maximum_start_message(bounds)?, START_CAPPED_CODE),
    ] {
        let Some(message) = message else { continue };
        all.push(new_record_with(
            py,
            RecordId::PlanWarning,
            &[
                ("model_name", model_name.clone()),
                (
                    "severity",
                    enum_member(py, RecordId::WarningSeverity, "WARNING")?,
                ),
                ("code", string(py, code)),
                ("message", string(py, &message)),
            ],
        )?);
    }
    Ok(PyTuple::new(py, all)?.into_any())
}
