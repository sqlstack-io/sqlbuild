//! A hand-built model's binding diagnostics at their authored locations.

use crate::analysis::main::locate_model_binding_diagnostics::locate_model_binding_diagnostics;
use crate::analysis::models::LocatedRow;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;
use sqlbuild_analysis::binding_locations::models::LocatedDiagnostic;

use sqlbuild_python_project::analysis::models::Positional;

pub(crate) fn located_row(item: LocatedDiagnostic) -> LocatedRow {
    LocatedRow {
        code: item.code,
        message: item.message,
        severity: item.severity,
        line: item.line,
        column: item.column,
        span: item.location.map(Positional),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(locate_model_binding_diagnostics, module)?)?;
    Ok(())
}
