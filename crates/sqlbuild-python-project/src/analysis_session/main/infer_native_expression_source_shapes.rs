//! One shape per expression, cached on the profile's binding catalog; a failure raises.

use crate::analysis_session::_helpers::stage_functions::expression_source_shapes;
use pyo3::prelude::{Bound, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_python_base::boundary::models::Request;

/// One shape per expression, cached on the profile's binding catalog; a failure raises.
#[pyfunction]
#[pyo3(signature = (**request))]
pub fn infer_native_expression_source_shapes<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("infer_native_expression_source_shapes", request)?;
    expression_source_shapes(&request.get("expressions")?, &request.get("profile")?)
}
