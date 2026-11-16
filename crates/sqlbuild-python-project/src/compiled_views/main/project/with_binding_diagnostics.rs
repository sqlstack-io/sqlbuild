//! The model with these final binding diagnostics.

use crate::compiled_views::main::project::replace::replace;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::types::RecordId;

/// The model with these final binding diagnostics.
#[pyfunction]
#[pyo3(signature = (*, model, diagnostics))]
pub fn with_binding_diagnostics<'py>(
    model: &Bound<'py, PyAny>,
    diagnostics: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if model.is_instance(&record_class(py, RecordId::CompiledModel)?)? {
        return replace(model, &[("binding_diagnostics", diagnostics.clone())]);
    }
    model.call_method1("with_binding_diagnostics", (diagnostics,))
}
