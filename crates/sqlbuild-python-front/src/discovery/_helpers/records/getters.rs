//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::discovery::_helpers::snapshots::native_classes::NativeDirectorySnapshot;
use crate::discovery::models::NativeModelFile;

pub(crate) fn read_native_model_file<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeModelFile>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "file_path" => this
            .file_path(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "relative_path" => this
            .relative_path(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "contents" => this.contents(py).into_bound_py_any(py),
        "header_values" => this
            .header_values(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "header_column_locations" => this
            .header_column_locations(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "output_column_locations" => this
            .output_column_locations(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "query_sql" => this.query_sql(py).into_bound_py_any(py),
        "enum_declarations" => this.enum_declarations(py).into_bound_py_any(py),
        "constant_declarations" => this.constant_declarations(py).into_bound_py_any(py),
        "extract_implicit_alias_columns" => {
            this.extract_implicit_alias_columns().into_bound_py_any(py)
        }
        _ => return None,
    })
}

pub(crate) fn read_native_directory_snapshot<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeDirectorySnapshot>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "project_dir" => this.project_dir(py).into_bound_py_any(py),
        "memo" => this.memo(py).into_bound_py_any(py),
        _ => return None,
    })
}
