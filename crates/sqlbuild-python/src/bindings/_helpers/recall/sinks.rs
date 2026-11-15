//! The native subscriber that publishes every lifecycle fact and artifact into Recall.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyclass, pymethods};
use pyo3::types::PyString;
use serde_json::{Map, Value};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_recall::records::main::parse_event::parse_event;
use sqlbuild_recall::writer::models::RecallSession;

use crate::bindings::_helpers::recall::constants::{
    INVOCATION_COMPLETED, INVOCATION_FAILED, INVOCATION_STARTED, STATEMENT_COMPLETED,
    STATEMENT_FAILED,
};
use crate::bindings::_helpers::recall::events::{escaped_text, event_object};
use crate::bindings::_helpers::recall::replacements::RecallReplacements;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// `event` as the JSON object `lifecycle_event_to_json` encodes; `None` when it cannot be encoded.
pub(crate) fn lifecycle_event_object(
    py: Python<'_>,
    event: &Bound<'_, PyAny>,
) -> PyResult<Option<Map<String, Value>>> {
    if let Some(object) = event_object(event)? {
        return Ok(Some(object));
    }
    let encoded = host_entry(py, ExecutorHost::LifecycleEventToJson)?.call1((event,))?;
    let text = escaped_text(encoded.downcast::<PyString>()?)?;
    match parse_event(&text) {
        Ok(object) => Ok(Some(object)),
        Err(_unencodable) => Ok(None),
    }
}

/// Queue `event` in `session`; an event that cannot be encoded is skipped.
pub(crate) fn publish_lifecycle_event(
    py: Python<'_>,
    session: &RecallSession,
    event: &Bound<'_, PyAny>,
) -> PyResult<()> {
    if let Some(object) = lifecycle_event_object(py, event)? {
        session.publish_event_object(object);
    }
    Ok(())
}

/// The SQL and resource of a statement that has not reached its terminal event yet.
type RunningSql = HashMap<String, (String, Option<String>)>;

/// Which of an invocation's own lifecycle events the sink has recorded, so an interrupted run records each exactly once.
#[derive(Debug, Default)]
pub(crate) struct InvocationEvents {
    started: AtomicBool,
    ended: AtomicBool,
}

impl InvocationEvents {
    /// Whether `invocation_started` was recorded.
    pub(crate) fn started(&self) -> bool {
        self.started.load(Ordering::SeqCst)
    }

    /// Whether `invocation_completed` or `invocation_failed` was recorded.
    pub(crate) fn ended(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }

    fn note(&self, event_type: &str) {
        if event_type == INVOCATION_STARTED {
            self.started.store(true, Ordering::SeqCst);
        } else if event_type == INVOCATION_COMPLETED || event_type == INVOCATION_FAILED {
            self.ended.store(true, Ordering::SeqCst);
        }
    }
}

/// Publishes lifecycle events, through the Recall replacement sink on the delivery thread when one is configured, and SQL artifacts of the active session; delivery is best effort. The SQL of a statement that fails or is cancelled is queued again with its terminal event's priority.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct RecallLifecycleSink {
    pub(crate) session: Arc<RecallSession>,
    pub(crate) replacements: Arc<RecallReplacements>,
    pub(crate) running_sql: Arc<Mutex<RunningSql>>,
    pub(crate) invocation_events: Arc<InvocationEvents>,
}

#[pymethods]
impl RecallLifecycleSink {
    /// Record `event`; an event Recall cannot encode is skipped, never raised into the command.
    fn __call__(&self, py: Python<'_>, event: &Bound<'_, PyAny>) -> PyResult<()> {
        recording(py, self.publish(py, event))
    }

    /// Record one SQL artifact; text Recall cannot encode is skipped, never raised into the command.
    #[pyo3(signature = (*, sql, statement_id=None, resource_id=None))]
    fn record_sql(
        &self,
        py: Python<'_>,
        sql: &Bound<'_, PyString>,
        statement_id: Option<&Bound<'_, PyString>>,
        resource_id: Option<&Bound<'_, PyString>>,
    ) -> PyResult<()> {
        let texts = (|| -> PyResult<(String, Option<String>, Option<String>)> {
            Ok((
                escaped_text(sql)?,
                statement_id.map(escaped_text).transpose()?,
                resource_id.map(escaped_text).transpose()?,
            ))
        })();
        recording(
            py,
            texts.map(|(sql, statement_id, resource_id)| {
                self.session
                    .publish_sql(&sql, statement_id.as_deref(), resource_id.as_deref());
                if let Some(statement_id) = statement_id {
                    self.running().insert(statement_id, (sql, resource_id));
                }
            }),
        )
    }
}

impl RecallLifecycleSink {
    fn running(&self) -> std::sync::MutexGuard<'_, RunningSql> {
        self.running_sql
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Forget a statement's SQL at its terminal event; a failed one's SQL is queued again first.
    fn settle_statement(&self, event: &Bound<'_, PyAny>, event_type: &str) -> PyResult<()> {
        if event_type != STATEMENT_COMPLETED && event_type != STATEMENT_FAILED {
            return Ok(());
        }
        let Some(statement_id) = event.getattr_opt("statement_id")? else {
            return Ok(());
        };
        if statement_id.is_none() {
            return Ok(());
        }
        let statement_id: String = escaped_text(&statement_id.str()?)?;
        let Some((sql, resource_id)) = self.running().remove(&statement_id) else {
            return Ok(());
        };
        if event_type == STATEMENT_FAILED {
            self.session
                .publish_failed_sql(&sql, Some(&statement_id), resource_id.as_deref());
        }
        Ok(())
    }

    fn publish(&self, py: Python<'_>, event: &Bound<'_, PyAny>) -> PyResult<()> {
        let event_type: String = match event.getattr_opt("event_type")? {
            Some(event_type) => event_type.str()?.to_string(),
            None => String::new(),
        };
        self.settle_statement(event, &event_type)?;
        if !self
            .replacements
            .deliver_lifecycle(py, &self.session, event)?
        {
            publish_lifecycle_event(py, &self.session, event)?;
        }
        self.invocation_events.note(&event_type);
        Ok(())
    }
}

/// `outcome` with ordinary exceptions dropped, so recording never changes what the command raises; interrupts still propagate.
fn recording(py: Python<'_>, outcome: PyResult<()>) -> PyResult<()> {
    match outcome {
        Err(error) if error.is_instance_of::<PyException>(py) => Ok(()),
        other => other,
    }
}
