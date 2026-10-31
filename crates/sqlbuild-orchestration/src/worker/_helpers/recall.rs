//! Reuse the native delivery thread and accounting over claim-scoped HTTP, without Python.

use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlbuild_recall::records::models::{RecallRow, RowEnvelope};
use sqlbuild_recall::schema::main::table_spec::table_spec;
use sqlbuild_recall::schema::models::Table;
use sqlbuild_recall::store::errors::{StoreError, StoreErrorKind};
use sqlbuild_recall::store::models::InsertOutcome;
use sqlbuild_recall::store::types::{RecallStore, RecallStoreOpener};
use sqlbuild_recall::writer::constants::{BATCH_ROWS, BATCH_WINDOW_MS};
use sqlbuild_recall::writer::models::SessionStore;

use crate::api::constants::BODY_BYTES;
use crate::api::main::client_response::client_response;
use crate::runs::constants::RECALL_BATCH_ROWS;
use crate::runs::models::{RecallBatch, RecallRecord};
use crate::worker::models::WorkerClient;

#[derive(Debug)]
struct WorkerRecallOpener(WorkerClient);

struct WorkerRecallStore {
    runtime: tokio::runtime::Runtime,
    client: WorkerClient,
    invocation_id: String,
}

fn failure(message: impl Into<String>, retryable: bool) -> StoreError {
    StoreError {
        kind: if retryable {
            StoreErrorKind::Busy
        } else {
            StoreErrorKind::Other
        },
        message: message.into(),
    }
}

fn transport_record(row: &RecallRow) -> RecallRecord {
    RecallRecord {
        table: row.table,
        values: table_spec(row.table)
            .columns
            .iter()
            .filter_map(|column| {
                let value = row.value(column.name)?;
                Some((column.name.to_owned(), value.clone()))
            })
            .collect(),
    }
}

impl WorkerClient {
    /// Pass this to the native Recall session; all delivery finishes before terminal reporting.
    #[must_use]
    pub fn recall_store(&self) -> SessionStore {
        SessionStore::Hosted(Arc::new(WorkerRecallOpener(self.clone())))
    }

    /// The server owns common envelope columns and recomputes each row's content hash.
    pub async fn publish_recall(
        &self,
        batch: &RecallBatch,
    ) -> Result<InsertOutcome, crate::worker::errors::WorkerError> {
        Ok(client_response(
            self.connection
                .client
                .post(format!("{}recall", self.run_url))
                .json(batch),
        )
        .await?)
    }
}

impl RecallStoreOpener for WorkerRecallOpener {
    fn open(
        &self,
        envelope: &RowEnvelope,
        give_up: Instant,
    ) -> Result<Box<dyn RecallStore>, StoreError> {
        if Instant::now() >= give_up {
            return Err(failure("Recall delivery deadline expired", true));
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| failure("Recall delivery runtime could not start", false))?;
        let client = {
            let _entered = runtime.enter();
            WorkerClient {
                connection: self
                    .0
                    .connection
                    .independent()
                    .map_err(|_| failure("Recall HTTP client could not start", false))?,
                ..self.0.clone()
            }
        };
        Ok(Box::new(WorkerRecallStore {
            runtime,
            client,
            invocation_id: envelope.invocation_id.clone(),
        }))
    }

    fn batch_rows(&self) -> usize {
        BATCH_ROWS
    }

    fn batch_window(&self) -> Duration {
        Duration::from_millis(BATCH_WINDOW_MS)
    }
}

/// Native materialization emits content/reference pairs, whose proof stays in one HTTP request.
impl RecallStore for WorkerRecallStore {
    fn insert(&self, rows: &[RecallRow], give_up: Instant) -> Result<InsertOutcome, StoreError> {
        if Instant::now() >= give_up {
            return Err(failure("Recall delivery deadline expired", true));
        }
        let mut batch = RecallBatch {
            invocation_id: self.invocation_id.clone(),
            records: Vec::new(),
        };
        let base_bytes = serde_json::to_vec(&batch)
            .map_err(|_| failure("Recall batch could not be encoded", false))?
            .len();
        self.runtime.block_on(async {
            let mut result = InsertOutcome::default();
            let mut bytes = base_bytes;
            let mut offset = 0;
            while offset < rows.len() {
                if Instant::now() >= give_up {
                    return Err(failure("Recall delivery deadline expired", true));
                }
                let paired = rows[offset].table == Table::Artifacts
                    && rows.get(offset + 1).is_some_and(|next| {
                        next.table == Table::ArtifactRefs
                            && next.value("digest") == rows[offset].value("digest")
                    });
                let end = offset + if paired { 2 } else { 1 };
                let records = rows[offset..end]
                    .iter()
                    .map(transport_record)
                    .collect::<Vec<_>>();
                let added = serde_json::to_vec(&records)
                    .map_err(|_| failure("Recall records could not be encoded", false))?
                    .len()
                    - 2;
                if base_bytes + added > BODY_BYTES {
                    return Err(failure("Recall record exceeds the HTTP body limit", false));
                }
                if !batch.records.is_empty()
                    && (batch.records.len() + records.len() > RECALL_BATCH_ROWS
                        || bytes + added + 1 > BODY_BYTES)
                {
                    (batch, result) = self.flush(batch, result, give_up).await?;
                    bytes = base_bytes;
                }
                bytes += added + usize::from(!batch.records.is_empty());
                batch.records.extend(records);
                offset = end;
            }
            if !batch.records.is_empty() || rows.is_empty() {
                (_, result) = self.flush(batch, result, give_up).await?;
            }
            result.delivered.sort_by_key(|(table, _)| *table);
            Ok(result)
        })
    }
}

impl WorkerRecallStore {
    async fn flush(
        &self,
        mut batch: RecallBatch,
        mut result: InsertOutcome,
        give_up: Instant,
    ) -> Result<(RecallBatch, InsertOutcome), StoreError> {
        let outcome = tokio::time::timeout_at(
            tokio::time::Instant::from_std(give_up),
            self.client.publish_recall(&batch),
        )
        .await
        .map_err(|_| failure("Recall delivery deadline expired", true))?
        .map_err(|error| {
            let retryable = match &error {
                crate::worker::errors::WorkerError::Transport(_) => true,
                crate::worker::errors::WorkerError::Rejected(status) => {
                    status.is_server_error() || *status == reqwest::StatusCode::TOO_MANY_REQUESTS
                }
                _ => false,
            };
            failure(error.to_string(), retryable)
        })?;
        for (table, count) in outcome.delivered {
            if let Some((_, previous)) = result
                .delivered
                .iter_mut()
                .find(|(found, _)| *found == table)
            {
                *previous += count;
            } else {
                result.delivered.push((table, count));
            }
        }
        result.conflicts.extend(outcome.conflicts);
        result.failed.extend(outcome.failed);
        result.failure = result.failure.take().or(outcome.failure);
        batch.records.clear();
        Ok((batch, result))
    }
}
