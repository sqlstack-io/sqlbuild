//! `build_python_sql_run_lifecycle(*, selection, python_graph)`: a `PythonSqlRunLifecyclePlan`.

use crate::graph::_helpers::python_nodes::python_lifecycle;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `build_python_sql_run_lifecycle(*, selection, python_graph)`: a `PythonSqlRunLifecyclePlan`.
#[inline]
pub fn build_python_sql_run_lifecycle<'py>(
    selection: &Bound<'py, PyAny>,
    python_graph: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    python_lifecycle::build_python_sql_run_lifecycle(selection, python_graph)
}
