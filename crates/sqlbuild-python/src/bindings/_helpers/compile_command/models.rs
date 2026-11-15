//! The per-invocation compile reuse state carried from the reuse check into a full compile.

use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{IntoPyObjectExt, pyclass, pymethods};

use crate::bindings::_helpers::compile_command::constants::REUSE_ATTEMPT_FIELDS;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Result of checking for a reusable compile, carried into a full compile on a miss.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub(crate) struct CompileReuseAttempt {
    /// `hit`, `miss` or `bypass`.
    pub(crate) outcome: Py<PyAny>,
    pub(crate) project_dir: Py<PyAny>,
    pub(crate) started: Py<PyAny>,
    pub(crate) check_ms: Py<PyAny>,
    pub(crate) exit_code: Py<PyAny>,
    pub(crate) entry_path: Py<PyAny>,
    /// The natively checked attempt, recorded natively after a miss.
    pub(crate) native: Py<PyAny>,
    pub(crate) selected_target: Py<PyAny>,
    pub(crate) json_output: Py<PyAny>,
}

#[pymethods]
impl CompileReuseAttempt {
    #[new]
    #[pyo3(signature = (**options))]
    fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let fields: KeywordArguments<'_> =
            KeywordArguments::read(py, "CompileReuseAttempt", options, REUSE_ATTEMPT_FIELDS)?;
        let field = |name: &str| -> PyResult<Py<PyAny>> { Ok(fields.value(name)?.unbind()) };
        Ok(Self {
            outcome: field("outcome")?,
            project_dir: field("project_dir")?,
            started: field("started")?,
            check_ms: if fields.passed("check_ms")? {
                field("check_ms")?
            } else {
                0_i64.into_py_any(py)?
            },
            exit_code: field("exit_code")?,
            entry_path: field("entry_path")?,
            native: field("native")?,
            selected_target: field("selected_target")?,
            json_output: fields.value_or_false("json_output")?.unbind(),
        })
    }
}
