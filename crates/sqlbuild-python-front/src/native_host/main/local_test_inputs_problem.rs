//! Why SQLQuack can't use a project's local test allowances or overrides, as the executor host checks.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// The problem SQLQuack finds with `allow` or the overrides under `project_dir`; `None` when it finds none.
pub fn local_test_inputs_problem(
    project_dir: &Bound<'_, PyAny>,
    allow: &Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    let py: Python<'_> = project_dir.py();
    call_with(
        &host_entry(py, ExecutorHost::CheckLocalTestInputs)?,
        &[
            ("project_dir", project_dir.clone()),
            ("allow", allow.clone()),
        ],
    )?
    .extract()
}
