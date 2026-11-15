//! `CostContext`: the per-resource cost attribution context, held in a Python `ContextVar`.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyTuple};
use pyo3::{IntoPyObjectExt, pyclass, pymethods};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::cost_telemetry::values::process_object;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const CONTEXT_VARIABLE_NAME: &str = "sqlbuild_cost_resource_context";
const DEFAULT_PHASE: &str = "execute";
const DEFAULT_ATTEMPT: i64 = 1;

static CURRENT: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

fn current_variable(py: Python<'_>) -> PyResult<&Bound<'_, PyAny>> {
    process_object(&CURRENT, py, || {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("default", py.None())?;
        imported(py, "contextvars", "ContextVar")?.call((CONTEXT_VARIABLE_NAME,), Some(&options))
    })
}

/// The active `CostResourceContext`, or `None`.
pub(crate) fn current_context(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    current_variable(py)?.call_method0("get")
}

/// A `with` scope that makes one resource context current, or does nothing without one.
#[pyclass(module = "sqlbuild._native")]
pub(crate) struct CostScope {
    context: Option<Py<PyAny>>,
    token: Option<Py<PyAny>>,
}

#[pymethods]
impl CostScope {
    fn __enter__(&mut self, py: Python<'_>) -> PyResult<()> {
        if let Some(context) = &self.context {
            self.token = Some(
                current_variable(py)?
                    .call_method1("set", (context.bind(py),))?
                    .unbind(),
            );
        }
        Ok(())
    }

    #[pyo3(signature = (*_exception))]
    fn __exit__(&mut self, py: Python<'_>, _exception: &Bound<'_, PyTuple>) -> PyResult<()> {
        if let Some(token) = self.token.take() {
            current_variable(py)?.call_method1("reset", (token.bind(py),))?;
        }
        Ok(())
    }
}

fn or_current<'py>(
    value: Bound<'py, PyAny>,
    current: &Bound<'py, PyAny>,
    field: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if !value.is_none() || current.is_none() {
        return Ok(value);
    }
    native_field(current, field)
}

fn scope_of(context: Bound<'_, PyAny>) -> CostScope {
    CostScope {
        context: Some(context.unbind()),
        token: None,
    }
}

/// Run-cost execution context entry points.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct CostContext;

#[pymethods]
impl CostContext {
    #[staticmethod]
    fn current(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        current_context(py)
    }

    #[staticmethod]
    #[pyo3(signature = (**options))]
    pub(crate) fn scope(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<CostScope> {
        let arguments: KeywordArguments<'_> = KeywordArguments::read(
            py,
            "scope",
            options,
            (
                &["run_id", "resource_type", "resource_name"],
                &["ledger_path", "phase", "attempt", "on_statement_complete"],
            ),
        )?;
        let current: Bound<'_, PyAny> = current_context(py)?;
        let defaulted = |name: &str| -> PyResult<Bound<'_, PyAny>> {
            if arguments.contains(name)? {
                return arguments.value(name);
            }
            match name {
                "attempt" => DEFAULT_ATTEMPT.into_bound_py_any(py),
                _ => DEFAULT_PHASE.into_bound_py_any(py),
            }
        };
        let context: Bound<'_, PyAny> = call_with(
            &host_entry(py, ExecutorHost::CostResourceContext)?,
            &[
                ("run_id", arguments.value("run_id")?),
                ("resource_type", arguments.value("resource_type")?),
                ("resource_name", arguments.value("resource_name")?),
                (
                    "ledger_path",
                    or_current(arguments.value("ledger_path")?, &current, "ledger_path")?,
                ),
                ("phase", defaulted("phase")?),
                ("attempt", defaulted("attempt")?),
                (
                    "on_statement_complete",
                    or_current(
                        arguments.value("on_statement_complete")?,
                        &current,
                        "on_statement_complete",
                    )?,
                ),
            ],
        )?;
        Ok(scope_of(context))
    }

    #[staticmethod]
    #[pyo3(signature = (**options))]
    pub(crate) fn resource_scope(
        py: Python<'_>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<CostScope> {
        let arguments: KeywordArguments<'_> = KeywordArguments::read(
            py,
            "resource_scope",
            options,
            (&["resource_type", "resource_name"], &["phase", "attempt"]),
        )?;
        let current: Bound<'_, PyAny> = current_context(py)?;
        if current.is_none() {
            return Ok(CostScope {
                context: None,
                token: None,
            });
        }
        let phase: Bound<'_, PyAny> = arguments.value("phase")?;
        let attempt: Bound<'_, PyAny> = if arguments.contains("attempt")? {
            arguments.value("attempt")?
        } else {
            DEFAULT_ATTEMPT.into_bound_py_any(py)?
        };
        let context: Bound<'_, PyAny> = call_with(
            &host_entry(py, ExecutorHost::CostResourceContext)?,
            &[
                ("run_id", native_field(&current, "run_id")?),
                ("resource_type", arguments.value("resource_type")?),
                ("resource_name", arguments.value("resource_name")?),
                ("ledger_path", native_field(&current, "ledger_path")?),
                (
                    "phase",
                    if phase.is_none() {
                        native_field(&current, "phase")?
                    } else {
                        phase
                    },
                ),
                ("attempt", attempt),
                (
                    "on_statement_complete",
                    native_field(&current, "on_statement_complete")?,
                ),
            ],
        )?;
        Ok(scope_of(context))
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CostContext>()?;
    module.add_class::<CostScope>()?;
    Ok(())
}
