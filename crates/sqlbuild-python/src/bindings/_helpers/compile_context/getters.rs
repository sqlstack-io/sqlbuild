//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::compile_context::diagnostic_collector::CollectedCompileDiagnostics;
use crate::bindings::_helpers::compile_context::input_reads::CompileInputReads;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<CollectedCompileDiagnostics>,
        read: read_collected_compile_diagnostics,
    },
    GetterEntry {
        class: class_of::<CompileInputReads>,
        read: read_compile_input_reads,
    },
];

fn read_collected_compile_diagnostics<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<CollectedCompileDiagnostics>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "diagnostics" => this
            .diagnostics(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_compile_input_reads<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<CompileInputReads>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "environment_names" => this
            .environment_names(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "settings_classes" => this
            .settings_classes(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "read_run_id" => this.read_run_id().into_bound_py_any(py),
        _ => return None,
    })
}
