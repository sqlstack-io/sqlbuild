//! Plan fixes without exposing any unverified intermediate source on disk.

use crate::rules::_helpers::fix::fix_session::FixSession;
use crate::rules::_helpers::fix::fix_session::result;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// Plan fixes without exposing any unverified intermediate source on disk.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn plan_rule_fixes<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "plan_rule_fixes",
        options,
        (
            &[
                "project_dir",
                "files",
                "config",
                "adapter",
                "discovered_inputs",
                "enabled",
            ],
            &[],
        ),
    )?;
    let files = args.value("files")?.cast_into::<PyDict>()?;
    if !args.flag("enabled")? {
        return result(&files, &[], &[]);
    }
    let adapter = args.value("adapter")?;
    let inputs = args.value("discovered_inputs")?;
    if !adapter.is_instance(&host_entry(py, AdapterHost::BaseAdapter)?)? || inputs.is_none() {
        return Err(PyErr::from_value(
            error_class(py, ErrorId::ProjectCompile)?
                .call1(("format --fix requires discovered compiler inputs and an adapter",))?,
        ));
    }
    FixSession {
        project_dir: args.value("project_dir")?,
        original: files,
        config: args.value("config")?,
        adapter,
        inputs,
        reports: Vec::new(),
        blocked: PyDict::new(py),
        skipped: PyDict::new(py),
        baseline: None,
        baseline_facts: None,
    }
    .run()
}
