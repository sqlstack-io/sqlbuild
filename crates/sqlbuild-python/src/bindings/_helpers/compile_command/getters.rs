//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::compile_command::models::CompileReuseAttempt;
use crate::bindings::_helpers::compile_command::prepared_artifacts::PreparedCompileArtifacts;
use crate::bindings::_helpers::compile_command::reuse_recording::{
    CompileArtifactWrites, RecordedCompileOutput,
};
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<CompileReuseAttempt>,
        read: read_compile_reuse_attempt,
    },
    GetterEntry {
        class: class_of::<PreparedCompileArtifacts>,
        read: read_prepared_compile_artifacts,
    },
    GetterEntry {
        class: class_of::<CompileArtifactWrites>,
        read: read_compile_artifact_writes,
    },
    GetterEntry {
        class: class_of::<RecordedCompileOutput>,
        read: read_recorded_compile_output,
    },
];

fn read_compile_reuse_attempt<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<CompileReuseAttempt>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "outcome" => (&this.outcome).into_bound_py_any(py),
        "project_dir" => (&this.project_dir).into_bound_py_any(py),
        "started" => (&this.started).into_bound_py_any(py),
        "check_ms" => (&this.check_ms).into_bound_py_any(py),
        "exit_code" => (&this.exit_code).into_bound_py_any(py),
        "entry_path" => (&this.entry_path).into_bound_py_any(py),
        "native" => (&this.native).into_bound_py_any(py),
        "selected_target" => (&this.selected_target).into_bound_py_any(py),
        "json_output" => (&this.json_output).into_bound_py_any(py),
        _ => return None,
    })
}

fn read_prepared_compile_artifacts<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<PreparedCompileArtifacts>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "enabled" => this.enabled().into_bound_py_any(py),
        _ => return None,
    })
}

fn read_compile_artifact_writes<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<CompileArtifactWrites>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "artifacts" => this
            .artifacts(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_recorded_compile_output<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<RecordedCompileOutput>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "stderr_lines" => this.stderr_lines(py).into_bound_py_any(py),
        "stdout" => this.stdout(py).into_bound_py_any(py),
        _ => return None,
    })
}
