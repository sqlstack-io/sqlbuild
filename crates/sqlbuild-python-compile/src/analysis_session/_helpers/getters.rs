//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use sqlbuild_python_project::analysis_session::models::NativeModelAnalysisSession;

pub(crate) fn read_native_model_analysis_session<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeModelAnalysisSession>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "fact_models" => this.fact_models().into_bound_py_any(py),
        "cache_stats" => this.cache_stats().into_bound_py_any(py),
        "sharing" => this.sharing().into_bound_py_any(py),
        _ => return None,
    })
}
