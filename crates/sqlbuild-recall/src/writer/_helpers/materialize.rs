//! Rows of queued input, built on the delivery thread so producers only enqueue.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::records::constants::{
    ARTIFACT_DIGEST_PREFIX, AUDIT_COMPLETED_EVENT, AUDIT_EVIDENCE_KIND, AUDIT_EVIDENCE_MEDIA_TYPE,
    AUDIT_EVIDENCE_ROLE, EVIDENCE_PAYLOAD_FIELD, EXECUTED_SQL_ROLE, SAMPLE_DIGEST_PAYLOAD_FIELD,
    SQL_ARTIFACT_KIND, SQL_MEDIA_TYPE, STARTED_AT_FIELD, STATEMENT_STARTED_EVENT,
    STATEMENT_TERMINAL_EVENTS,
};
use crate::records::main::artifact_rows::artifact_rows;
use crate::records::main::event_object_row::event_object_row;
use crate::records::main::format_timestamp::format_timestamp;
use crate::records::main::output_row::output_row;
use crate::records::main::sha256_hex::sha256_hex;
use crate::records::models::{ArtifactInput, OutputChunk, OutputStream, RecallRow, RowEnvelope};
use crate::settings::models::AuditEvidence;
use crate::writer::_helpers::output_chunk::output_chunk;
use crate::writer::_helpers::queue::{Payload, Queued, SqlInput};
use crate::writer::models::{DeliveryStream, ReplacedEvents, ReplacedOutput};
use crate::writer::types::{OutputReplacement, PendingLifecycleEvent};

/// A batch ready to write: its rows, the streams of the records they came from, and records already settled.
#[derive(Debug, Default)]
pub(crate) struct Built {
    pub(crate) rows: Vec<RecallRow>,
    /// One entry per record the rows were built from, for delivery accounting.
    pub(crate) written: Vec<DeliveryStream>,
    /// Records that are already stored, such as SQL this invocation recorded before.
    pub(crate) settled: Vec<DeliveryStream>,
    /// Records whose rows could not be built, or that a replacement sink failed on.
    pub(crate) unbuildable: Vec<DeliveryStream>,
    /// Records a replacement sink chose not to store.
    pub(crate) filtered: Vec<DeliveryStream>,
    /// Events a lifecycle replacement published beyond the one it was handed, accepted as they are built.
    pub(crate) extra_events: u64,
    /// Audit evidence artifacts taken out of their events, accepted as they are built.
    pub(crate) extra_artifacts: u64,
}

impl Built {
    /// Account for one event's row and the artifact rows of its evidence.
    fn add_event(
        &mut self,
        stream: DeliveryStream,
        (row, evidence_rows): (
            Result<RecallRow, crate::records::errors::RecordError>,
            Option<Vec<RecallRow>>,
        ),
    ) {
        if let Some(rows) = evidence_rows {
            self.rows.extend(rows);
            self.written.push(DeliveryStream::Artifacts);
            self.extra_artifacts += 1;
        }
        match row {
            Ok(row) => {
                self.rows.push(row);
                self.written.push(stream);
            }
            Err(_) => self.unbuildable.push(stream),
        }
    }
}

/// Builds rows for one session; remembers which SQL and references it has stored.
#[derive(Debug)]
pub(crate) struct Materializer {
    envelope: RowEnvelope,
    /// Digests whose content row and one reference row are written.
    artifacts_seen: HashSet<String>,
    /// Output bytes stored per stream, for the offsets of replaced chunks.
    output_bytes: [u64; 2],
    audit_evidence: AuditEvidence,
}

impl Materializer {
    pub(crate) fn new(envelope: RowEnvelope, audit_evidence: AuditEvidence) -> Self {
        Self {
            audit_evidence,
            envelope,
            artifacts_seen: HashSet::new(),
            output_bytes: [0; 2],
        }
    }

