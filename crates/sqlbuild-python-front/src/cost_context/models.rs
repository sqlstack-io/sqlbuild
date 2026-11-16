//! Public records of cost context.

use crate::cost_context::_helpers::context::{
    DEFAULT_ATTEMPT, DEFAULT_PHASE, current_context, current_variable, or_current, scope_of,
};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};
use pyo3::{IntoPyObjectExt, pyclass, pymethods};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A `with` scope that makes one resource context current, or does nothing without one.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct CostScope {
    pub(crate) context: Option<Py<PyAny>>,
    pub(crate) token: Option<Py<PyAny>>,
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

/// Run-cost execution context entry points.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct CostContext;

#[pymethods]
impl CostContext {
    #[staticmethod]
    fn current(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        current_context(py)
    }

    #[staticmethod]
    #[pyo3(signature = (**options))]
    pub fn scope(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<CostScope> {
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
    pub fn resource_scope(
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
