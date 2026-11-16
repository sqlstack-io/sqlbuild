//! Append canonical integration-result records for a parent integration, one per claimed terminal.

use std::sync::Mutex;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyclass, pymethods};
use sqlbuild_core::integration_results::errors::IntegrationRecordError;
use sqlbuild_core::terminal_events::models::TerminalRequest;

use crate::bindings::_helpers::result_output::integration_records::{
    clone_record, degraded_record, degraded_warning, execution_succeeded, integration_record,
};
use crate::bindings::_helpers::result_output::projection::resource_identity;
use crate::bindings::_helpers::result_output::terminal_index::{
    TerminalEventClaim, TerminalEventIndex, current_index_object,
};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::result_output::types::Checked;

pub(crate) const RESULT_PATH_ENV: &str = "SQLBUILD_INTEGRATION_RESULT_PATH";
const BUILD_OPTIONS: (&[&str], &[&str]) = (&["result", "plan"], &["command"]);

/// Thread-safe JSON Lines execution event writer.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ExecutionEventWriter {
    /// A `threading.Lock`; it releases the GIL while it waits.
    lock: Py<PyAny>,
    terminal_index: Option<Py<TerminalEventIndex>>,
    /// The open text stream, until the writer is closed.
    stream: Mutex<Option<Py<PyAny>>>,
}

/// A Python context manager entered for the duration of `body`.
fn within<T>(manager: &Bound<'_, PyAny>, body: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
    manager.call_method0("__enter__")?;
    let outcome: PyResult<T> = body();
    let none = manager.py().None();
    manager.call_method1("__exit__", (&none, &none, &none))?;
    outcome
}

impl ExecutionEventWriter {
    fn open_stream(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Ok(self
            .stream
            .lock()
            .map_err(|_| PyRuntimeError::new_err("execution event writer lock is poisoned"))?
            .as_ref()
            .map(|stream| stream.clone_ref(py)))
    }

    /// Run `body` holding the writer lock and the index's output scope, when the stream is open.
    fn serialized(
        &self,
        py: Python<'_>,
        body: impl FnOnce(&Bound<'_, PyAny>) -> PyResult<()>,
    ) -> PyResult<()> {
        within(self.lock.bind(py), || {
            let Some(stream) = self.open_stream(py)? else {
                return Ok(());
            };
            match &self.terminal_index {
                Some(index) => within(&index.get().output_lock(py), || body(stream.bind(py))),
                None => body(stream.bind(py)),
            }
        })
    }

    fn claim(
        &self,
        py: Python<'_>,
        request: &TerminalRequest,
    ) -> PyResult<Option<TerminalEventClaim>> {
        match &self.terminal_index {
            Some(index) => index.get().claim(py, request),
            None => Ok(None),
        }
    }

    /// Write the record, or after a contract failure warn and write the degraded record.
    fn publish(
        stream: &Bound<'_, PyAny>,
        terminal: &Bound<'_, PyAny>,
        record: Checked<Option<String>>,
        degraded: impl FnOnce() -> Checked<Option<String>>,
    ) -> PyResult<()> {
        let error: IntegrationRecordError = match record? {
            Ok(Some(line)) => return write(stream, &line),
            Ok(None) => return Ok(()),
            Err(error) => error,
        };
        let stderr: Bound<'_, PyAny> = stream.py().import("sys")?.getattr("stderr")?;
        stderr.call_method1(
            "write",
            (degraded_warning(&error, execution_succeeded(terminal)?),),
        )?;
        stderr.call_method0("flush")?;
        match degraded()? {
            Ok(Some(line)) => write(stream, &line),
            Ok(None) | Err(_) => Ok(()),
        }
    }
}

fn write(stream: &Bound<'_, PyAny>, line: &str) -> PyResult<()> {
    stream.call_method1("write", (line,))?;
    stream.call_method0("flush")?;
    Ok(())
}

#[pymethods]
impl ExecutionEventWriter {
    #[new]
    #[pyo3(signature = (*, path = None))]
    pub(crate) fn new(py: Python<'_>, path: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
        let configured: Bound<'_, PyAny> = py
            .import("os")?
            .getattr("environ")?
            .call_method1("get", (RESULT_PATH_ENV,))?;
        let path: Option<Bound<'_, PyAny>> =
            match path.filter(|path| path.is_truthy().unwrap_or(true)) {
                Some(path) => Some(path),
                None if configured.is_truthy()? => Some(
                    py.import("pathlib")?
                        .getattr("Path")?
                        .call1((configured,))?,
                ),
                None => None,
            };
        let stream: Option<Py<PyAny>> = match path {
            Some(path) => {
                let keywords: Bound<'_, PyDict> = PyDict::new(py);
                keywords.set_item("parents", true)?;
                keywords.set_item("exist_ok", true)?;
                native_field(&path, "parent")?.call_method("mkdir", (), Some(&keywords))?;
                let options: Bound<'_, PyDict> = PyDict::new(py);
                options.set_item("mode", "a")?;
                options.set_item("encoding", "utf-8")?;
                Some(path.call_method("open", (), Some(&options))?.unbind())
            }
            None => None,
        };
        Ok(Self {
            lock: py.import("threading")?.call_method0("Lock")?.unbind(),
            terminal_index: current_index_object(py)?,
            stream: Mutex::new(stream),
        })
    }

