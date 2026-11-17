//! Whether two source freshness records are equivalent for skip decisions.

use crate::state_rows::_helpers::event_rows::freshness::StateTimestamps;
use crate::state_rows::_helpers::event_rows::freshness::comparable;
use crate::state_rows::_helpers::event_rows::freshness::python_error;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_state::freshness::main::records_equivalent::records_equivalent;
use std::cell::RefCell;

/// Whether two source freshness records are equivalent for skip decisions.
#[pyfunction(name = "source_freshness_records_equivalent")]
#[pyo3(signature = (*, previous_record, current_record, lag_tolerance = None))]
pub fn equivalent(
    previous_record: &Bound<'_, PyAny>,
    current_record: &Bound<'_, PyAny>,
    lag_tolerance: Option<&str>,
) -> PyResult<bool> {
    let py: Python<'_> = previous_record.py();
    let parser = StateTimestamps {
        py,
        failure: RefCell::new(None),
    };
    let records = (&comparable(previous_record)?, &comparable(current_record)?);
    records_equivalent(records, lag_tolerance, &parser).map_err(|error| {
        parser
            .failure
            .borrow_mut()
            .take()
            .unwrap_or_else(|| python_error(py, error))
    })
}
