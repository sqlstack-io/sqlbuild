//! What a run printed per node and stream, and what its nodes logged; output can quote warehouse data, so it needs view.evidence.

use serde_json::json;

use crate::credentials::models::Action;
use crate::run_output::_helpers::index::numbers;
use crate::run_output::models::{
    NodeLogLine, OutputStreamSummary, PersonRunRead, RunOutputIndex, StoredStream,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_history::_helpers::run_recall::sensitive_run_read;

/// `None` when the run is not in this location and target; each read is audited.
pub async fn read_person_output_index(
    store: &ControlStore,
    (credentials, run, default_role): PersonRunRead<'_>,
) -> Result<Option<RunOutputIndex>, ControlError> {
    let run_id = run.2;
    sensitive_run_read(
        store,
        credentials,
        (run, default_role, Action::ViewEvidence),
        (
            "output.index",
            async move |transaction, _| {
                let (transaction, streams): (_, Vec<StoredStream>) = query_records(transaction,
                    "SELECT node_key, stream, line_count, text_bytes, error_lines, warning_lines FROM sqb_run_output_streams \
                     WHERE run_id=$1 ORDER BY node_key, stream",
                    vec![Parameter::Id(run_id)]).await?;
                let (transaction, totals) = query(transaction,
                    "SELECT omitted_bytes, omitted_lines, dropped_bytes FROM sqb_run_output WHERE run_id=$1",
                    vec![Parameter::Id(run_id)]).await?;
                let (transaction, logs): (_, Vec<NodeLogLine>) = query_records(transaction,
                    "SELECT node_key, level, message, logged_at FROM sqb_run_node_logs WHERE run_id=$1 ORDER BY logged_at, event_id",
                    vec![Parameter::Id(run_id)]).await?;
                let mut summaries: Vec<OutputStreamSummary> = Vec::with_capacity(streams.len());
                for stream in streams {
                    summaries.push(OutputStreamSummary {
                        error_lines: numbers(&stream.error_lines)?,
                        warning_lines: numbers(&stream.warning_lines)?,
                        node_key: stream.node_key,
                        stream: stream.stream,
                        line_count: stream.line_count,
                        text_bytes: stream.text_bytes,
                    });
                }
                let total = |name: &str| totals.first().and_then(|row| row.get(name)).and_then(serde_json::Value::as_i64).unwrap_or_default();
                Ok((transaction, RunOutputIndex {
                    streams: summaries,
                    omitted_bytes: total("omitted_bytes"),
                    omitted_lines: total("omitted_lines"),
                    dropped_bytes: total("dropped_bytes"),
                    logs,
                }))
            },
            |index: &RunOutputIndex| json!({"streams": index.streams.len(), "logs": index.logs.len()}),
        ),
    )
    .await
}