    /// The output rows to store for `rows`, passed through `replacement`: what it published, under the chunk's sequence.
    fn output_rows(
        &mut self,
        rows: Vec<RecallRow>,
        replacement: Option<&dyn OutputReplacement>,
    ) -> Result<Vec<RecallRow>, ReplacedOutput> {
        let mut stored: Vec<RecallRow> = Vec::with_capacity(rows.len());
        for row in rows {
            let Some(chunk) = output_chunk(&row) else {
                stored.push(row);
                continue;
            };
            let index = usize::from(chunk.stream == OutputStream::Stderr);
            let Some(replacement) = replacement else {
                self.output_bytes[index] += chunk.data.len() as u64;
                stored.push(row);
                continue;
            };
            if chunk.omitted.is_some() {
                let gap = OutputChunk {
                    byte_offset: self.output_bytes[index],
                    ..chunk
                };
                stored.push(output_row(&self.envelope, &gap));
                continue;
            }
            match replacement.replace(&chunk) {
                ReplacedOutput::Published(data) => {
                    let replaced = OutputChunk {
                        byte_offset: self.output_bytes[index],
                        data,
                        ..chunk
                    };
                    self.output_bytes[index] += replaced.data.len() as u64;
                    stored.push(output_row(&self.envelope, &replaced));
                }
                other => return Err(other),
            }
        }
        Ok(stored)
    }

    /// The content row and its reference row the first time a digest is seen; `None` after that.
    fn sql_rows(&mut self, input: SqlInput) -> Option<Vec<RecallRow>> {
        let artifact: ArtifactInput = artifact_input(
            input.sql.into_bytes(),
            (SQL_ARTIFACT_KIND, SQL_MEDIA_TYPE, EXECUTED_SQL_ROLE),
            (input.statement_id, input.resource_id),
            &format_timestamp(input.seen_at),
        );
        self.artifact_rows(artifact)
    }

    /// `artifact`'s content row and one reference row per invocation, named by the first statement or event written with it; statements and audits find it by digest. `None` once its digest is written.
    fn artifact_rows(&mut self, artifact: ArtifactInput) -> Option<Vec<RecallRow>> {
        if !self.artifacts_seen.insert(artifact.digest.clone()) {
            return None;
        }
        Some(artifact_rows(&self.envelope, &artifact).to_vec())
    }

    /// The row of `event`, then the artifact rows of its evidence: an `audit_completed` event's evidence rows leave its payload and, when the outcome keeps them, become an artifact its `sample_digest` names.
    fn event_rows(
        &mut self,
        event: Map<String, Value>,
    ) -> (
        Result<RecallRow, crate::records::errors::RecordError>,
        Option<Vec<RecallRow>>,
    ) {
        let (event, artifact): (Map<String, Value>, Option<ArtifactInput>) =
            self.take_evidence(event);
        let evidence_rows: Option<Vec<RecallRow>> =
            artifact.and_then(|artifact| self.artifact_rows(artifact));
        (event_object_row(&self.envelope, &event), evidence_rows)
    }

    /// `event` without its evidence rows, and the artifact holding them when its outcome keeps them.
    fn take_evidence(
        &self,
        mut event: Map<String, Value>,
    ) -> (Map<String, Value>, Option<ArtifactInput>) {
        let evidence: Option<Value> =
            if event.get("event_type").and_then(Value::as_str) == Some(AUDIT_COMPLETED_EVENT) {
                event
                    .get_mut("payload")
                    .and_then(Value::as_object_mut)
                    .and_then(|payload| payload.remove(EVIDENCE_PAYLOAD_FIELD))
            } else {
                None
            };
        let text = |field: &str| -> Option<String> {
            event.get(field).and_then(Value::as_str).map(str::to_owned)
        };
        let outcome: Option<&str> = event
            .get("payload")
            .and_then(|payload| payload.get("outcome"))
            .and_then(Value::as_str);
        let Some(evidence) =
            evidence.filter(|_| self.audit_evidence.keeps(outcome.unwrap_or_default()))
        else {
            return (event, None);
        };
        let mut artifact: ArtifactInput = artifact_input(
            evidence.to_string().into_bytes(),
            (
                AUDIT_EVIDENCE_KIND,
                AUDIT_EVIDENCE_MEDIA_TYPE,
                AUDIT_EVIDENCE_ROLE,
            ),
            (None, text("resource_id")),
            &text("occurred_at").unwrap_or_default(),
        );
        artifact.ref_id = text("event_id").unwrap_or_default();
        let digest: String = artifact
            .digest
            .strip_prefix(ARTIFACT_DIGEST_PREFIX)
            .unwrap_or(&artifact.digest)
            .to_owned();
        if let Some(payload) = event.get_mut("payload").and_then(Value::as_object_mut) {
            payload.insert(SAMPLE_DIGEST_PAYLOAD_FIELD.to_owned(), Value::from(digest));
        }
        (event, Some(artifact))
    }

