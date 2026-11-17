//! Public records of result output.

use crate::result_output::_helpers::events::event_writer::BUILD_OPTIONS;
use crate::result_output::_helpers::events::event_writer::within;
use crate::result_output::_helpers::events::event_writer::write;
use crate::result_output::_helpers::events::integration_records::clone_record;
use crate::result_output::_helpers::events::integration_records::degraded_record;
use crate::result_output::_helpers::events::integration_records::degraded_warning;
use crate::result_output::_helpers::events::integration_records::execution_succeeded;
use crate::result_output::_helpers::events::integration_records::integration_record;
use crate::result_output::_helpers::events::terminal_index::IndexState;
use crate::result_output::_helpers::events::terminal_index::TerminalEventClaim;
use crate::result_output::_helpers::events::terminal_index::context_variable;
use crate::result_output::_helpers::events::terminal_index::current_index_object;
use crate::result_output::_helpers::events::terminal_index::terminal_fact;
use crate::result_output::_helpers::results::projection::resource_identity;
use crate::result_output::constants::RESULT_PATH_ENV;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;
use sqlbuild_core::integration_results::errors::IntegrationRecordError;
use sqlbuild_core::terminal_events::main::claim_resource_terminal::claim_resource_terminal;
use sqlbuild_core::terminal_events::main::consume_terminal_fact::consume_terminal_fact;
use sqlbuild_core::terminal_events::main::latest_resource_terminal::latest_resource_terminal;
use sqlbuild_core::terminal_events::main::lifecycle_terminal::lifecycle_terminal;
use sqlbuild_core::terminal_events::models::TerminalFact;
use sqlbuild_core::terminal_events::models::TerminalIndex;
use sqlbuild_core::terminal_events::models::TerminalRequest;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_progress::main::progress::terminal_request::terminal_request;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::result_output::types::Checked;
use std::sync::Mutex;
use std::sync::MutexGuard;

/// Thread-safe JSON Lines execution event writer.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct ExecutionEventWriter {
    /// A `threading.Lock`; it releases the GIL while it waits.
    pub(crate) lock: Py<PyAny>,
    pub(crate) terminal_index: Option<Py<TerminalEventIndex>>,
    /// The open text stream, until the writer is closed.
    pub(crate) stream: Mutex<Option<Py<PyAny>>>,
}

impl ExecutionEventWriter {
    pub(crate) fn open_stream(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Ok(self
            .stream
            .lock()
            .map_err(|_| PyRuntimeError::new_err("execution event writer lock is poisoned"))?
            .as_ref()
            .map(|stream| stream.clone_ref(py)))
    }

