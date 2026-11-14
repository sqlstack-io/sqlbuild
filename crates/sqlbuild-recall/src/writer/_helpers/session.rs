//! The session handle: producers queue records; `close` flushes within the shutdown budget.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Map, Value};

use crate::records::constants::SEVERITY_ORDER;
use crate::records::errors::RecordError;
use crate::records::main::diagnostic_input_from_json::diagnostic_input_from_json;
use crate::records::main::diagnostic_row::diagnostic_row;
use crate::records::main::event_severity::event_severity;
use crate::records::main::parse_event::parse_event;
use crate::records::models::OutputStream;
use crate::writer::_helpers::delivery::locked;
use crate::writer::_helpers::queue::{Payload, Queued, SqlInput};
use crate::writer::constants::{JSON_SCALAR_BYTES, MAX_ARTIFACT_BYTES};
use crate::writer::models::{
    DeliveryStream, DeliverySummary, Priority, RecallSession, SetupOutcome,
};
use crate::writer::types::{OutputReplacement, PendingLifecycleEvent};

pub(crate) fn severity_rank(severity: &str) -> usize {
    SEVERITY_ORDER
        .iter()
        .position(|candidate| *candidate == severity)
        .unwrap_or(0)
}

/// Roughly how many bytes `event` holds, for the queue's byte capacity.
fn json_bytes(event: &Map<String, Value>) -> usize {
    event
        .iter()
        .map(|(key, value)| key.len() + value_bytes(value))
        .sum()
}

fn value_bytes(value: &Value) -> usize {
    match value {
        Value::String(text) => text.len(),
        Value::Array(items) => items.iter().map(value_bytes).sum(),
        Value::Object(entries) => json_bytes(entries),
        Value::Null | Value::Bool(_) | Value::Number(_) => JSON_SCALAR_BYTES,
    }
}

fn event_priority(severity: &str) -> Priority {
    match severity {
        "error" => Priority::FailedTerminal,
        "info" => Priority::Terminal,
        _ => Priority::Lifecycle,
    }
}

