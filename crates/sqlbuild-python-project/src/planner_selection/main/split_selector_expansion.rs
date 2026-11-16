//! Split `+selector+` marker syntax from the selector core without resolving the core.

use crate::planner_selection::_helpers::model_selectors::shared_input_error;
use crate::planner_selection::models::Carrier;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyBool;
use pyo3::types::PyString;
use pyo3::{IntoPyObject, pyfunction};
use sqlbuild_analysis::planner_selection::main::split_selector_expansion::split_selector_expansion as split;
use sqlbuild_analysis::planner_selection::models::SelectorExpansion;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// Split `+selector+` marker syntax from the selector core without resolving the core.
#[pyfunction]
pub fn split_selector_expansion<'py>(raw: &Bound<'py, PyString>) -> PyResult<Bound<'py, PyAny>> {
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
