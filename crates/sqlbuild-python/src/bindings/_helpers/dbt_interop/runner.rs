//! The dbt CLI runner: captured dbt commands with operation lifecycle events and `dbt ls` memo.

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

use pyo3::exceptions::{PyOSError, PyRuntimeError};
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::sync::MutexExt;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{pyclass, pymethods};
use sqlbuild_core::dbt_cli::models::DbtArgvCommand;
use sqlbuild_core::dbt_process::main::run_dbt_process::run_dbt_process;
use sqlbuild_core::dbt_process::models::DbtProcessOutput;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::dbt_interop::cli::{
    argv, ls_nodes, resolve_dbt_executable, sequence,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;

const LAUNCH_FAILURE: &str = "failed to execute dbt";
const POISONED_CACHE: &str = "dbt ls cache is poisoned";

/// Small dbt CLI runner with per-invocation `dbt ls` memoization.
#[pyclass(module = "sqlbuild._native", frozen, subclass)]
pub(crate) struct DbtRunner {
    #[pyo3(get)]
    pub(crate) dbt_executable: String,
    #[pyo3(get)]
    pub(crate) invoker: Option<Py<PyAny>>,
    ls_cache: Mutex<HashMap<Vec<String>, Py<PyAny>>>,
}

#[pymethods]
impl DbtRunner {
    #[new]
    #[pyo3(signature = (*, dbt_executable = None, invoker = None))]
    pub(crate) fn new(dbt_executable: Option<String>, invoker: Option<Py<PyAny>>) -> Self {
        Self {
            dbt_executable: dbt_executable
                .filter(|value| !value.is_empty())
                .unwrap_or_else(resolve_dbt_executable),
            invoker,
            ls_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Run dbt compile.
    #[pyo3(signature = (*, options, full_refresh = false))]
    fn compile<'py>(
        &self,
        options: &Bound<'py, PyAny>,
        full_refresh: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let command: Bound<'py, PyTuple> = argv(
            options.py(),
            (&self.dbt_executable, options),
            DbtArgvCommand::Compile { full_refresh },
        )?;
        self.run(command, &native_field(options, "project_dir")?)
    }

    /// Run dbt deps.
    #[pyo3(signature = (*, options))]
    fn deps<'py>(&self, options: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let command: Bound<'py, PyTuple> = argv(
            options.py(),
            (&self.dbt_executable, options),
            DbtArgvCommand::Deps,
        )?;
        self.run(command, &native_field(options, "project_dir")?)
    }

    /// Run dbt debug.
    #[pyo3(signature = (*, options, args = None))]
    fn debug<'py>(
        &self,
        options: &Bound<'py, PyAny>,
        args: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let args: Vec<String> = args.map_or_else(|| Ok(Vec::new()), sequence)?;
        let command: Bound<'py, PyTuple> = argv(
            options.py(),
            (&self.dbt_executable, options),
            DbtArgvCommand::Debug { args },
        )?;
        self.run(command, &native_field(options, "project_dir")?)
    }

    /// Run dbt ls and parse selected nodes, reusing an earlier identical listing.
    #[pyo3(signature = (*, options, select = None, exclude = None, resource_types = None))]
    fn ls<'py>(
        &self,
        options: &Bound<'py, PyAny>,
        select: Option<&Bound<'py, PyAny>>,
        exclude: Option<&Bound<'py, PyAny>>,
        resource_types: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = options.py();
        let listed = |values: Option<&Bound<'py, PyAny>>| -> PyResult<Vec<String>> {
            values.map_or_else(|| Ok(Vec::new()), sequence)
        };
        let command: Bound<'py, PyTuple> = argv(
            py,
            (&self.dbt_executable, options),
            DbtArgvCommand::Ls {
                select: listed(select)?,
                exclude: listed(exclude)?,
                resource_types: listed(resource_types)?,
            },
        )?;
        let key: Vec<String> = command.extract()?;
        if let Some(cached) = self.cached_listing(py, &key)? {
            return Ok(cached);
        }
        let result: Bound<'py, PyAny> =
            self.run(command, &native_field(options, "project_dir")?)?;
        let stdout: String = native_field(&result, "stdout")?.extract()?;
        let listing: Bound<'py, PyAny> = new_record_with(
            py,
            RecordId::DbtLsResult,
            &[
                ("nodes", ls_nodes(py, &stdout)?.into_any()),
                ("command", result),
            ],
        )?;
        self.ls_cache
            .lock_py_attached(py)
            .map_err(|_| PyRuntimeError::new_err(POISONED_CACHE))?
            .insert(key, listing.clone().unbind());
        Ok(listing)
    }

    /// Run an explicit dbt argv.
    #[pyo3(signature = (*, argv, cwd))]
    fn invoke<'py>(
        &self,
        argv: &Bound<'py, PyAny>,
        cwd: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let command: Bound<'py, PyTuple> =
            PyTuple::new(argv.py(), argv.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        self.run(command, cwd)
    }
}

