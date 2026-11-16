//! The invocation-scoped index of canonical lifecycle facts that result output projects from.

use sqlbuild_python_base::build_progress::main::progress::terminal_request::terminal_request;
use std::sync::{Mutex, MutexGuard};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRef, PyResult, Python,
};
use pyo3::types::{PyDict, PyInt, PyString, PyTuple};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_core::terminal_events::main::claim_resource_terminal::claim_resource_terminal;

use sqlbuild_core::terminal_events::main::consume_terminal_fact::consume_terminal_fact;
use sqlbuild_core::terminal_events::main::latest_resource_terminal::latest_resource_terminal;
use sqlbuild_core::terminal_events::main::lifecycle_terminal::lifecycle_terminal;
use sqlbuild_core::terminal_events::models::{TerminalFact, TerminalIndex, TerminalRequest};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::NativeAttr;

/// The module attribute holding the `ContextVar` of the installed index.
const CONTEXT_ATTRIBUTE: &str = "_terminal_event_index_context";
const CONTEXT_NAME: &str = "sqlbuild_terminal_event_index";

struct IndexState {
    index: TerminalIndex,
    events: Vec<Py<PyAny>>,
}

/// Retain canonical facts and claim resource terminals for result projection.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct TerminalEventIndex {
    state: Mutex<IndexState>,
    output_lock: Py<PyAny>,
}

/// One claimed terminal with its immutable canonical publication sequence.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct TerminalEventClaim {
    #[pyo3(get)]
    pub(crate) terminal: Py<PyAny>,
    #[pyo3(get)]
    pub(crate) event_sequence: usize,
}

/// `payload.get(key)` when it is a string.
fn payload_text(payload: &Bound<'_, PyAny>, key: &str) -> PyResult<Option<String>> {
    let value: Bound<'_, PyAny> = mapping_get(payload, key)?;
    if value.is_instance_of::<PyString>() {
        return Ok(Some(value.extract()?));
    }
    Ok(None)
}

/// The payload attempt number, when it is an `int` (or `bool`, which Python compares as one).
fn payload_attempt_number(payload: &Bound<'_, PyAny>) -> PyResult<Option<i64>> {
    let value: Bound<'_, PyAny> = mapping_get(payload, "attempt_number")?;
    if !value.is_instance_of::<PyInt>() {
        return Ok(None);
    }
    let Ok(number) = value.extract::<i64>() else {
        return Ok(None);
    };
    Ok(Some(number))
}

fn optional_text(event: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    native_field(event, name)?.extract()
}

/// The facts of one lifecycle event that terminal indexing reads.
fn terminal_fact(event: &Bound<'_, PyAny>) -> PyResult<TerminalFact> {
    let payload: Bound<'_, PyAny> = native_field(event, "payload")?;
    Ok(TerminalFact {
        event_id: native_field(event, "event_id")?.extract()?,
        event_type: native_field(event, "event_type")?.extract()?,
        resource_id: optional_text(event, "resource_id")?,
        resource_name: payload_text(&payload, "resource_name")?,
        resource_attempt_id: optional_text(event, "resource_attempt_id")?,
        attempt_number: payload_attempt_number(&payload)?,
        operation_name: payload_text(&payload, "operation_name")?,
    })
}

impl TerminalEventIndex {
    fn locked(&self) -> PyResult<MutexGuard<'_, IndexState>> {
        self.state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("terminal event index lock is poisoned"))
    }

    fn event_at(&self, py: Python<'_>, sequence: Option<usize>) -> PyResult<Option<Py<PyAny>>> {
        let state: MutexGuard<'_, IndexState> = self.locked()?;
        Ok(sequence.map(|sequence| state.events[sequence].clone_ref(py)))
    }

    /// The latest terminal of a resource without claiming it.
    pub(crate) fn latest_terminal<'py>(
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
    pub(crate) fn aggregate_terminal<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
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
    pub(crate) fn output_lock<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.output_lock.bind(py).clone()
    }
}