    /// Run `body` holding the writer lock and the index's output scope, when the stream is open.
    pub(crate) fn serialized(
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

    pub(crate) fn claim(
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
    pub(crate) fn publish(
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

#[pymethods]
impl ExecutionEventWriter {
    #[new]
    #[pyo3(signature = (*, path = None))]
    pub fn new(py: Python<'_>, path: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
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
    pub(crate) fn write_build_result(
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
    pub(crate) fn write_clone_result(
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
    pub(crate) fn close(&self, py: Python<'_>) -> PyResult<()> {
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

/// Retain canonical facts and claim resource terminals for result projection.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct TerminalEventIndex {
    pub(crate) state: Mutex<IndexState>,
    pub(crate) output_lock: Py<PyAny>,
}

impl TerminalEventIndex {
    pub(crate) fn locked(&self) -> PyResult<MutexGuard<'_, IndexState>> {
        self.state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("terminal event index lock is poisoned"))
    }

    pub(crate) fn event_at(
        &self,
        py: Python<'_>,
        sequence: Option<usize>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let state: MutexGuard<'_, IndexState> = self.locked()?;
        Ok(sequence.map(|sequence| state.events[sequence].clone_ref(py)))
    }

    /// The latest terminal of a resource without claiming it.
    pub fn latest_terminal<'py>(
        &self,
        py: Python<'py>,
        resource_name: &str,
        resource_id: Option<&str>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let sequence: Option<usize> = {
            let state: MutexGuard<'_, IndexState> = self.locked()?;
            latest_resource_terminal(&state.index, resource_name, resource_id)
        };
        Ok(self
            .event_at(py, sequence)?
            .map(|event| event.into_bound(py)))
    }

    /// The strongest aggregate terminal for the final result document.
    pub fn aggregate_terminal<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let sequence: Option<usize> = lifecycle_terminal(&self.locked()?.index);
        Ok(self
            .event_at(py, sequence)?
            .map(|event| event.into_bound(py)))
    }

    /// Claim the terminal a result callback reports on.
    pub(crate) fn claim(
        &self,
        py: Python<'_>,
        request: &TerminalRequest,
    ) -> PyResult<Option<TerminalEventClaim>> {
        let mut state: MutexGuard<'_, IndexState> = self.locked()?;
        let (index, claimed) = claim_resource_terminal(std::mem::take(&mut state.index), request);
        state.index = index;
        let Some(sequence) = claimed else {
            return Ok(None);
        };
        Ok(Some(TerminalEventClaim {
            terminal: state.events[sequence].clone_ref(py),
            event_sequence: sequence,
        }))
    }

    /// The reentrant lock that serializes claims with physical result output.
    pub fn output_lock<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.output_lock.bind(py).clone()
    }
}

#[pymethods]
impl TerminalEventIndex {
    #[new]
    pub fn new(py: Python<'_>) -> PyResult<Self> {
        Ok(Self {
            state: Mutex::new(IndexState {
                index: TerminalIndex::default(),
                events: Vec::new(),
            }),
            output_lock: py.import("threading")?.call_method0("RLock")?.unbind(),
        })
    }

    /// Store a known fact once in dispatcher publication order.
    pub(crate) fn consume(&self, event: &Bound<'_, PyAny>) -> PyResult<()> {
        let fact: TerminalFact = terminal_fact(event)?;
        let mut state: MutexGuard<'_, IndexState> = self.locked()?;
        let (index, stored) = consume_terminal_fact(std::mem::take(&mut state.index), fact);
        state.index = index;
        if stored.is_some() {
            state.events.push(event.clone().unbind());
        }
        Ok(())
    }

    /// Claim the latest exact terminal available when a result callback arrives.
    #[pyo3(signature = (**options))]
    pub(crate) fn claim_resource_terminal(
        &self,
        py: Python<'_>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Option<TerminalEventClaim>> {
        self.claim(
            py,
            &terminal_request(py, "claim_resource_terminal", options)?,
        )
    }

    /// Return the latest matching terminal without claiming it.
    #[pyo3(signature = (*, resource_name, resource_id = None))]
    pub(crate) fn resource_terminal<'py>(
        &self,
        py: Python<'py>,
        resource_name: &str,
        resource_id: Option<&str>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.latest_terminal(py, resource_name, resource_id)
    }

    /// Return the strongest aggregate terminal available to final JSON.
    pub(crate) fn lifecycle_terminal<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.aggregate_terminal(py)
    }

    /// Serialize terminal claims with physical integration-result output.
    pub(crate) fn output_serialization_scope<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.output_lock(py)
    }

    /// Return idempotent canonical facts in publication order.
    pub(crate) fn events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let state: MutexGuard<'_, IndexState> = self.locked()?;
        PyTuple::new(py, state.events.iter().map(|event| event.bind(py)))
    }
}

/// Installs one index for the duration of a `with` block.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct TerminalEventIndexScope {
    pub(crate) index: Py<TerminalEventIndex>,
    pub(crate) token: Mutex<Option<Py<PyAny>>>,
}

#[pymethods]
impl TerminalEventIndexScope {
    pub(crate) fn __enter__(&self, py: Python<'_>) -> PyResult<Py<TerminalEventIndex>> {
        let token: Bound<'_, PyAny> = context_variable(py)?.call_method1("set", (&self.index,))?;
        *self.token.lock().map_err(|_| {
            PyRuntimeError::new_err("terminal event index scope lock is poisoned")
        })? = Some(token.unbind());
        Ok(self.index.clone_ref(py))
    }

    #[pyo3(signature = (*_exc_info))]
    pub(crate) fn __exit__(
        &self,
        py: Python<'_>,
        _exc_info: &Bound<'_, PyTuple>,
    ) -> PyResult<bool> {
        let token: Option<Py<PyAny>> = self
            .token
            .lock()
            .map_err(|_| PyRuntimeError::new_err("terminal event index scope lock is poisoned"))?
            .take();
        if let Some(token) = token {
            context_variable(py)?.call_method1("reset", (token,))?;
        }
        Ok(false)
    }
}