impl DbtRunner {
    fn cached_listing<'py>(
        &self,
        py: Python<'py>,
        key: &[String],
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let cache = self
            .ls_cache
            .lock_py_attached(py)
            .map_err(|_| PyRuntimeError::new_err(POISONED_CACHE))?;
        Ok(cache
            .get(key)
            .map(|value| value.clone_ref(py).into_bound(py)))
    }

    fn run<'py>(
        &self,
        command: Bound<'py, PyTuple>,
        cwd: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = command.py();
        if let Some(invoker) = &self.invoker {
            return call_with(
                invoker.bind(py),
                &[("argv", command.into_any()), ("cwd", cwd.clone())],
            );
        }
        let arguments: Vec<String> = command.extract()?;
        let directory: Option<PathBuf> = cwd.extract()?;
        let lifecycle: Bound<'py, PyAny> = dbt_command_lifecycle(py)?;
        let output: PyResult<DbtProcessOutput> = within(&lifecycle, |lifecycle| {
            let output: io::Result<DbtProcessOutput> = py
                .compiler_detach(|| Ok(run_dbt_process(&arguments, directory.as_deref())))
                .map_err(PyRuntimeError::new_err)?;
            py.check_signals()?;
            let output: DbtProcessOutput = output
                .map_err(|error| launch_os_error(py, &error, (&arguments, directory.as_deref())))?;
            report_exit(lifecycle, output.returncode, None)?;
            Ok(output)
        });
        let output: DbtProcessOutput = output.map_err(|error| launch_failure(py, error))?;
        new_record_with(
            py,
            RecordId::DbtCommandResult,
            &[
                ("argv", command.into_any()),
                ("returncode", output.returncode.into_pyobject_any(py)?),
                ("stdout", output.stdout.into_pyobject_any(py)?),
                ("stderr", output.stderr.into_pyobject_any(py)?),
            ],
        )
    }
}

/// Shorthand for converting owned Rust values into Python values.
trait IntoAny {
    fn into_pyobject_any<'py>(self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>>;
}

impl<T: for<'py> pyo3::IntoPyObject<'py>> IntoAny for T {
    fn into_pyobject_any<'py>(self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pyo3::IntoPyObjectExt::into_bound_py_any(self, py)
    }
}

/// `OperationLifecycle(operation_kind="subprocess", operation_name="dbt_command", ...)`.
pub(crate) fn dbt_command_lifecycle(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("operation_kind", "subprocess")?;
    options.set_item("operation_name", "dbt_command")?;
    options.set_item("auto_fail_base_exceptions", false)?;
    host_entry(py, ExecutorHost::OperationLifecycle)?.call((), Some(&options))
}

/// Complete a zero exit, or fail with the exit code and any signal number.
pub(crate) fn report_exit(
    lifecycle: &Bound<'_, PyAny>,
    returncode: i64,
    process_id: Option<u32>,
) -> PyResult<()> {
    let py: Python<'_> = lifecycle.py();
    let options: Bound<'_, PyDict> = PyDict::new(py);
    if let Some(process_id) = process_id {
        options.set_item("process_id", process_id)?;
    }
    if returncode == 0 {
        options.set_item("exit_code", 0)?;
        lifecycle.call_method("completed", (), Some(&options))?;
        return Ok(());
    }
    options.set_item("error_code", format!("exit_{returncode}"))?;
    options.set_item("exit_code", returncode)?;
    options.set_item(
        "signal_number",
        if returncode < 0 {
            Some(-returncode)
        } else {
            None
        },
    )?;
    lifecycle.call_method("failed", (), Some(&options))?;
    Ok(())
}

/// The `OSError` subclass Python's `subprocess` raises for a failed launch.
pub(crate) fn launch_os_error(
    py: Python<'_>,
    error: &io::Error,
    (arguments, cwd): (&[String], Option<&std::path::Path>),
) -> PyErr {
    let Some(code) = error.raw_os_error() else {
        return PyOSError::new_err(error.to_string());
    };
    let filename: String = match cwd {
        Some(directory) if !directory.is_dir() => directory.display().to_string(),
        _ => arguments.first().cloned().unwrap_or_default(),
    };
    let message: PyResult<String> = imported(py, "os", "strerror")
        .and_then(|strerror| strerror.call1((code,)))
        .and_then(|text| text.extract());
    match message {
        Ok(message) => PyOSError::new_err((code, message, filename)),
        Err(failure) => failure,
    }
}

/// `DbtInteropRuntimeError("failed to execute dbt", help=str(error))` from a launch `OSError`.
pub(crate) fn launch_failure(py: Python<'_>, error: PyErr) -> PyErr {
    if !error.is_instance_of::<PyOSError>(py) {
        return error;
    }
    let created = || -> PyResult<PyErr> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("help", error.value(py).str()?)?;
        let instance: Bound<'_, PyAny> =
            error_class(py, ErrorId::DbtInteropRuntime)?.call((LAUNCH_FAILURE,), Some(&options))?;
        let raised: PyErr = PyErr::from_value(instance);
        raised.set_cause(py, Some(error.clone_ref(py)));
        Ok(raised)
    };
    created().unwrap_or_else(|failure| failure)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<DbtRunner>()
}