    /// Write canonical terminal evidence plus typed build-result enrichment.
    #[pyo3(signature = (**options))]
    fn write_build_result(
        &self,
        py: Python<'_>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let arguments: KeywordArguments<'_> =
            KeywordArguments::read(py, "write_build_result", options, BUILD_OPTIONS)?;
        let result: Bound<'_, PyAny> = arguments.value("result")?;
        let plan: Bound<'_, PyAny> = arguments.value("plan")?;
        let command: String = if arguments.passed("command")? {
            arguments.value("command")?.extract()?
        } else {
            "build".to_owned()
        };
        self.serialized(py, |stream| {
            let Some((resource_name, resource_id)) = resource_identity(&result)? else {
                return Ok(());
            };
            let Some(claim) = self.claim(
                py,
                &TerminalRequest {
                    resource_name,
                    resource_id: Some(resource_id),
                    ..TerminalRequest::default()
                },
            )?
            else {
                return Ok(());
            };
            let terminal: Bound<'_, PyAny> = claim.terminal_event(py);
            let sequence: usize = claim.sequence();
            Self::publish(
                stream,
                &terminal,
                integration_record((&result, &plan), (&terminal, sequence), &command),
                || degraded_record(&result, (&terminal, sequence), &command),
            )
        })
    }

    /// Write canonical terminal evidence plus typed clone-result enrichment.
    #[pyo3(signature = (*, item, resource_type))]
    fn write_clone_result(
        &self,
        py: Python<'_>,
        item: Bound<'_, PyAny>,
        resource_type: &str,
    ) -> PyResult<()> {
        self.serialized(py, |stream| {
            let name: String = native_field(&item, "name")?.extract()?;
            let Some(claim) = self.claim(
                py,
                &TerminalRequest {
                    resource_id: Some(format!("{resource_type}:{name}")),
                    resource_name: name,
                    ..TerminalRequest::default()
                },
            )?
            else {
                return Ok(());
            };
            let terminal: Bound<'_, PyAny> = claim.terminal_event(py);
            let sequence: usize = claim.sequence();
            Self::publish(
                stream,
                &terminal,
                Ok(clone_record(&item, (&terminal, sequence))?.map(Some)),
                || degraded_record(&item, (&terminal, sequence), "clone"),
            )
        })
    }

    /// Close the event stream when configured.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        within(self.lock.bind(py), || {
            let Some(stream) = self.open_stream(py)? else {
                return Ok(());
            };
            let close = || -> PyResult<()> {
                *self.stream.lock().map_err(|_| {
                    PyRuntimeError::new_err("execution event writer lock is poisoned")
                })? = None;
                stream.bind(py).call_method0("close")?;
                Ok(())
            };
            match &self.terminal_index {
                Some(index) => within(&index.get().output_lock(py), close),
                None => close(),
            }
        })
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<ExecutionEventWriter>()
}
