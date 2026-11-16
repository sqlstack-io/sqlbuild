//! The active command's warehouse group, `--warehouse` override and CLI vars.

use sqlbuild_python_base::boundary::main::context_variable::context_variable as shared_context_variable;
use std::sync::Mutex;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyTuple};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};

use sqlbuild_config::command_connection::constants::COMMAND_WAREHOUSE_GROUPS;

use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

const CONTEXT_ATTRIBUTE: &str = "_command_warehouse_scope_context";
const CONTEXT_NAME: &str = "sqlbuild_command_warehouse_scope";

/// The `ContextVar` holding the active command's warehouse scope.
static CONTEXT: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

fn context_variable(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    shared_context_variable(py, &CONTEXT, CONTEXT_NAME)
}

/// Installs one command's warehouse scope for the duration of a `with` block.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct CommandWarehouseScopeInstall {
    scope: Py<PyAny>,
    token: Mutex<Option<Py<PyAny>>>,
}

#[pymethods]
impl CommandWarehouseScopeInstall {
    fn __enter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let token: Bound<'_, PyAny> = context_variable(py)?.call_method1("set", (&self.scope,))?;
        *self
            .token
            .lock()
            .map_err(|_| PyRuntimeError::new_err("warehouse scope lock is poisoned"))? =
            Some(token.unbind());
        Ok(self.scope.clone_ref(py))
    }

    #[pyo3(signature = (*_exc_info))]
    fn __exit__(&self, py: Python<'_>, _exc_info: &Bound<'_, PyTuple>) -> PyResult<bool> {
        let token: Option<Py<PyAny>> = self
            .token
            .lock()
            .map_err(|_| PyRuntimeError::new_err("warehouse scope lock is poisoned"))?
            .take();
        if let Some(token) = token {
            context_variable(py)?.call_method1("reset", (token,))?;
        }
        Ok(false)
    }
}

/// Install the warehouse group, CLI override, and CLI vars for one command invocation.
#[pyfunction]
#[pyo3(signature = (*, command, cli_warehouse, cli_vars = None))]
pub(crate) fn command_warehouse_scope<'py>(
    command: &Bound<'py, PyAny>,
    cli_warehouse: &Bound<'py, PyAny>,
    cli_vars: Option<&Bound<'py, PyAny>>,
) -> PyResult<CommandWarehouseScopeInstall> {
    let py: Python<'py> = command.py();
    let group: Bound<'py, PyAny> = if command.is_none() {
        py.None().into_bound(py)
    } else {
        let name: String = enum_from_value(py, RecordId::CliCommand, command)?
            .str()?
            .to_string();
        match COMMAND_WAREHOUSE_GROUPS
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .and_then(|(_, group)| *group)
        {
            Some(group) => enum_from_value(py, RecordId::WarehouseGroup, group)?,
            None => py.None().into_bound(py),
        }
    };
    let vars: Bound<'py, PyAny> = match cli_vars.filter(|cli_vars| !cli_vars.is_none()) {
        Some(cli_vars) => py.get_type::<PyDict>().call1((cli_vars,))?,
        None => PyDict::new(py).into_any(),
    };
    let scope: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CommandWarehouseScope,
        &[
            ("group", group),
            ("cli_warehouse", cli_warehouse.clone()),
            ("cli_vars", vars),
        ],
    )?;
    Ok(CommandWarehouseScopeInstall {
        scope: scope.unbind(),
        token: Mutex::new(None),
    })
}

/// Return the active command's warehouse scope, or an empty one outside the CLI.
#[pyfunction]
pub(crate) fn current_command_warehouse_scope(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let scope: Bound<'_, PyAny> = context_variable(py)?.call_method0("get")?;
    if !scope.is_none() {
        return Ok(scope);
    }
    new_record_with(
        py,
        RecordId::CommandWarehouseScope,
        &[("group", py.None().into_bound(py))],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py: Python<'_> = module.py();
    module.add(CONTEXT_ATTRIBUTE, context_variable(py)?)?;
    module.add_class::<CommandWarehouseScopeInstall>()?;
    module.add_function(wrap_pyfunction!(command_warehouse_scope, module)?)?;
    module.add_function(wrap_pyfunction!(current_command_warehouse_scope, module)?)
}
