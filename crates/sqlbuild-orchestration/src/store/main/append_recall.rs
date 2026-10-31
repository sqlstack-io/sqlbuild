//! Append Recall facts and retry-conflict evidence to the caller's control transaction.

use chrono::SecondsFormat;
use sqlbuild_recall::records::main::conflict_row::conflict_row;
use sqlbuild_recall::records::models::{RecallRow, RowEnvelope};
use sqlbuild_recall::schema::main::table_spec::table_spec;
use sqlbuild_recall::schema::models::Table;

use crate::store::_helpers::recall_rows::{text, write_row};
use crate::store::errors::ControlError;
use crate::store::models::{ControlTransaction, RecallAppendOutcome};

pub(crate) async fn append_recall(
    transaction: ControlTransaction,
    row: &RecallRow,
) -> Result<(ControlTransaction, RecallAppendOutcome), ControlError> {
    let (mut transaction, outcome) = write_row(transaction, row).await?;
    if let Some(evidence) = &outcome.conflict
        && row.table != Table::Conflicts
    {
        let envelope = RowEnvelope {
            invocation_id: text(row, "invocation_id")?.into(),
            invocation_started_at: text(row, "invocation_started_at")?.into(),
            producer_version: text(row, "producer_version")?.into(),
        };
        let detected_at = transaction.now.to_rfc3339_opts(SecondsFormat::Micros, true);
        let conflict = conflict_row(
            &envelope,
            (table_spec(row.table).name, &evidence.record_key),
            (&evidence.existing_sha256, &evidence.rejected_sha256),
            &detected_at,
        );
        (transaction, _) = write_row(transaction, &conflict).await?;
    }
    Ok((transaction, outcome))
}
