//! Adapter resolution for CLI commands, with adapter configuration failures as `C601`.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::build_command::constants::PLAN_SYNCED_COPY_ENV;
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::ErrorId;

const ADAPTER_CONFIGURATION_CODE: &str = "C601";

/// `CliUserError(error.message, code="C601", help=error.help)` raised from `error`.
fn cli_adapter_error(py: Python<'_>, error: &PyErr) -> PyResult<PyErr> {
    let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("code", ADAPTER_CONFIGURATION_CODE)?;
    keywords.set_item("help", native_field(&value, "help")?)?;
    let raised: PyErr = PyErr::from_value(
        error_class(py, ErrorId::CliUser)?
            .call((native_field(&value, "message")?,), Some(&keywords))?,
    );
    raised.set_cause(py, Some(error.clone_ref(py)));
    Ok(raised)
}

/// The registered adapter for `adapter_name`, resolved through the adapter registry.
pub(crate) fn resolve_cli_adapter<'py>(
    adapter_name: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = adapter_name.py();
    let resolved: PyResult<Bound<'py, PyAny>> = call_with(
        &host::entry(py, AdapterHost::ResolveAdapter)?,
        &[
            ("adapter_name", adapter_name.clone()),
            ("project_dir", project_dir.clone()),
        ],
    );
    let adapter: Bound<'py, PyAny> = match resolved {
        Err(error) if error.is_instance(py, &host::entry(py, AdapterHost::AdapterUserError)?) => {
            return Err(cli_adapter_error(py, &error)?);
        }
        resolved => resolved?,
    };
    let Some(synced_copy) = std::env::var_os(PLAN_SYNCED_COPY_ENV) else {
        return Ok(adapter);
    };
    call_with(
        &host::entry(py, AdapterHost::SyncedCopyAdapter)?,
        &[
            ("adapter", adapter),
            (
                "path",
                imported(py, "pathlib", "Path")?.call1((synced_copy.to_string_lossy(),))?,
            ),
        ],
    )
}

/// Resolve an adapter name from project config to an adapter instance.
#[pyfunction]
#[pyo3(signature = (*, adapter_name, project_dir = None))]
fn resolve_adapter<'py>(
    adapter_name: &Bound<'py, PyAny>,
    project_dir: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let project_dir: Bound<'py, PyAny> =
        project_dir.unwrap_or_else(|| adapter_name.py().None().into_bound(adapter_name.py()));
    resolve_cli_adapter(adapter_name, &project_dir)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_adapter, module)?)
}