    /// The rows of `batch`, with each record's stream sorted by how it is accounted; lifecycle replacements see their events in publication order.
    pub(crate) fn build(
        &mut self,
        batch: Vec<Queued>,
        replacement: Option<&dyn OutputReplacement>,
    ) -> Built {
        let mut built = Built::default();
        let mut pending_events: Vec<(u64, Box<dyn PendingLifecycleEvent>)> = Vec::new();
        let starts: HashMap<String, String> = ended_starts(&batch);
        for record in batch {
            let stream: DeliveryStream = record.stream;
            match record.payload {
                Payload::Event(event) if starts.contains_key(statement_of(&event, true)) => {
                    built.settled.push(stream);
                }
                Payload::Event(mut event) if starts.contains_key(statement_of(&event, false)) => {
                    let started_at = starts[statement_of(&event, false)].clone();
                    event.insert(STARTED_AT_FIELD.to_owned(), Value::from(started_at));
                    built.add_event(stream, self.event_rows(event));
                }
                Payload::Rows(rows) if stream == DeliveryStream::Output => {
                    match self.output_rows(rows, replacement) {
                        Ok(rows) => {
                            built.rows.extend(rows);
                            built.written.push(stream);
                        }
                        Err(ReplacedOutput::Filtered) => built.filtered.push(stream),
                        Err(_) => built.unbuildable.push(stream),
                    }
                }
                Payload::Rows(rows) => {
                    built.rows.extend(rows);
                    built.written.push(stream);
                }
                Payload::Event(event) => built.add_event(stream, self.event_rows(event)),
                Payload::Pending(sequence, pending) => pending_events.push((sequence, pending)),
                Payload::Sql(input) => match self.sql_rows(input) {
                    Some(rows) => {
                        built.rows.extend(rows);
                        built.written.push(stream);
                    }
                    None => built.settled.push(stream),
                },
            }
        }
        pending_events.sort_by_key(|(sequence, _)| *sequence);
        for (_, pending) in pending_events {
            let stream = DeliveryStream::Events;
            match pending.replace() {
                ReplacedEvents::Published(events) if events.is_empty() => {
                    built.filtered.push(stream);
                }
                ReplacedEvents::Published(events) => {
                    built.extra_events += events.len() as u64 - 1;
                    for event in events {
                        built.add_event(stream, self.event_rows(event));
                    }
                }
                ReplacedEvents::Failed => built.unbuildable.push(stream),
            }
        }
        built
    }
}

/// The statement an event starts (`start`) or ends; empty for any other event.
fn statement_of(event: &Map<String, Value>, start: bool) -> &str {
    let text = |field: &str| event.get(field).and_then(Value::as_str);
    let event_type: &str = text("event_type").unwrap_or_default();
    let matches: bool = if start {
        event_type == STATEMENT_STARTED_EVENT
    } else {
        STATEMENT_TERMINAL_EVENTS.contains(&event_type)
    };
    if matches {
        text("statement_id").unwrap_or_default()
    } else {
        ""
    }
}

/// Start times of the statements that both start and end in `batch`, by statement ID: one row records each. A batch takes higher priorities first, so an end may precede its start.
fn ended_starts(batch: &[Queued]) -> HashMap<String, String> {
    let events = || {
        batch.iter().filter_map(|record| match &record.payload {
            Payload::Event(event) => Some(event),
            _ => None,
        })
    };
    let mut starts: HashMap<&str, &str> = HashMap::new();
    for event in events() {
        let started: &str = statement_of(event, true);
        let at: &str = event
            .get("occurred_at")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !started.is_empty() && !at.is_empty() {
            starts.entry(started).or_insert(at);
        }
    }
    events()
        .filter_map(|event| {
            let ending: &str = statement_of(event, false);
            starts
                .get(ending)
                .map(|at| (ending.to_owned(), (*at).to_owned()))
        })
        .collect()
}

/// An artifact of `content`, referenced by its statement or, without one, by its digest.
fn artifact_input(
    content: Vec<u8>,
    (kind, media_type, role): (&str, &str, &str),
    (statement_id, resource_id): (Option<String>, Option<String>),
    seen_at: &str,
) -> ArtifactInput {
    let digest = format!("{ARTIFACT_DIGEST_PREFIX}{}", sha256_hex(&content));
    ArtifactInput {
        ref_id: statement_id.clone().unwrap_or_else(|| digest.clone()),
        digest,
        kind: kind.to_owned(),
        media_type: media_type.to_owned(),
        content,
        seen_at: seen_at.to_owned(),
        role: role.to_owned(),
        statement_id,
        resource_id,
    }
}
