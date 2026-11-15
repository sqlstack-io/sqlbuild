//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::diagnostics::trackers::BuildPhaseTimingTracker;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<BuildPhaseTimingTracker>,
    read: read_build_phase_timing_tracker,
}];

fn read_build_phase_timing_tracker<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<BuildPhaseTimingTracker>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "compile_seconds" => (&this.compile_seconds).into_bound_py_any(py),
        "planning_seconds" => (&this.planning_seconds).into_bound_py_any(py),
        "connection_preparation_seconds" => {
            (&this.connection_preparation_seconds).into_bound_py_any(py)
        }
        "schema_preparation_seconds" => (&this.schema_preparation_seconds).into_bound_py_any(py),
        "execution_seconds" => (&this.execution_seconds).into_bound_py_any(py),
        "cost_collection_seconds" => (&this.cost_collection_seconds).into_bound_py_any(py),
        _ => return None,
    })
}
