//! The selected target's name, record, database and schema, resolved from `connection` and the adapter's defaults without compiling resources.

use crate::compile_attachment::_helpers::target;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// The selected target's name, record, database and schema, resolved from `connection` and the adapter's defaults without compiling resources.
#[inline]
pub fn effective_target_namespace<'py>(
    (discovered_inputs, selected_target): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (adapter_name, connection, adapter): (&str, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyTuple>> {
    target::effective_target_namespace(
        (discovered_inputs, selected_target),
        (adapter_name, connection, adapter),
    )
}
