//! Live progress rows, completion rows and runtime diagnostics of `sqb build`.

use std::collections::HashMap;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyCFunction, PyDict, PyTuple};

use crate::build_progress::models::BuildProgressCallbacks;
use sqlbuild_core::build_progress::models::{SchedulerStateText, StatementText, TestResultText};

use crate::build_progress::_helpers::result_text::optional;
use crate::build_progress::_helpers::result_text::text;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const RUNTIME_LOGGER: &str = "sqlbuild.executor.runtime";
pub(crate) const PASS_OUTCOME: &str = "pass";
pub(crate) const SCHEDULER_DIAGNOSTIC_LIMIT: usize = 50;
pub(crate) const QUERY_DIAGNOSTIC_LIMIT: usize = 25;

/// Which executor result type a completion reports.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultKind {
    Model,
    Seed,
    Function,
    Load,
    PythonNode,
    SqlTest,
    Other,
}

pub(crate) fn result_kind(result: &Bound<'_, PyAny>) -> PyResult<ResultKind> {
    for (field, kind) in [
        ("model_name", ResultKind::Model),
        ("seed_name", ResultKind::Seed),
        ("function_name", ResultKind::Function),
        ("loader_name", ResultKind::Load),
        ("node_name", ResultKind::PythonNode),
        ("test_name", ResultKind::SqlTest),
    ] {
        if result.hasattr(field)? {
            return Ok(kind);
        }
    }
    Ok(ResultKind::Other)
}

fn sql_test_resource_id(result: &Bound<'_, PyAny>) -> PyResult<String> {
    let name: String = text(result, "test_name")?;
    let path: Bound<'_, PyAny> = native_field(result, "source_path")?;
    let block: Bound<'_, PyAny> = native_field(result, "block_index")?;
    let case: Bound<'_, PyAny> = native_field(result, "case_name")?;
    if path.is_none() || block.is_none() || case.is_none() {
        return Ok(format!("sql_test:{name}"));
    }
    Ok(format!(
        "sql_test:{}:{}:{}",
        path.call_method0("as_posix")?.str()?,
        block.str()?,
        case.str()?
    ))
}

/// The resource name and projector identity of a completion.
pub(crate) fn resource_identity(result: &Bound<'_, PyAny>) -> PyResult<Option<(String, String)>> {
    Ok(match result_kind(result)? {
        ResultKind::Model => {
            let name: String = text(result, "model_name")?;
            Some((name.clone(), format!("model:{name}")))
        }
        ResultKind::Seed => {
            let name: String = text(result, "seed_name")?;
            Some((name.clone(), format!("seed:{name}")))
        }
        ResultKind::Function => {
            let name: String = text(result, "function_name")?;
            let kind: String = text(result, "function_kind")?;
            Some((name.clone(), format!("{kind}:{name}")))
        }
        ResultKind::Load => {
            let name: String = text(result, "source_name")?;
            Some((name.clone(), format!("source:{name}")))
        }
        ResultKind::PythonNode => {
            let name: String = text(result, "node_name")?;
            let kind: String = text(result, "kind")?;
            Some((name.clone(), format!("{kind}:{name}")))
        }
        ResultKind::SqlTest => Some((text(result, "test_name")?, sql_test_resource_id(result)?)),
        ResultKind::Other => None,
    })
}

/// Mutable callback state; only held briefly and never across Python calls.
#[derive(Debug, Default)]
pub(crate) struct CallbackState {
    pub(crate) counter: usize,
    pub(crate) node_name: String,
    pub(crate) node_kind: String,
    pub(crate) sub_message: String,
    pub(crate) frame: usize,
    pub(crate) line_active: bool,
    pub(crate) cursor_hidden: bool,
    pub(crate) stop_event: Option<Py<PyAny>>,
    pub(crate) thread: Option<Py<PyAny>>,
    pub(crate) tests_by_model: HashMap<String, Vec<TestResultText>>,
    pub(crate) scheduler_count: usize,
    pub(crate) scheduler_omitted: usize,
    pub(crate) last_scheduler: Option<SchedulerStateText>,
    pub(crate) query_counts: HashMap<String, usize>,
    pub(crate) query_omitted: usize,
    pub(crate) last_query: Option<StatementText>,
    /// Long-running nodes for the heartbeat line of captured output.
    pub(crate) heartbeat: crate::build_progress::_helpers::heartbeat::HeartbeatState,
}

pub(crate) fn scheduler_state(state: &Bound<'_, PyAny>) -> PyResult<SchedulerStateText> {
    Ok(SchedulerStateText {
        running: native_field(state, "running")?.extract()?,
        ready: native_field(state, "ready")?.extract()?,
        waiting: native_field(state, "waiting")?.extract()?,
        limit: native_field(state, "limit")?.extract()?,
        aborted: native_field(state, "aborted")?.extract()?,
    })
}

pub(crate) fn statement(telemetry: &Bound<'_, PyAny>) -> PyResult<StatementText> {
    Ok(StatementText {
        query_id: optional(telemetry, "query_id")?,
        status: text(telemetry, "status")?,
        elapsed_seconds: native_field(telemetry, "elapsed_seconds")?.extract()?,
        resource_type: text(telemetry, "resource_type")?,
        resource_name: text(telemetry, "resource_name")?,
        phase: text(telemetry, "phase")?,
    })
}

/// What a relay does with its owner and the arguments it was called with.
pub(crate) type Forward = fn(&BuildProgressCallbacks, &Bound<'_, PyTuple>) -> PyResult<()>;

/// A callable that calls `forward` on `callbacks`, built without a Python lookup.
pub(crate) fn relay<'py>(
    callbacks: &Bound<'py, PyAny>,
    forward: Forward,
) -> PyResult<Bound<'py, PyAny>> {
    let owned: Py<BuildProgressCallbacks> = callbacks
        .downcast::<BuildProgressCallbacks>()?
        .clone()
        .unbind();
    let call = move |arguments: &Bound<'_, PyTuple>,
                     _keywords: Option<&Bound<'_, PyDict>>|
          -> PyResult<()> { forward(owned.get(), arguments) };
    Ok(PyCFunction::new_closure(callbacks.py(), None, None, call)?.into_any())
}