impl RecallSession {
    /// Wait up to `budget` for table setup; `None` while it is still running.
    pub fn wait_for_setup(&self, budget: Duration) -> Option<SetupOutcome> {
        let deadline = Instant::now() + budget;
        let mut setup = self
            .shared
            .setup
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while setup.is_none() {
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            setup = self
                .shared
                .setup_done
                .wait_timeout(setup, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        setup.clone()
    }

    /// The invocation this session records.
    #[must_use]
    pub fn invocation_id(&self) -> String {
        locked(&self.shared.state).envelope.invocation_id.clone()
    }

    /// Accept bytes a process stream accepted.
    pub fn append_output(&self, stream: OutputStream, data: &[u8]) {
        let mut state = locked(&self.shared.state);
        state.append_output(stream, data, SystemTime::now());
        self.shared.wake.notify_one();
    }

    /// Queue one lifecycle event from its canonical JSON.
    pub fn publish_event(&self, event_json: &str) -> Result<(), RecordError> {
        self.publish_event_object(parse_event(event_json)?);
        Ok(())
    }

    /// Queue one lifecycle event already decoded to its canonical JSON object; its row is built on the delivery thread.
    pub fn publish_event_object(&self, event: Map<String, Value>) {
        let priority: Priority = event_severity(&event).map_or(Priority::Lifecycle, event_priority);
        let bytes: usize = json_bytes(&event);
        let record = Queued {
            payload: Payload::Event(event),
            stream: DeliveryStream::Events,
            bytes,
        };
        let mut state = locked(&self.shared.state);
        state.push_record(record, priority);
        self.shared.wake.notify_one();
    }

    /// Queue one lifecycle event of `event_type` for the replacement sink, which runs on the delivery thread within the shutdown budget.
    pub fn publish_pending_event(
        &self,
        event: Box<dyn PendingLifecycleEvent>,
        (event_type, schema_version): (&str, i64),
    ) {
        let mut identity: Map<String, Value> = Map::new();
        identity.insert("event_type".to_owned(), Value::from(event_type));
        identity.insert("schema_version".to_owned(), Value::from(schema_version));
        let priority: Priority =
            event_severity(&identity).map_or(Priority::Lifecycle, event_priority);
        let mut state = locked(&self.shared.state);
        state.push_pending(event, priority);
        self.shared.wake.notify_one();
    }

    /// Queue one diagnostic from its canonical JSON, when at or above the minimum severity.
    pub fn publish_diagnostic(&self, diagnostic_json: &str) -> Result<(), RecordError> {
        let mut state = locked(&self.shared.state);
        let sequence = state.diagnostic_sequence;
        let diagnostic = diagnostic_input_from_json(diagnostic_json, sequence)?;
        let rank = severity_rank(&diagnostic.severity);
        if rank < self.min_severity {
            return Ok(());
        }
        state.diagnostic_sequence += 1;
        let priority = if rank >= severity_rank("warning") {
            Priority::Diagnostic
        } else {
            Priority::DebugDiagnostic
        };
        let row = diagnostic_row(&state.envelope, &diagnostic);
        state.push(vec![row], DeliveryStream::Diagnostics, priority);
        self.shared.wake.notify_one();
        Ok(())
    }

    /// Queue the SQL a statement executed: its text and one reference once per digest per invocation; statements find it by `sql_digest`.
    pub fn publish_sql(&self, sql: &str, statement_id: Option<&str>, resource_id: Option<&str>) {
        self.queue_sql(sql, (statement_id, resource_id), Priority::Artifact);
    }

    /// Queue the SQL of a statement that failed or was cancelled again, above output and diagnostics, so a full queue or a spent shutdown budget keeps it; it is stored once if it already was.
    pub fn publish_failed_sql(
        &self,
        sql: &str,
        statement_id: Option<&str>,
        resource_id: Option<&str>,
    ) {
        self.queue_sql(sql, (statement_id, resource_id), Priority::FailedTerminal);
    }

    fn queue_sql(
        &self,
        sql: &str,
        (statement_id, resource_id): (Option<&str>, Option<&str>),
        priority: Priority,
    ) {
        if sql.len() > MAX_ARTIFACT_BYTES {
            let mut state = locked(&self.shared.state);
            let counts = state.counts_mut(DeliveryStream::Artifacts);
            counts.accepted += 1;
            counts.dropped += 1;
            return;
        }
        let record = Queued {
            bytes: sql.len(),
            payload: Payload::Sql(SqlInput {
                sql: sql.to_owned(),
                statement_id: statement_id.map(str::to_owned),
                resource_id: resource_id.map(str::to_owned),
                seen_at: SystemTime::now(),
            }),
            stream: DeliveryStream::Artifacts,
        };
        let mut state = locked(&self.shared.state);
        state.push_record(record, priority);
        self.shared.wake.notify_one();
    }

    /// Queue the context row held by `SessionConfig::defer_context`, with whether a Recall replacement sink filters this invocation; later calls do nothing.
    pub fn publish_context(&self, filter_active: bool) {
        let mut state = locked(&self.shared.state);
        state.release_context(filter_active);
        self.shared.wake.notify_one();
    }

    /// Pass every output chunk produced from now on through `replacement` before it is stored; only the first replacement is kept.
    pub fn set_output_replacement(&self, replacement: Arc<dyn OutputReplacement>) {
        drop(self.shared.output_replacement.set(replacement));
    }

    /// Count records a replacement sink filtered out of `stream`.
    pub fn record_filtered(&self, stream: DeliveryStream, count: u64) {
        let mut state = locked(&self.shared.state);
        let counts = state.counts_mut(stream);
        counts.accepted += count;
        counts.filtered += count;
    }

    /// The accounting so far.
    #[must_use]
    pub fn summary(&self) -> DeliverySummary {
        let state = locked(&self.shared.state);
        DeliverySummary {
            streams: DeliveryStream::ALL
                .iter()
                .map(|stream| (*stream, state.counts[stream.index()]))
                .collect(),
            flush_complete: state.finished && state.queued() == 0 && state.degraded.is_none(),
            degraded_reason: state.degraded.clone(),
        }
    }

    /// Start closing: flush pending output and give delivery the shutdown budget from now.
    pub fn begin_close(&self) {
        let mut state = locked(&self.shared.state);
        if !state.closing {
            state.release_context(false);
            state.closing = true;
            state.deadline = Some(Instant::now() + self.shutdown_timeout);
            state.flush_tails();
        }
        self.shared.wake.notify_all();
    }

    /// Wait up to `slice` for delivery to finish; past the deadline, drop what is left and report done.
    pub fn wait_closed(&self, slice: Duration) -> bool {
        let mut state = locked(&self.shared.state);
        let Some(deadline) = state.deadline else {
            return state.finished;
        };
        let until: Instant = (Instant::now() + slice).min(deadline);
        while !state.finished {
            let now = Instant::now();
            if now >= deadline {
                state.rejecting = true;
                state.drop_all();
                state.degraded.get_or_insert_with(|| {
                    "Recall delivery did not finish within the shutdown timeout".to_owned()
                });
                return true;
            }
            if now >= until {
                return false;
            }
            state = self
                .shared
                .wake
                .wait_timeout(state, until - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        true
    }

    /// Stop waiting for delivery at once, as on a second interrupt; what is queued is dropped.
    pub fn abandon(&self) {
        let mut state = locked(&self.shared.state);
        state.rejecting = true;
        state.drop_all();
        state
            .degraded
            .get_or_insert_with(|| "Recall delivery was interrupted".to_owned());
        self.shared.wake.notify_all();
    }

    /// Flush within the shutdown budget, a hard cap on the delay, and return the accounting; records still queued at the deadline are counted as dropped.
    pub fn close(&self) -> DeliverySummary {
        self.begin_close();
        while !self.wait_closed(self.shutdown_timeout) {}
        if locked(&self.shared.state).finished {
            let delivery = self
                .delivery
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(delivery) = delivery {
                drop(delivery.join());
            }
        }
        self.summary()
    }

    /// Store notices the user should see, such as a corrupt store moved aside.
    #[must_use]
    pub fn notices(&self) -> Vec<String> {
        locked(&self.shared.state).notices.clone()
    }

    /// Informational store notices, such as a database Recall created; they report no problem.
    #[must_use]
    pub fn information(&self) -> Vec<String> {
        locked(&self.shared.state).information.clone()
    }
}
