//! `request` with its project directory resolved to the working directory when unset.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;

/// `request` with its project directory resolved to the working directory when unset.
pub fn resolved_request<'py>(request: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if !native_field(request, "project_dir")?.is_none() {
        return Ok(request.clone());
    }
    let cwd: Bound<'py, PyAny> = imported(request.py(), "pathlib", "Path")?.call_method0("cwd")?;
    replaced(&request.clone(), &[("project_dir", cwd)])
}