#[pymethods]
impl TerminalEventIndex {
    #[new]
    pub(crate) fn new(py: Python<'_>) -> PyResult<Self> {
        Ok(Self {
            state: Mutex::new(IndexState {
                index: TerminalIndex::default(),
                events: Vec::new(),
            }),
            output_lock: py.import("threading")?.call_method0("RLock")?.unbind(),
        })
    }

    /// Store a known fact once in dispatcher publication order.
    fn consume(&self, event: &Bound<'_, PyAny>) -> PyResult<()> {
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
    fn claim_resource_terminal(
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
    fn resource_terminal<'py>(
        &self,
        py: Python<'py>,
        resource_name: &str,
        resource_id: Option<&str>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.latest_terminal(py, resource_name, resource_id)
    }

    /// Return the strongest aggregate terminal available to final JSON.
    fn lifecycle_terminal<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.aggregate_terminal(py)
    }

    /// Serialize terminal claims with physical integration-result output.
    fn output_serialization_scope<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.output_lock(py)
    }

    /// Return idempotent canonical facts in publication order.
    fn events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let state: MutexGuard<'_, IndexState> = self.locked()?;
        PyTuple::new(py, state.events.iter().map(|event| event.bind(py)))
    }
}

/// Installs one index for the duration of a `with` block.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct TerminalEventIndexScope {
    index: Py<TerminalEventIndex>,
    token: Mutex<Option<Py<PyAny>>>,
}

fn context_variable(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    native_attr(py, NativeAttr::TerminalEventIndexContext)
}

#[pymethods]
impl TerminalEventIndexScope {
    fn __enter__(&self, py: Python<'_>) -> PyResult<Py<TerminalEventIndex>> {
        let token: Bound<'_, PyAny> = context_variable(py)?.call_method1("set", (&self.index,))?;
        *self.token.lock().map_err(|_| {
            PyRuntimeError::new_err("terminal event index scope lock is poisoned")
        })? = Some(token.unbind());
        Ok(self.index.clone_ref(py))
    }

    #[pyo3(signature = (*_exc_info))]
    fn __exit__(&self, py: Python<'_>, _exc_info: &Bound<'_, PyTuple>) -> PyResult<bool> {
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

/// The terminal event index installed for this invocation.
pub(crate) fn current_index(py: Python<'_>) -> PyResult<Option<PyRef<'_, TerminalEventIndex>>> {
    let installed: Bound<'_, PyAny> = context_variable(py)?.call_method0("get")?;
    if installed.is_none() {
        return Ok(None);
    }
    Ok(Some(installed.extract()?))
}

/// The installed terminal event index, as an owned reference.
pub(crate) fn current_index_object(py: Python<'_>) -> PyResult<Option<Py<TerminalEventIndex>>> {
    let installed: Bound<'_, PyAny> = context_variable(py)?.call_method0("get")?;
    if installed.is_none() {
        return Ok(None);
    }
    Ok(Some(installed.extract()?))
}

impl TerminalEventClaim {
    pub(crate) fn terminal_event<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.terminal.bind(py).clone()
    }

    pub(crate) fn sequence(&self) -> usize {
        self.event_sequence
    }
}

/// Return the terminal event index installed for this invocation.
#[pyfunction]
pub(crate) fn current_terminal_event_index(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    context_variable(py)?.call_method0("get")
}

/// Install and reliably restore one invocation terminal event index.
#[pyfunction]
pub(crate) fn terminal_event_index_scope(index: Py<TerminalEventIndex>) -> TerminalEventIndexScope {
    TerminalEventIndexScope {
        index,
        token: Mutex::new(None),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py: Python<'_> = module.py();
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    pyo3::types::PyDictMethods::set_item(&keywords, "default", py.None())?;
    module.add(
        CONTEXT_ATTRIBUTE,
        py.import("contextvars")?
            .getattr("ContextVar")?
            .call((CONTEXT_NAME,), Some(&keywords))?,
    )?;
    module.add_class::<TerminalEventIndex>()?;
    module.add_class::<TerminalEventClaim>()?;
    module.add_class::<TerminalEventIndexScope>()?;
    module.add_function(wrap_pyfunction!(current_terminal_event_index, module)?)?;
    module.add_function(wrap_pyfunction!(terminal_event_index_scope, module)?)
}
